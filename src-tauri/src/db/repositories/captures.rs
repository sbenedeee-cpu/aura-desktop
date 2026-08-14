use crate::db::migrations::utc_timestamp;
use crate::domain::{
    capture::{CaptureClassification, CaptureKind, CaptureRecord, CaptureRetention},
    project::AuraError,
};
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct CreateCapture {
    pub project_id: String,
    pub kind: CaptureKind,
    pub label: String,
    pub content: String,
    pub classification: CaptureClassification,
    pub retention: CaptureRetention,
}

pub struct CaptureRepository<'connection> {
    connection: &'connection Connection,
}

impl<'connection> CaptureRepository<'connection> {
    pub fn new(connection: &'connection Connection) -> Self {
        Self { connection }
    }

    pub fn create(&self, input: CreateCapture) -> Result<CaptureRecord, AuraError> {
        self.ensure_active_project(&input.project_id)?;
        let label = required_text("Capture label", input.label, 120)?;
        let content = required_text("Capture content", input.content, 20_000)?;
        if input.kind == CaptureKind::Url && !is_safe_url(&content) {
            return Err(AuraError::InvalidInput(
                "A URL capture must begin with http:// or https://.".to_string(),
            ));
        }

        let now = utc_timestamp();
        let capture = CaptureRecord {
            id: Uuid::new_v4().to_string(),
            project_id: input.project_id,
            kind: input.kind,
            label,
            content,
            classification: input.classification,
            retention: input.retention,
            created_at: now.clone(),
        };

        let transaction = self.connection.unchecked_transaction().map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not begin local capture storage: {error}"
            ))
        })?;
        transaction
            .execute(
                "INSERT INTO captures (
                    id, project_id, kind, label, content, classification, retention, created_at,
                    lifecycle_state, lifecycle_updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    &capture.id,
                    &capture.project_id,
                    capture.kind.as_str(),
                    &capture.label,
                    &capture.content,
                    capture.classification.as_str(),
                    capture.retention.as_str(),
                    &capture.created_at,
                    "active",
                    &capture.created_at,
                ],
            )
            .map_err(|error| {
                AuraError::Storage(format!("Aura could not save this local capture: {error}"))
            })?;

        transaction
            .execute(
                "INSERT INTO activity_records (id, project_id, kind, title, detail, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    Uuid::new_v4().to_string(),
                    &capture.project_id,
                    "capture",
                    "Manual capture saved locally",
                    format!(
                        "{} capture “{}” was saved with {} classification and {} retention.",
                        capture_kind_label(capture.kind),
                        capture.label,
                        capture.classification.as_str(),
                        capture.retention.as_str()
                    ),
                    utc_timestamp()
                ],
            )
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not record local capture activity: {error}"
                ))
            })?;
        transaction.commit().map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not finalize this local capture: {error}"
            ))
        })?;

        Ok(capture)
    }

    fn ensure_active_project(&self, project_id: &str) -> Result<(), AuraError> {
        let status: Option<String> = self
            .connection
            .query_row(
                "SELECT status FROM projects WHERE id = ?1",
                [project_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not validate the capture destination: {error}"
                ))
            })?;

        match status.as_deref() {
            Some("active") | Some("paused") => Ok(()),
            Some(status) => Err(AuraError::InvalidInput(format!(
                "Aura cannot save a capture to a {status} project."
            ))),
            None => Err(AuraError::NotFound(
                "Aura cannot save a capture to a project that is not stored locally.".to_string(),
            )),
        }
    }
}

fn required_text(field: &str, value: String, limit: usize) -> Result<String, AuraError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AuraError::InvalidInput(format!(
            "{field} is required before Aura can save it locally."
        )));
    }
    if value.len() > limit {
        return Err(AuraError::InvalidInput(format!(
            "{field} must be {limit} characters or fewer."
        )));
    }
    Ok(value.to_string())
}

fn is_safe_url(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    normalized.starts_with("https://") || normalized.starts_with("http://")
}

