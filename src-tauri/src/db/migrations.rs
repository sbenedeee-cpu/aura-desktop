use crate::domain::project::AuraError;
use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection};

pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial-local-project-workspace",
        sql: "
            CREATE TABLE projects (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                goal TEXT,
                status TEXT NOT NULL CHECK(status IN ('active', 'paused', 'archived')) DEFAULT 'active',
                current_task TEXT,
                blocker TEXT,
                next_step TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                archived_at TEXT
            );

            CREATE INDEX projects_active_updated_at_idx
                ON projects(status, updated_at DESC);

            CREATE TABLE settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE context_markers (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                source TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE INDEX context_markers_project_created_at_idx
                ON context_markers(project_id, created_at DESC);

            CREATE TABLE activity_records (
                id TEXT PRIMARY KEY,
                project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
                kind TEXT NOT NULL,
                title TEXT NOT NULL,
                detail TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE INDEX activity_records_created_at_idx
                ON activity_records(created_at DESC);
        ",
    },
    Migration {
        version: 2,
        name: "project-scoped-manual-captures",
        sql: "
            CREATE TABLE captures (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                kind TEXT NOT NULL CHECK(kind IN ('manual_note', 'pasted_text', 'url')),
                label TEXT NOT NULL,
                content TEXT NOT NULL,
                classification TEXT NOT NULL CHECK(classification IN ('standard', 'sensitive')),
                retention TEXT NOT NULL CHECK(retention IN ('until_deleted', 'review_in_30_days')),
                created_at TEXT NOT NULL
            );

            CREATE INDEX captures_project_created_at_idx
                ON captures(project_id, created_at DESC);
        ",
    },
];

pub fn run(connection: &mut Connection) -> Result<(), AuraError> {
    let transaction = connection.transaction().map_err(|error| {
        AuraError::Storage(format!(
            "Aura could not begin a local schema migration: {error}"
        ))
    })?;

    transaction
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TEXT NOT NULL
             );",
        )
        .map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not prepare schema migration metadata: {error}"
            ))
        })?;

    let current_version: i64 = transaction
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not read the local schema version: {error}"
            ))
        })?;

    let supported_version = MIGRATIONS
        .last()
        .map(|migration| migration.version)
        .unwrap_or(0);
    if current_version > supported_version {
        return Err(AuraError::Storage(
            "Aura found a newer local database format. Update Aura before opening this workspace."
                .to_string(),
        ));
    }

    for migration in MIGRATIONS
        .iter()
        .filter(|migration| migration.version > current_version)
    {
        transaction.execute_batch(migration.sql).map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not apply local schema migration {} ({}). Your existing database was left unchanged: {error}",
                migration.version, migration.name
            ))
        })?;

        transaction
            .execute(
                "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                params![migration.version, migration.name, utc_timestamp()],
            )
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not record local schema migration {}: {error}",
                    migration.version
                ))
            })?;
    }

    transaction.commit().map_err(|error| {
        AuraError::Storage(format!(
            "Aura could not complete the local schema migration safely: {error}"
        ))
    })
}

#[cfg(test)]
pub fn current_version(connection: &Connection) -> Result<i64, AuraError> {
    connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not read the local schema version: {error}"
            ))
        })
}

pub fn utc_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}
