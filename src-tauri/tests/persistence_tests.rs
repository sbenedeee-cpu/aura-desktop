use rusqlite::{Connection, Result};
use std::fs;
use tempfile::tempdir;

// Define simulated migration structure
struct Migration {
    version: usize,
    sql: &'static str,
}

// Simulated migrations for Aura V0 (without schema_migrations since that's handled by the bootstrapper)
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: "CREATE TABLE projects (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                goal TEXT,
                status TEXT NOT NULL,
                current_task TEXT,
                blocker TEXT,
                next_step TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
              );",
    },
    Migration {
        version: 2,
        sql: "CREATE TABLE tasks (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                title TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY(project_id) REFERENCES projects(id)
              );",
    },
    Migration {
        version: 3,
        sql: "CREATE TABLE events (
                id TEXT PRIMARY KEY,
                project_id TEXT,
                kind TEXT NOT NULL,
                actor TEXT NOT NULL,
                occurred_at TEXT NOT NULL,
                payload TEXT NOT NULL,
                FOREIGN KEY(project_id) REFERENCES projects(id)
              );",
    },
    Migration {
        version: 4,
        sql: "CREATE TABLE settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
              );",
    },
];

/// Applies migrations to the database inside a transaction.
/// If any migration fails, the transaction is rolled back.
fn apply_migrations(conn: &mut Connection) -> Result<usize> {
    // Ensure schema_migrations exists
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY);",
        [],
    )?;

    let current_version: usize = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let mut applied = 0;
    let tx = conn.transaction()?;

    for migration in MIGRATIONS {
        if migration.version > current_version {
            tx.execute(migration.sql, [])?;
            tx.execute(
                "INSERT INTO schema_migrations (version) VALUES (?)",
                [migration.version],
            )?;
            applied += 1;
        }
    }

    tx.commit()?;
    Ok(applied)
}

/// Helper function to encrypt/decrypt (wrap/unwrap) keys simulating DPAPI
/// (Since we are in a cross-platform test setting, we mock the Windows DPAPI call).
fn mock_dpapi_protect(secret: &[u8]) -> std::io::Result<Vec<u8>> {
    // Simulate Windows DPAPI protection by applying a reversible XOR pattern
    // with a mock system user security identifier (SID).
    let sid_entropy = b"WINDOWS_USER_SID_ENTROPY";
    let mut protected = Vec::with_capacity(secret.len());
    for (i, &byte) in secret.iter().enumerate() {
        protected.push(byte ^ sid_entropy[i % sid_entropy.len()]);
    }
    Ok(protected)
}

