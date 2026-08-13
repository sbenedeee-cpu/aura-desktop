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

        let capture = CaptureRecord {
            id: Uuid::new_v4().to_string(),
            project_id: input.project_id,
            kind: input.kind,
            label,
            content,
            classification: input.classification,
            retention: input.retention,
            created_at: utc_timestamp(),
        };

        let transaction = self.connection.unchecked_transaction().map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not begin local capture storage: {error}"
            ))
        })?;
        transaction
            .execute(
                "INSERT INTO captures (
                    id, project_id, kind, label, content, classification, retention, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    &capture.id,
                    &capture.project_id,
                    capture.kind.as_str(),
                    &capture.label,
                    &capture.content,
                    capture.classification.as_str(),
                    capture.retention.as_str(),
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
}
