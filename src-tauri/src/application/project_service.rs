use crate::db::{repositories::projects::ActivityRecord, LocalStore};
use crate::domain::project::{AuraError, Project, ProjectStatus};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    pub active_project: String,
    pub continuity_note: String,
    pub next_step: String,
    pub privacy_mode: String,
    pub projects: Vec<ProjectListItem>,
    pub signals: Vec<WorkspaceSignal>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectListItem {
    pub id: String,
    pub name: String,
    pub status: String,
    pub signal: String,
    pub updated_at: String,
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
        let project_items = projects.iter().map(project_list_item).collect::<Vec<_>>();
        let active_project = projects
            .first()
            .map(|project| project.name.clone())
            .unwrap_or_else(|| "No local project selected".to_string());
        let privacy_mode = self.store.privacy_mode()?;
        let signals = self
            .store
            .projects()
            .activity()?
            .into_iter()
            .map(workspace_signal)
            .collect();

        Ok(WorkspaceSnapshot {
            active_project,
            continuity_note: "Aura uses only the local project workspace. No desktop capture, cloud sync, or AI provider is enabled.".to_string(),
            next_step: if projects.is_empty() {
                "Create a local project to begin an intentional continuity record.".to_string()
            } else {
                "Select a project and add an intentional context marker when you choose.".to_string()
            },
            privacy_mode,
            projects: project_items,
            signals,
        })
    }

    pub fn create_project(
        &self,
        name: String,
        goal: Option<String>,
        current_task: Option<String>,
        next_step: Option<String>,
    ) -> Result<Project, AuraError> {
        self.store
            .projects()
            .create(crate::db::repositories::projects::CreateProject {
                name,
                goal,
                current_task,
                next_step,
            })
    }

    pub fn update_project(
        &self,
        project_id: String,
        input: crate::db::repositories::projects::UpdateProject,
    ) -> Result<Project, AuraError> {
        self.store.projects().update(&project_id, input)
    }

    pub fn archive_project(&self, project_id: String) -> Result<(), AuraError> {
        self.store.projects().archive(&project_id)
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
}

fn project_list_item(project: &Project) -> ProjectListItem {
    let signal = project
        .current_task
        .as_ref()
        .or(project.next_step.as_ref())
        .or(project.goal.as_ref())
        .map_or_else(
            || "No intentional context recorded yet".to_string(),
            std::clone::Clone::clone,
        );

    ProjectListItem {
        id: project.id.clone(),
        name: project.name.clone(),
        status: project.status.display_name().to_string(),
        signal,
        updated_at: project.updated_at.clone(),
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