/// EXP-003: raw capture row used by the retention sweep, including the
/// lifecycle columns the sweep owns. Rendered domain types are built from
/// this row by the application service, never by the renderer reading
/// lifecycle state directly.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct LifecycleCapture {
    pub id: String,
    pub project_id: String,
    /// Source kind of the capture, kept for the review rail and future
    /// audit reporting even when no caller reads it yet.
    pub kind: String,
    pub label: String,
    /// Raw capture content, kept for the review rail and future audit
    /// reporting even when no caller reads it yet.
    pub content: String,
    pub classification: String,
    pub retention: String,
    pub created_at: String,
    pub lifecycle_state: String,
    pub lifecycle_updated_at: String,
}

impl<'connection> CaptureRepository<'connection> {
    /// Every capture that could be touched by the retention sweep.
    /// `deleted` rows are still returned so the sweep result counts the full
    /// local footprint honestly; the sweep policy itself ignores them.
    pub fn captures_for_retention_sweep(&self) -> Result<Vec<LifecycleCapture>, AuraError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, project_id, kind, label, content, classification, retention,
                        created_at, lifecycle_state, lifecycle_updated_at
                 FROM captures
                 ORDER BY created_at ASC",
            )
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not prepare its retention sweep query: {error}"
                ))
            })?;

        let captures = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                ))
            })
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not read captures for the retention sweep: {error}"
                ))
            })?
            .filter_map(|row| row.ok())
            .map(
                |(
                    id,
                    project_id,
                    kind,
                    label,
                    content,
                    classification,
                    retention,
                    created_at,
                    lifecycle_state,
                    lifecycle_updated_at,
                )| {
                    LifecycleCapture {
                        id,
                        project_id,
                        kind,
                        label,
                        content,
                        classification,
                        retention,
                        created_at,
                        lifecycle_state,
                        lifecycle_updated_at,
                    }
                },
            )
            .collect();

        Ok(captures)
    }

    fn transition_lifecycle(
        &self,
        capture_id: &str,
        expected_state: &str,
        next_state: &str,
        action_label: &str,
    ) -> Result<LifecycleCapture, AuraError> {
        let now = utc_timestamp();
        let updated = self
            .connection
            .execute(
                "UPDATE captures
                 SET lifecycle_state = ?2, lifecycle_updated_at = ?3
                 WHERE id = ?1 AND lifecycle_state = ?4",
                params![capture_id, next_state, now, expected_state],
            )
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not apply the capture lifecycle transition: {error}"
                ))
            })?;
        if updated == 0 {
            return Err(AuraError::InvalidInput(format!(
                "{action_label} is not allowed right now: Aura could not find an active candidate with that identifier, or its lifecycle state changed before the request."
            )));
        }
        self.lifecycle_capture_by_id(capture_id)?.ok_or_else(|| {
            AuraError::NotFound(
                "Aura could not reload the capture after its lifecycle transition.".to_string(),
            )
        })
    }

    /// Marks aged captures surfaced by the last sweep. The sweep owns the
    /// transition; the renderer only confirms what the sweep produced.
    pub fn age_capture(&self, capture_id: &str) -> Result<LifecycleCapture, AuraError> {
        self.transition_lifecycle(capture_id, "active", "aged", "Ageing this capture")
    }

    /// User keeps the capture: the 30-day review clock restarts from now.
    pub fn keep_capture(&self, capture_id: &str) -> Result<LifecycleCapture, AuraError> {
        let updated = self
            .connection
            .execute(
                "UPDATE captures
                 SET lifecycle_state = 'active', lifecycle_updated_at = ?2
                 WHERE id = ?1 AND lifecycle_state = 'aged'",
                params![capture_id, utc_timestamp()],
            )
            .map_err(|error| {
                AuraError::Storage(format!("Aura could not keep this capture: {error}"))
            })?;
        if updated == 0 {
            return Err(AuraError::InvalidInput(
                "Aura could not keep that capture: it is not in the review state right now."
                    .to_string(),
            ));
        }
        self.lifecycle_capture_by_id(capture_id)?.ok_or_else(|| {
            AuraError::NotFound("Aura could not reload the capture after keeping it.".to_string())
        })
    }

    /// User deletes the capture: deliberate, audited, irreversible.
    pub fn delete_capture(&self, capture_id: &str) -> Result<LifecycleCapture, AuraError> {
        self.transition_lifecycle(capture_id, "active", "deleted", "Deleting this capture")
    }

    pub fn lifecycle_capture_by_id(
        &self,
        capture_id: &str,
    ) -> Result<Option<LifecycleCapture>, AuraError> {
        self.connection
            .query_row(
                "SELECT id, project_id, kind, label, content, classification, retention,
                        created_at, lifecycle_state, lifecycle_updated_at
                 FROM captures
                 WHERE id = ?1",
                params![capture_id],
                |row| {
                    Ok(LifecycleCapture {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        kind: row.get(2)?,
                        label: row.get(3)?,
                        content: row.get(4)?,
                        classification: row.get(5)?,
                        retention: row.get(6)?,
                        created_at: row.get(7)?,
                        lifecycle_state: row.get(8)?,
                        lifecycle_updated_at: row.get(9)?,
                    })
                },
            )
            .optional()
            .map_err(|error| {
                AuraError::Storage(format!("Aura could not load this capture: {error}"))
            })
    }
}