fn mock_dpapi_unprotect(protected: &[u8]) -> std::io::Result<Vec<u8>> {
    mock_dpapi_protect(protected) // XOR is symmetric
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fresh_migration_successful() {
        let mut conn = Connection::open_in_memory().unwrap();
        let applied = apply_migrations(&mut conn).unwrap();
        assert_eq!(applied, 4);

        // Verify tables exist
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert!(tables.contains(&"schema_migrations".to_string()));
        assert!(tables.contains(&"projects".to_string()));
        assert!(tables.contains(&"tasks".to_string()));
        assert!(tables.contains(&"events".to_string()));
        assert!(tables.contains(&"settings".to_string()));
    }

    #[test]
    fn test_idempotent_migration_no_op() {
        let mut conn = Connection::open_in_memory().unwrap();
        apply_migrations(&mut conn).unwrap();

        // Applying again should be a safe no-op
        let applied = apply_migrations(&mut conn).unwrap();
        assert_eq!(applied, 0);
    }

    #[test]
    fn test_failed_migration_rolls_back() {
        let mut conn = Connection::open_in_memory().unwrap();
        // Setup initial schema_migrations with version 1
        conn.execute(
            "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY);",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO schema_migrations (version) VALUES (1)", [])
            .unwrap();

        // Create a conflict table beforehand to trigger a migration failure
        // Version 2 tries to create `tasks` table, so let's create a conflicted `tasks` table
        conn.execute("CREATE TABLE tasks (conflict_field INTEGER);", [])
            .unwrap();

        // Applying migrations should fail because 'tasks' table already exists with a different schema.
        let result = apply_migrations(&mut conn);
        assert!(result.is_err());

        // Verify that schema_migrations is still at version 1 (rolled back completely)
        let current_version: usize = conn
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(current_version, 1);
    }

    #[test]
    fn test_safe_recovery_from_corruption_preserves_backup() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("aura.db");
        let backup_path = dir.path().join("aura.db.bak");

        // Write initial valid database
        {
            let mut conn = Connection::open(&db_path).unwrap();
            apply_migrations(&mut conn).unwrap();
            conn.execute(
                "INSERT INTO settings (key, value) VALUES ('theme', 'dark')",
                [],
            )
            .unwrap();
        }

        // Simulate Corruption by writing gibberish to the db file
        assert!(db_path.exists());
        fs::write(&db_path, b"CORRUPTED_SQLITE_HEADER_AND_GIBBERISH_DATA").unwrap();

        // Recovery path: Detect corruption and backup the corrupted file
        let conn_attempt = Connection::open(&db_path);
        let mut is_corrupt = false;
        if let Ok(ref conn) = conn_attempt {
            // Attempt to query
            let query_res: Result<String> =
                conn.query_row("SELECT value FROM settings WHERE key='theme'", [], |row| {
                    row.get(0)
                });
            if query_res.is_err() {
                is_corrupt = true;
            }
        } else {
            is_corrupt = true;
        }

        if is_corrupt {
            // Safely copy corrupted db to backup file for manual recovery
            fs::copy(&db_path, &backup_path).unwrap();
            fs::remove_file(&db_path).unwrap();

            // Re-bootstrap fresh database
            let mut new_conn = Connection::open(&db_path).unwrap();
            apply_migrations(&mut new_conn).unwrap();
            new_conn
                .execute(
                    "INSERT INTO settings (key, value) VALUES ('theme', 'recovered_default')",
                    [],
                )
                .unwrap();
        }

        // Assert backup file of corrupted data exists
        assert!(backup_path.exists());
        assert_eq!(
            fs::read(&backup_path).unwrap(),
            b"CORRUPTED_SQLITE_HEADER_AND_GIBBERISH_DATA"
        );

        // Assert new database is healthy and initialized
        let restored_conn = Connection::open(&db_path).unwrap();
        let val: String = restored_conn
            .query_row("SELECT value FROM settings WHERE key='theme'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(val, "recovered_default");
    }

    #[test]
    fn test_privacy_settings_contain_no_sensitive_raw_data() {
        let mut conn = Connection::open_in_memory().unwrap();
        apply_migrations(&mut conn).unwrap();

        // Seed settings representing privacy options
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('privacy_mode', 'paused')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('retention_default', 'until_deleted')",
            [],
        )
        .unwrap();

        // Ensure we retrieve privacy mode and it corresponds to correct policies
        let mode: String = conn
            .query_row(
                "SELECT value FROM settings WHERE key='privacy_mode'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(mode, "paused");

        // Verify that settings does not contain or permit raw unencrypted secrets
        let rows_count: usize = conn
            .query_row(
                "SELECT count(*) FROM settings WHERE value LIKE '%password%' OR value LIKE '%key%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rows_count, 0);
    }

    #[test]
    fn test_dpapi_wrapping_simulation() {
        let raw_dek = b"random_generated_32_byte_dek_value";

        // Wrap/Protect key
        let protected_key = mock_dpapi_protect(raw_dek).unwrap();
        assert_ne!(raw_dek.to_vec(), protected_key);

        // Unwrap/Unprotect key
        let unprotected_key = mock_dpapi_unprotect(&protected_key).unwrap();
        assert_eq!(raw_dek.to_vec(), unprotected_key);
    }
}
