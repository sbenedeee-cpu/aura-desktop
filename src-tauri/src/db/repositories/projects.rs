use crate::db::migrations::utc_timestamp;
use crate::domain::project::{AuraError, Project, ProjectStatus};
use rusqlite::{params, Connection, OptionalExtension, Row};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct CreateProject {
    pub name: String,
    pub goal: Option<String>,
    pub current_task: Option<String>,
    pub next_step: Option<String>,
}

#[derive(Clone, Debug)]
pub struct UpdateProject {
    pub name: Option<String>,
    pub goal: Option<String>,
    pub status: Option<ProjectStatus>,
    pub current_task: Option<String>,
    pub blocker: Option<String>,
    pub next_step: Option<String>,
}

pub struct ProjectRepository<'connection> {
    connection: &'connection Connection,
}

impl<'connection> ProjectRepository<'connection> {
    pub fn new(connection: &'connection Connection) -> Self {
        Self { connection }
    }

    pub fn create(&self, input: CreateProject) -> Result<Project, AuraError> {
        let name = required_text("Project name", input.name)?;
        let now = utc_timestamp();
        let id = Uuid::new_v4().to_string();
        let goal = optional_text(input.goal);
        let current_task = optional_text(input.current_task);
        let next_step = optional_text(input.next_step);

        self.connection
            .execute(
                "INSERT INTO projects (
                    id, name, goal, status, current_task, blocker, next_step, created_at, updated_at, archived_at
                 ) VALUES (?1, ?2, ?3, 'active', ?4, NULL, ?5, ?6, ?6, NULL)",
                params![id, name, goal, current_task, next_step, now],
            )
            .map_err(|error| {
                AuraError::Storage(format!("Aura could not save the new project locally: {error}"))
            })?;

        self.record_activity(
            Some(&id),
            "project",
            "Project created locally",
            &format!("{name} is now stored in Aura’s local workspace."),
        )?;