fn capture_kind_label(kind: CaptureKind) -> &'static str {
    match kind {
        CaptureKind::ManualNote => "Manual note",
        CaptureKind::PastedText => "Pasted text",
        CaptureKind::Url => "URL",
    }
}

#[cfg(test)]
mod tests {
    use super::CreateCapture;
    use crate::{
        db::{repositories::projects::CreateProject, LocalStore},
        domain::capture::{CaptureClassification, CaptureKind, CaptureRetention},
    };

    #[test]
    fn captures_are_durable_and_scoped_to_their_selected_project() {
        let store = LocalStore::open_in_memory().expect("test database should migrate");
        let first = store
            .projects()
            .create(CreateProject {
                name: "Aura".to_string(),
                goal: None,
                current_task: None,
                next_step: None,
            })
            .expect("first project");
        let second = store
            .projects()
            .create(CreateProject {
                name: "Ascend".to_string(),
                goal: None,
                current_task: None,
                next_step: None,
            })
            .expect("second project");

        let capture = store
            .captures()
            .create(CreateCapture {
                project_id: first.id.clone(),
                kind: CaptureKind::ManualNote,
                label: "Continuity note".to_string(),
                content: "Keep the next slice local-first.".to_string(),
                classification: CaptureClassification::Sensitive,
                retention: CaptureRetention::ReviewIn30Days,
            })
            .expect("capture should save");

        assert_eq!(capture.project_id, first.id);
        let first_capture_count: i64 = store
            .connection
            .query_row(
                "SELECT COUNT(*) FROM captures WHERE project_id = ?1",
                [first.id.as_str()],
                |row| row.get(0),
            )
            .expect("first project capture count");
        let second_capture_count: i64 = store
            .connection
            .query_row(
                "SELECT COUNT(*) FROM captures WHERE project_id = ?1",
                [second.id.as_str()],
                |row| row.get(0),
            )
            .expect("second project capture count");

        assert_eq!(first_capture_count, 1);
        assert_eq!(second_capture_count, 0);
    }

    #[test]
    fn capture_and_activity_write_rollback_together_when_activity_storage_fails() {
        let store = LocalStore::open_in_memory().expect("test database should migrate");
        let project = store
            .projects()
            .create(CreateProject {
                name: "Aura".to_string(),
                goal: None,
                current_task: None,
                next_step: None,
            })
            .expect("project");
        store
            .connection
            .execute("DROP TABLE activity_records", [])
            .expect("simulate unavailable local activity storage");

        let result = store.captures().create(CreateCapture {
            project_id: project.id.clone(),
            kind: CaptureKind::ManualNote,
            label: "Must not persist partially".to_string(),
            content: "The paired activity write should make this transaction fail.".to_string(),
            classification: CaptureClassification::Standard,
            retention: CaptureRetention::UntilDeleted,
        });

        assert!(result.is_err());
        let capture_count: i64 = store
            .connection
            .query_row(
                "SELECT COUNT(*) FROM captures WHERE project_id = ?1",
                [project.id.as_str()],
                |row| row.get(0),
            )
            .expect("capture count after rolled-back write");
        assert_eq!(capture_count, 0);
    }

