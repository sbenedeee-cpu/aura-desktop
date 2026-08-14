pub mod migrations;
pub mod repositories;

#[cfg(test)]
use crate::db::migrations::current_version;
use crate::db::migrations::run;
use crate::domain::{
    capture::CaptureRetention,
    project::AuraError,
    settings::{
        ExclusionKind, ExclusionRule, PrivacyMode, PrivacyPreferences, SetExclusionEnabledInput,
        UpdatePrivacyPreferencesInput,
    },
};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use std::path::Path;
use uuid::Uuid;

pub struct LocalStore {
    connection: Connection,
}

fn seed_default_capture_retention(connection: &mut Connection) -> Result<(), AuraError> {
    connection
        .execute(
            "INSERT INTO settings (key, value, updated_at)
             SELECT 'default_capture_retention', 'until_deleted', ?1
             WHERE NOT EXISTS (
                 SELECT 1 FROM settings WHERE key = 'default_capture_retention'
             )",
            params![migrations::utc_timestamp()],
        )
        .map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not seed its default capture retention setting: {error}"
            ))
        })?;
    Ok(())
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
        seed_default_capture_retention(&mut connection)?;

        Ok(Self { connection })
    }

    pub fn connection_ref(&self) -> &Connection {
        &self.connection
    }

    pub fn connection_ref_mut(&mut self) -> &mut Connection {
        &mut self.connection
    }

    pub fn privacy_preferences(&self) -> Result<PrivacyPreferences, AuraError> {
        let privacy_mode = self
            .setting_value("privacy_mode")?
            .unwrap_or_else(|| "manual_only".to_string());
        let retention = self.setting_value("default_capture_retention")?;

        let default_capture_retention = retention
            .as_deref()
            .map(CaptureRetention::from_store)
            .transpose()?
            .unwrap_or(CaptureRetention::UntilDeleted);

        let exclusions = self.exclusion_rules()?;

        Ok(PrivacyPreferences {
            privacy_mode: PrivacyMode::from_store(&privacy_mode)?,
            default_capture_retention,
            exclusions,
        })
    }

    pub fn update_privacy_preferences(
        &mut self,
        input: UpdatePrivacyPreferencesInput,
    ) -> Result<PrivacyPreferences, AuraError> {
        let transaction = self.connection.transaction().map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not begin a local privacy settings change: {error}"
            ))
        })?;

        transaction
            .execute(
                "INSERT INTO settings (key, value, updated_at)
                 VALUES ('privacy_mode', ?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                params![input.privacy_mode.as_str(), migrations::utc_timestamp()],
            )
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not update its local privacy mode: {error}"
                ))
            })?;

        transaction
            .execute(
                "INSERT INTO settings (key, value, updated_at)
                 VALUES ('default_capture_retention', ?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                params![input.default_capture_retention.as_str(), migrations::utc_timestamp()],
            )
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not update its local default capture retention: {error}"
                ))
            })?;

        transaction.commit().map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not save its local privacy settings safely. Your earlier settings were not changed: {error}"
            ))
        })?;

        self.privacy_preferences()
    }

    pub fn default_capture_retention(&self) -> Result<CaptureRetention, AuraError> {
        self.setting_value("default_capture_retention")?
            .as_deref()
            .map(CaptureRetention::from_store)
            .transpose()
            .map(|value| value.unwrap_or(CaptureRetention::UntilDeleted))
    }

    pub fn create_exclusion_rule(
        &self,
        kind: ExclusionKind,
        value: String,
    ) -> Result<ExclusionRule, AuraError> {
        if value.trim().is_empty() {
            return Err(AuraError::InvalidInput(
                "An exclusion rule needs a non-empty application, domain, or project value."
                    .to_string(),
            ));
        }

        if value.len() > 160 {
            return Err(AuraError::InvalidInput(
                "An exclusion value must not exceed 160 characters.".to_string(),
            ));
        }

        let id = Uuid::new_v4().to_string();
        let now = migrations::utc_timestamp();

        self.connection
            .execute(
                "INSERT INTO exclusion_rules (id, rule_type, value, is_enabled, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 1, ?4, ?5)",
                params![id, kind.as_str(), value.trim(), now, now],
            )
            .map_err(|error| {
                AuraError::Storage(format!("Aura could not save the local exclusion rule: {error}"))
            })?;

        Ok(ExclusionRule {
            id,
            kind,
            value: value.trim().to_string(),
            is_enabled: true,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn set_exclusion_enabled(
        &self,
        exclusion_id: &str,
        input: SetExclusionEnabledInput,
    ) -> Result<ExclusionRule, AuraError> {
        let now = migrations::utc_timestamp();

        self.connection
            .execute(
                "UPDATE exclusion_rules
                 SET is_enabled = ?2, updated_at = ?3
                 WHERE id = ?1",
                params![exclusion_id, input.is_enabled as i64, now],
            )
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not update the local exclusion rule: {error}"
                ))
            })?;

        self.exclusion_rule_by_id(exclusion_id)?.ok_or_else(|| {
            AuraError::NotFound("Aura could not find that local exclusion rule.".to_string())
        })
    }

    pub fn selected_project_id(&self) -> Result<Option<String>, AuraError> {
        self.setting_value("selected_project_id")
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

    pub fn projects(&self) -> repositories::projects::ProjectRepository<'_> {
        repositories::projects::ProjectRepository::new(&self.connection)
    }

    pub fn captures(&self) -> repositories::captures::CaptureRepository<'_> {
        repositories::captures::CaptureRepository::new(&self.connection)
    }

    pub fn decisions(&self) -> repositories::claims::DecisionRepository<'_> {
        repositories::claims::DecisionRepository::new(&self.connection)
    }

    fn setting_value(&self, key: &str) -> Result<Option<String>, AuraError> {
        self.connection
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not read its local workspace setting: {error}"
                ))
            })
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

    fn exclusion_rules(&self) -> Result<Vec<ExclusionRule>, AuraError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, rule_type, value, is_enabled, created_at, updated_at
                 FROM exclusion_rules
                 ORDER BY created_at DESC",
            )
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not prepare its local exclusion rule query: {error}"
                ))
            })?;

        let rules = statement
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get::<_, String>(1)?,
                    row.get(2)?,
                    row.get::<_, i64>(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            })
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not read its local exclusion rules: {error}"
                ))
            })?
            .filter_map(|row| row.ok())
            .filter_map(
                |(id, rule_type, value, is_enabled, created_at, updated_at)| {
                    ExclusionKind::from_store(&rule_type)
                        .ok()
                        .map(|kind| ExclusionRule {
                            id,
                            kind,
                            value,
                            is_enabled: is_enabled != 0,
                            created_at,
                            updated_at,
                        })
                },
            )
            .collect();

        Ok(rules)
    }

    fn exclusion_rule_by_id(&self, exclusion_id: &str) -> Result<Option<ExclusionRule>, AuraError> {
        Ok(self
            .exclusion_rules()?
            .into_iter()
            .find(|rule| rule.id == exclusion_id))
    }

    #[cfg(test)]
    pub fn schema_version(&self) -> Result<i64, AuraError> {
        current_version(&self.connection)
    }
}

