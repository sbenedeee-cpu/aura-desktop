pub mod migrations;
pub mod repositories;

#[cfg(test)]
use crate::db::migrations::current_version;
use crate::db::migrations::run;
use crate::domain::project::AuraError;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use std::path::Path;

pub struct LocalStore {
    connection: Connection,
}

impl LocalStore {
    pub fn open(path: &Path) -> Result<Self, AuraError> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
        )
        .map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not open its local workspace database. Your existing data was not changed: {error}"
            ))
        })?;

        Self::from_connection(connection)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self, AuraError> {
        Self::from_connection(Connection::open_in_memory().map_err(|error| {
            AuraError::Storage(format!("Aura could not create its test database: {error}"))
        })?)
    }

    fn from_connection(mut connection: Connection) -> Result<Self, AuraError> {
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA busy_timeout = 5000;
                 PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;",
            )
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not configure local storage safely: {error}"
                ))
            })?;
        run(&mut connection)?;

        Ok(Self { connection })
    }

    pub fn privacy_mode(&self) -> Result<String, AuraError> {
        self.connection
            .query_row(
                "SELECT value FROM settings WHERE key = 'privacy_mode'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map(|value| value.unwrap_or_else(|| "focused".to_string()))
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not read its local privacy setting: {error}"
                ))
            })
    }

    pub fn set_privacy_mode(&self, mode: &str) -> Result<(), AuraError> {
        self.set_setting("privacy_mode", mode)
    }

    pub fn selected_project_id(&self) -> Result<Option<String>, AuraError> {
        self.connection
            .query_row(
                "SELECT value FROM settings WHERE key = 'selected_project_id'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not read the selected local project: {error}"
                ))
            })
    }

    pub fn set_selected_project_id(&self, project_id: &str) -> Result<(), AuraError> {
        self.set_setting("selected_project_id", project_id)
    }

    pub fn clear_selected_project_id(&self) -> Result<(), AuraError> {
        self.connection
            .execute("DELETE FROM settings WHERE key = 'selected_project_id'", [])
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not clear the selected local project: {error}"
                ))
            })?;
        Ok(())
    }

    fn set_setting(&self, key: &str, value: &str) -> Result<(), AuraError> {
        self.connection
            .execute(
                "INSERT INTO settings (key, value, updated_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                params![key, value, migrations::utc_timestamp()],
            )
            .map_err(|error| {
                AuraError::Storage(format!("Aura could not update its local workspace setting: {error}"))
            })?;
        Ok(())
    }

    pub fn projects(&self) -> repositories::projects::ProjectRepository<'_> {
        repositories::projects::ProjectRepository::new(&self.connection)
    }

    pub fn captures(&self) -> repositories::captures::CaptureRepository<'_> {
        repositories::captures::CaptureRepository::new(&self.connection)
    }

    #[cfg(test)]
    pub fn schema_version(&self) -> Result<i64, AuraError> {
        current_version(&self.connection)
    }
}

#[cfg(test)]
mod tests {
    use super::{migrations::run, LocalStore};
    use rusqlite::Connection;

    #[test]
    fn migration_creates_an_empty_local_workspace() {
        let store = LocalStore::open_in_memory().expect("test database should migrate");

        assert_eq!(store.schema_version().expect("schema version"), 2);
        assert_eq!(store.privacy_mode().expect("privacy mode"), "focused");
        assert!(store.projects().list_active().expect("projects").is_empty());
    }

    #[test]
    fn reapplying_migrations_keeps_the_recorded_schema_version() {
        let mut connection = Connection::open_in_memory().expect("test database");
        run(&mut connection).expect("first migration run");
        run(&mut connection).expect("second migration run");

        let applied_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("migration count");
        assert_eq!(applied_count, 2);
    }

    #[test]
    fn failed_migration_rolls_back_without_resetting_the_existing_database() {
        let mut connection = Connection::open_in_memory().expect("test database");
        connection
            .execute_batch("CREATE TABLE projects (legacy_value TEXT NOT NULL);")
            .expect("legacy fixture");

        assert!(run(&mut connection).is_err());

        let preserved_column: String = connection
            .query_row(
                "SELECT name FROM pragma_table_info('projects') WHERE cid = 0",
                [],
                |row| row.get(0),
            )
            .expect("legacy table remains readable");
        assert_eq!(preserved_column, "legacy_value");
    }
}
