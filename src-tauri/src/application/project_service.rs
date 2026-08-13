use crate::db::{repositories::projects::ActivityRecord, LocalStore};
use crate::domain::project::{AuraError, Project, ProjectStatus};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    pub privacy_mode: String,
    pub selected_project: Option<Project>,
    pub projects: Vec<ProjectListItem>,
    pub activity: Vec<WorkspaceSignal>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectListItem {
    pub id: String,
    pub name: String,
    pub status: String,
    pub next_step: Option<String>,
    pub updated_at: String,
    pub is_selected: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSignal {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub time: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureMarker {
    pub project_id: String,
    pub captured_at: String,
    pub source: String,
}

pub struct ProjectService<'store> {
    store: &'store LocalStore,
}

impl<'store> ProjectService<'store> {
    pub fn new(store: &'store LocalStore) -> Self {
        Self { store }
    }

    pub fn workspace_snapshot(&self) -> Result<WorkspaceSnapshot, AuraError> {
        let projects = self.store.projects().list_active()?;
        let selected_project = self.resolve_selected_project(&projects)?;
        let selected_id = selected_project.as_ref().map(|project| project.id.as_str());
        let activity = match selected_id {
            Some(project_id) => self
                .store
                .projects()
                .activity_for_project(project_id)?
                .into_iter()
                .map(workspace_signal)
                .collect(),
            None => Vec::new(),
        };

        Ok(WorkspaceSnapshot {
            privacy_mode: self.store.privacy_mode()?,
            projects: projects
                .iter()
                .map(|project| project_list_item(project, selected_id))
                .collect(),
            selected_project,
            activity,
        })
    }

    pub fn create_project(
        &self,
        name: String,
        goal: Option<String>,
        current_task: Option<String>,
        next_step: Option<String>,
    ) -> Result<Project, AuraError> {
        let project =
            self.store
                .projects()
                .create(crate::db::repositories::projects::CreateProject {
                    name,
                    goal,
                    current_task,
                    next_step,
                })?;
        self.store.set_selected_project_id(&project.id)?;
        Ok(project)
    }

    pub fn select_project(&self, project_id: String) -> Result<Project, AuraError> {
        let project = self
            .store
            .projects()
            .find_by_id(&project_id)?
            .ok_or_else(|| {
                AuraError::NotFound(
                    "Aura cannot select a project that is not stored locally.".to_string(),
                )
            })?;

        if project.status == ProjectStatus::Archived {
            return Err(AuraError::InvalidInput(
                "Archived projects cannot be selected. Restore support is not available in Aura V0."
                    .to_string(),
            ));
        }

        self.store.set_selected_project_id(&project.id)?;
        Ok(project)
    }

    pub fn update_project(
        &self,
        project_id: String,
        input: crate::db::repositories::projects::UpdateProject,
    ) -> Result<Project, AuraError> {
        self.store.projects().update(&project_id, input)
    }

    pub fn archive_project(&self, project_id: String) -> Result<(), AuraError> {
        self.store.projects().archive(&project_id)?;

        if self.store.selected_project_id()?.as_deref() == Some(project_id.as_str()) {
            let replacement = self.store.projects().list_active()?.into_iter().next();
            match replacement {
                Some(project) => self.store.set_selected_project_id(&project.id)?,
                None => self.store.clear_selected_project_id()?,
            }
        }
        Ok(())
    }

    pub fn record_intentional_capture(
        &self,
        project_id: String,
    ) -> Result<CaptureMarker, AuraError> {
        let captured_at = self.store.projects().record_context_marker(&project_id)?;
        Ok(CaptureMarker {
            project_id,
            captured_at,
            source: "manual-context-marker".to_string(),
        })
    }

    fn resolve_selected_project(&self, projects: &[Project]) -> Result<Option<Project>, AuraError> {
        let stored_selection = self.store.selected_project_id()?;
        if let Some(selected_id) = stored_selection {
            if let Some(project) = projects.iter().find(|project| project.id == selected_id) {
                return Ok(Some(project.clone()));
            }
        }

        match projects.first() {
            Some(project) => {
                self.store.set_selected_project_id(&project.id)?;
                Ok(Some(project.clone()))
            }
            None => {
                self.store.clear_selected_project_id()?;
                Ok(None)
            }
        }
    }
}

fn project_list_item(project: &Project, selected_id: Option<&str>) -> ProjectListItem {
    ProjectListItem {
        id: project.id.clone(),
        name: project.name.clone(),
        status: project.status.display_name().to_string(),
        next_step: project.next_step.clone(),
        updated_at: project.updated_at.clone(),
        is_selected: selected_id == Some(project.id.as_str()),
    }
}

fn workspace_signal(record: ActivityRecord) -> WorkspaceSignal {
    WorkspaceSignal {
        id: record.id,
        kind: match record.kind.as_str() {
            "project" | "context" | "system" => record.kind,
            _ => "system".to_string(),
        },
        title: record.title,
        detail: record.detail,
        time: record.created_at,
    }
}

pub fn parse_project_status(value: Option<String>) -> Result<Option<ProjectStatus>, AuraError> {
    value
        .map(|value| match value.as_str() {
            "active" => Ok(ProjectStatus::Active),
            "paused" => Ok(ProjectStatus::Paused),
            "archived" => Ok(ProjectStatus::Archived),
            _ => Err(AuraError::InvalidInput(
                "Project status must be active, paused, or archived.".to_string(),
            )),
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::ProjectService;
    use crate::db::LocalStore;

    #[test]
    fn selected_project_persists_and_scopes_the_workspace_record() {
        let store = LocalStore::open_in_memory().expect("test database should migrate");
        let service = ProjectService::new(&store);
        let first = service
            .create_project(
                "Aura Desktop".to_string(),
                Some("Ship a trustworthy local-first desktop app".to_string()),
                None,
                Some("Review the project brief".to_string()),
            )
            .expect("first project should save");
        let second = service
            .create_project("Eternal Studios".to_string(), None, None, None)
            .expect("second project should save");

        service
            .record_intentional_capture(first.id.clone())
            .expect("first project marker should save");
        service
            .select_project(second.id.clone())
            .expect("second project should select");

        let snapshot = service.workspace_snapshot().expect("snapshot should load");
        assert_eq!(
            snapshot
                .selected_project
                .as_ref()
                .map(|project| project.id.as_str()),
            Some(second.id.as_str())
        );
        assert!(snapshot
            .activity
            .iter()
            .all(|signal| signal.title != "Intentional context recorded"));
        assert!(snapshot
            .activity
            .iter()
            .any(|signal| signal.title == "Project created locally"));

        let reloaded = ProjectService::new(&store)
            .workspace_snapshot()
            .expect("selected project should survive a new service");
        assert_eq!(
            reloaded
                .selected_project
                .as_ref()
                .map(|project| project.id.as_str()),
            Some(second.id.as_str())
        );
    }
}