#[cfg(test)]
mod tests {
    use super::{migrations::run, LocalStore};
    use crate::domain::settings::{ExclusionKind, PrivacyMode, UpdatePrivacyPreferencesInput};
    use rusqlite::Connection;

    #[test]
    fn migration_creates_an_empty_local_workspace_with_privacy_controls() {
        let store = LocalStore::open_in_memory().expect("test database should migrate");

        assert_eq!(store.schema_version().expect("schema version"), 6);

        let preferences = store.privacy_preferences().expect("preferences");
        assert_eq!(preferences.privacy_mode.as_str(), "manual_only");
        assert_eq!(
            preferences.default_capture_retention.as_str(),
            "until_deleted"
        );
        assert!(preferences.exclusions.is_empty());

        assert!(store.projects().list_active().expect("projects").is_empty());
    }

    #[test]
    fn legacy_focused_mode_is_renamed_to_manual_only() {
        let mut connection = Connection::open_in_memory().expect("test database");
        run(&mut connection).expect("first migration run");

        connection
            .execute_batch("UPDATE settings SET value = 'focused' WHERE key = 'privacy_mode';")
            .expect("legacy fixture");

        run(&mut connection).expect("fifth migration should apply");

        let store = LocalStore::open_in_memory().expect("store");
        assert_eq!(store.schema_version().expect("schema version"), 6);
        assert_eq!(
            store
                .privacy_preferences()
                .expect("preferences")
                .privacy_mode
                .as_str(),
            "manual_only"
        );
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
        assert_eq!(applied_count, 6);
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

    #[test]
    fn privacy_preferences_roundtrip_through_atomic_updates() {
        let mut store = LocalStore::open_in_memory().expect("test database should migrate");

        let updated = store
            .update_privacy_preferences(UpdatePrivacyPreferencesInput {
                privacy_mode: crate::domain::settings::PrivacyMode::Paused,
                default_capture_retention: crate::domain::capture::CaptureRetention::ReviewIn30Days,
            })
            .expect("preferences should update");

        assert_eq!(updated.privacy_mode.as_str(), "paused");
        assert_eq!(
            updated.default_capture_retention.as_str(),
            "review_in_30_days"
        );

        let reloaded = store.privacy_preferences().expect("reloaded preferences");
        assert_eq!(reloaded, updated);
    }

    #[test]
    fn exclusion_rules_support_full_lifecycle_with_validation() {
        let store = LocalStore::open_in_memory().expect("test database should migrate");

        let rule = store
            .create_exclusion_rule(ExclusionKind::Domain, " example.com ".to_string())
            .expect("rule should save");
        assert_eq!(rule.value, "example.com");
        assert!(rule.is_enabled);

        assert!(store
            .create_exclusion_rule(ExclusionKind::Application, "  ".to_string())
            .is_err());
        assert!(store
            .create_exclusion_rule(ExclusionKind::Domain, "x".repeat(161))
            .is_err());

        let disabled = store
            .set_exclusion_enabled(
                &rule.id,
                crate::domain::settings::SetExclusionEnabledInput { is_enabled: false },
            )
            .expect("toggle should save");
        assert!(!disabled.is_enabled);

        assert!(store
            .set_exclusion_enabled(
                "missing-id",
                crate::domain::settings::SetExclusionEnabledInput { is_enabled: true }
            )
            .is_err());
    }

    #[test]
    fn only_application_domain_and_project_exclusion_kinds_are_storable() {
        let store = LocalStore::open_in_memory().expect("store should open");

        for kind in [
            ExclusionKind::Application,
            ExclusionKind::Domain,
            ExclusionKind::Project,
        ] {
            let rule = store
                .create_exclusion_rule(kind, "webcam.app".to_string())
                .expect("supported kind should save");
            assert_eq!(rule.kind, kind);
        }

        assert!(ExclusionKind::from_store("camera").is_err());
    }

    #[test]
    fn unknown_privacy_mode_value_is_rejected_on_read() {
        let _store = LocalStore::open_in_memory().expect("store should open");

        assert_eq!(
            PrivacyMode::from_store("camera")
                .expect_err("unknown mode must be rejected")
                .to_string(),
            "Aura found an unsupported local privacy mode."
        );
    }
}