    #[test]
    fn captures_start_active_and_lifecycle_transitions_are_tracked() {
        let store = LocalStore::open_in_memory().expect("test database should migrate");
        let project = store
            .projects()
            .create(CreateProject {
                name: "Aura".to_string(),
                goal: None,
                current_task: None,
                next_step: None,
            })
            .expect("project");

        let capture = store
            .captures()
            .create(CreateCapture {
                project_id: project.id.clone(),
                kind: CaptureKind::ManualNote,
                label: "Aging context".to_string(),
                content: "Standard retention context used to verify the lifecycle sweep."
                    .to_string(),
                classification: CaptureClassification::Standard,
                retention: CaptureRetention::ReviewIn30Days,
            })
            .expect("capture");

        let loaded = store
            .captures()
            .lifecycle_capture_by_id(&capture.id)
            .expect("load")
            .expect("exists");
        assert_eq!(loaded.lifecycle_state, "active");

        let aged = store.captures().age_capture(&capture.id).expect("age");
        assert_eq!(aged.lifecycle_state, "aged");

        let kept = store.captures().keep_capture(&capture.id).expect("keep");
        assert_eq!(kept.lifecycle_state, "active");
    }

    #[test]
    fn keep_requires_the_capture_to_be_in_the_aged_review_state() {
        let store = LocalStore::open_in_memory().expect("test database should migrate");
        let project = store
            .projects()
            .create(CreateProject {
                name: "Aura".to_string(),
                goal: None,
                current_task: None,
                next_step: None,
            })
            .expect("project");
        let capture = store
            .captures()
            .create(CreateCapture {
                project_id: project.id,
                kind: CaptureKind::ManualNote,
                label: "Fresh capture".to_string(),
                content: "Not yet eligible for the review queue.".to_string(),
                classification: CaptureClassification::Standard,
                retention: CaptureRetention::ReviewIn30Days,
            })
            .expect("capture");

        let result = store.captures().keep_capture(&capture.id);
        assert!(result.is_err());
    }

    #[test]
    fn delete_is_deliberate_and_requires_an_active_candidate() {
        let store = LocalStore::open_in_memory().expect("test database should migrate");
        let project = store
            .projects()
            .create(CreateProject {
                name: "Aura".to_string(),
                goal: None,
                current_task: None,
                next_step: None,
            })
            .expect("project");
        let capture = store
            .captures()
            .create(CreateCapture {
                project_id: project.id.clone(),
                kind: CaptureKind::PastedText,
                label: "Expired paste".to_string(),
                content: "User chose to delete this deliberately.".to_string(),
                classification: CaptureClassification::Standard,
                retention: CaptureRetention::ReviewIn30Days,
            })
            .expect("capture");

        let deleted = store
            .captures()
            .delete_capture(&capture.id)
            .expect("delete");
        assert_eq!(deleted.lifecycle_state, "deleted");

        let second_attempt = store.captures().delete_capture(&capture.id);
        assert!(second_attempt.is_err());
    }

    #[test]
    fn sweep_query_returns_every_capture_with_its_lifecycle_state() {
        let store = LocalStore::open_in_memory().expect("test database should migrate");
        let project = store
            .projects()
            .create(CreateProject {
                name: "Aura".to_string(),
                goal: None,
                current_task: None,
                next_step: None,
            })
            .expect("project");
        let standard = store
            .captures()
            .create(CreateCapture {
                project_id: project.id.clone(),
                kind: CaptureKind::ManualNote,
                label: "Standard note".to_string(),
                content: "Reviewable retention context.".to_string(),
                classification: CaptureClassification::Standard,
                retention: CaptureRetention::ReviewIn30Days,
            })
            .expect("standard");
        let sensitive = store
            .captures()
            .create(CreateCapture {
                project_id: project.id.clone(),
                kind: CaptureKind::ManualNote,
                label: "Sensitive note".to_string(),
                content: "Sensitive context that the sweep must protect.".to_string(),
                classification: CaptureClassification::Sensitive,
                retention: CaptureRetention::ReviewIn30Days,
            })
            .expect("sensitive");

        store.captures().age_capture(&standard.id).expect("age");

        let sweep_rows = store
            .captures()
            .captures_for_retention_sweep()
            .expect("sweep query");
        assert_eq!(sweep_rows.len(), 2);

        let standard_row = sweep_rows
            .iter()
            .find(|row| row.id == standard.id)
            .expect("standard row");
        assert_eq!(standard_row.lifecycle_state, "aged");
        assert_eq!(standard_row.classification, "standard");
        assert_eq!(
            sweep_rows
                .iter()
                .find(|row| row.id == sensitive.id)
                .expect("sensitive row")
                .classification,
            "sensitive"
        );
    }
}