        self.find_by_id(&id)?.ok_or_else(|| {
            AuraError::Storage("Aura could not read the project it just saved locally.".to_string())
        })
    }

    pub fn list_active(&self) -> Result<Vec<Project>, AuraError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, name, goal, status, current_task, blocker, next_step, created_at, updated_at, archived_at
                 FROM projects
                 WHERE status != 'archived'
                 ORDER BY updated_at DESC, name COLLATE NOCASE ASC",
            )
            .map_err(|error| {
                AuraError::Storage(format!("Aura could not prepare local project data: {error}"))
            })?;

        let rows = statement.query_map([], project_from_row).map_err(|error| {
            AuraError::Storage(format!("Aura could not read local projects: {error}"))
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(|error| {
            AuraError::Storage(format!("Aura could not decode local projects: {error}"))
        })
    }

    pub fn find_by_id(&self, id: &str) -> Result<Option<Project>, AuraError> {
        self.connection
            .query_row(
                "SELECT id, name, goal, status, current_task, blocker, next_step, created_at, updated_at, archived_at
                 FROM projects WHERE id = ?1",
                [id],
                project_from_row,
            )
            .optional()
            .map_err(|error| AuraError::Storage(format!("Aura could not read the local project: {error}")))
    }

    pub fn update(&self, id: &str, input: UpdateProject) -> Result<Project, AuraError> {
        let existing = self.find_by_id(id)?.ok_or_else(|| {
            AuraError::NotFound(
                "Aura cannot update a project that is not stored locally.".to_string(),
            )
        })?;
        if existing.status == ProjectStatus::Archived {
            return Err(AuraError::InvalidInput(
                "Archived projects cannot be changed. Restore support is not available in Aura V0."
                    .to_string(),
            ));
        }

        let name = match input.name {
            Some(value) => required_text("Project name", value)?,
            None => existing.name,
        };
        let goal = input
            .goal
            .map_or(existing.goal, |value| optional_text(Some(value)));
        let current_task = input
            .current_task
            .map_or(existing.current_task, |value| optional_text(Some(value)));
        let blocker = input
            .blocker
            .map_or(existing.blocker, |value| optional_text(Some(value)));
        let next_step = input
            .next_step
            .map_or(existing.next_step, |value| optional_text(Some(value)));
        let status = input.status.unwrap_or(existing.status);
        let now = utc_timestamp();

        self.connection
            .execute(
                "UPDATE projects
                 SET name = ?2,
                     goal = ?3,
                     status = ?4,
                     current_task = ?5,
                     blocker = ?6,
                     next_step = ?7,
                     updated_at = ?8
                 WHERE id = ?1",
                params![
                    id,
                    name,
                    goal,
                    status.as_str(),
                    current_task,
                    blocker,
                    next_step,
                    now
                ],
            )
            .map_err(|error| {
                AuraError::Storage(format!("Aura could not update the local project: {error}"))
            })?;

        self.record_activity(Some(id), "project", "Project updated locally", &name)?;
        self.find_by_id(id)?.ok_or_else(|| {
            AuraError::Storage(
                "Aura could not read the project it just updated locally.".to_string(),
            )
        })
    }

    pub fn archive(&self, id: &str) -> Result<(), AuraError> {
        let project = self.find_by_id(id)?.ok_or_else(|| {
            AuraError::NotFound(
                "Aura cannot archive a project that is not stored locally.".to_string(),
            )
        })?;
        if project.status == ProjectStatus::Archived {
            return Ok(());
        }

        let now = utc_timestamp();
        self.connection
            .execute(
                "UPDATE projects
                 SET status = 'archived', archived_at = ?2, updated_at = ?2
                 WHERE id = ?1",
                params![id, now],
            )
            .map_err(|error| {
                AuraError::Storage(format!("Aura could not archive the local project: {error}"))
            })?;
        self.record_activity(
            Some(id),
            "project",
            "Project archived locally",
            &project.name,
        )
    }

    pub fn record_context_marker(&self, project_id: &str) -> Result<String, AuraError> {
        let project = self.find_by_id(project_id)?.ok_or_else(|| {
            AuraError::NotFound(
                "Aura cannot add context to a project that is not stored locally.".to_string(),
            )
        })?;
        if project.status == ProjectStatus::Archived {
            return Err(AuraError::InvalidInput(
                "Aura cannot add context to an archived project.".to_string(),
            ));
        }

        let now = utc_timestamp();
        self.connection
            .execute(
                "INSERT INTO context_markers (id, project_id, source, created_at)
                 VALUES (?1, ?2, 'manual-context-marker', ?3)",
                params![Uuid::new_v4().to_string(), project_id, now],
            )
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not save the local context marker: {error}"
                ))
            })?;
        self.record_activity(
            Some(project_id),
            "context",
            "Intentional context recorded",
            "A local manual context marker was added. Aura did not capture desktop content.",
        )?;
        Ok(now)
    }

    pub fn activity_for_project(&self, project_id: &str) -> Result<Vec<ActivityRecord>, AuraError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, kind, title, detail, created_at
                 FROM activity_records
                 WHERE project_id = ?1
                 ORDER BY created_at DESC, id DESC
                 LIMIT 6",
            )
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not prepare the project local record: {error}"
                ))
            })?;
        let rows = statement
            .query_map([project_id], activity_record_from_row)
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not read the project local record: {error}"
                ))
            })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(|error| {
            AuraError::Storage(format!(
                "Aura could not decode the project local record: {error}"
            ))
        })
    }

    fn record_activity(
        &self,
        project_id: Option<&str>,
        kind: &str,
        title: &str,
        detail: &str,
    ) -> Result<(), AuraError> {
        self.connection
            .execute(
                "INSERT INTO activity_records (id, project_id, kind, title, detail, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    Uuid::new_v4().to_string(),
                    project_id,
                    kind,
                    title,
                    detail,
                    utc_timestamp()
                ],
            )
            .map_err(|error| {
                AuraError::Storage(format!("Aura could not record local activity: {error}"))
            })?;
        Ok(())
    }

    #[cfg(test)]
    pub fn marker_count_for_project(&self, project_id: &str) -> Result<i64, AuraError> {
        self.connection
            .query_row(
                "SELECT COUNT(*) FROM context_markers WHERE project_id = ?1",
                [project_id],
                |row| row.get(0),
            )
            .map_err(|error| {
                AuraError::Storage(format!(
                    "Aura could not read local context markers: {error}"
                ))
            })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityRecord {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub created_at: String,
}

fn activity_record_from_row(row: &Row<'_>) -> rusqlite::Result<ActivityRecord> {
    Ok(ActivityRecord {
        id: row.get(0)?,
        kind: row.get(1)?,
        title: row.get(2)?,
        detail: row.get(3)?,
        created_at: row.get(4)?,
    })
}

fn project_from_row(row: &Row<'_>) -> rusqlite::Result<Project> {
    let status: String = row.get(3)?;
    ProjectStatus::from_store(&status).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(error))
    })?;

    Ok(Project {
        id: row.get(0)?,
        name: row.get(1)?,
        goal: row.get(2)?,
        status: ProjectStatus::from_store(&status).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                3,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        current_task: row.get(4)?,
        blocker: row.get(5)?,
        next_step: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        archived_at: row.get(9)?,
    })
}

fn required_text(field_name: &str, value: String) -> Result<String, AuraError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AuraError::InvalidInput(format!(
            "{field_name} is required before Aura can save it locally."
        )));
    }
    if value.len() > 120 {
        return Err(AuraError::InvalidInput(format!(
            "{field_name} must be 120 characters or fewer."
        )));
    }
    Ok(value.to_string())
}

fn optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::CreateProject;
    use crate::db::LocalStore;

    #[test]
    fn context_markers_remain_scoped_to_their_own_project() {
        let store = LocalStore::open_in_memory().expect("test database should migrate");
        let repository = store.projects();
        let first_project = repository
            .create(CreateProject {
                name: "Aura Desktop".to_string(),
                goal: None,
                current_task: None,
                next_step: None,
            })
            .expect("first project should save");
        let second_project = repository
            .create(CreateProject {
                name: "Ascend".to_string(),
                goal: None,
                current_task: None,
                next_step: None,
            })
            .expect("second project should save");

        repository
            .record_context_marker(&first_project.id)
            .expect("first project marker should save");

        assert_eq!(
            repository
                .marker_count_for_project(&first_project.id)
                .expect("first project marker count"),
            1
        );
        assert_eq!(
            repository
                .marker_count_for_project(&second_project.id)
                .expect("second project marker count"),
            0
        );
    }
}
