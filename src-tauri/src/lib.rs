mod application;
mod db;
mod domain;

use application::project_service::{parse_project_status, ProjectService, WorkspaceSnapshot};
use db::{repositories::projects::UpdateProject, LocalStore};
use domain::project::{AuraError, Project};
use serde::Deserialize;
use std::sync::Mutex;
use tauri::Manager;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum PrivacyMode {
    Focused,
    Paused,
}

impl PrivacyMode {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Focused => "focused",
            Self::Paused => "paused",
        }
    }

    fn from_store(value: &str) -> Result<Self, AuraError> {
        match value {
            "focused" => Ok(Self::Focused),
            "paused" => Ok(Self::Paused),
            _ => Err(AuraError::Storage(
                "Aura found an unsupported local privacy mode.".to_string(),
            )),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateProjectInput {
    name: String,
    goal: Option<String>,
    current_task: Option<String>,
    next_step: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateProjectInput {
    name: Option<String>,
    goal: Option<String>,
    status: Option<String>,
    current_task: Option<String>,
    blocker: Option<String>,
    next_step: Option<String>,
}

struct AppState {
    store: Mutex<LocalStore>,
}

fn with_store<T>(
    state: &tauri::State<'_, AppState>,
    operation: impl FnOnce(&LocalStore) -> Result<T, AuraError>,
) -> Result<T, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "Aura could not access its local workspace safely.".to_string())?;
    operation(&store).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_workspace_snapshot(state: tauri::State<'_, AppState>) -> Result<WorkspaceSnapshot, String> {
    with_store(&state, |store| {
        let snapshot = ProjectService::new(store).workspace_snapshot()?;
        PrivacyMode::from_store(&snapshot.privacy_mode)?;
        Ok(snapshot)
    })
}

#[tauri::command]
fn create_project(
    input: CreateProjectInput,
    state: tauri::State<'_, AppState>,
) -> Result<Project, String> {
    with_store(&state, |store| {
        ProjectService::new(store).create_project(
            input.name,
            input.goal,
            input.current_task,
            input.next_step,
        )
    })
}

#[tauri::command]
fn list_projects(state: tauri::State<'_, AppState>) -> Result<Vec<Project>, String> {
    with_store(&state, |store| store.projects().list_active())
}

#[tauri::command]
fn update_project(
    project_id: String,
    input: UpdateProjectInput,
    state: tauri::State<'_, AppState>,
) -> Result<Project, String> {
    with_store(&state, |store| {
        ProjectService::new(store).update_project(
            project_id,
            UpdateProject {
                name: input.name,
                goal: input.goal,
                status: parse_project_status(input.status)?,
                current_task: input.current_task,
                blocker: input.blocker,
                next_step: input.next_step,
            },
        )
    })
}

#[tauri::command]
fn archive_project(project_id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    with_store(&state, |store| {
        ProjectService::new(store).archive_project(project_id)
    })
}

#[tauri::command]
fn set_privacy_mode(mode: PrivacyMode, state: tauri::State<'_, AppState>) -> Result<(), String> {
    with_store(&state, |store| store.set_privacy_mode(mode.as_str()))
}

fn authorize_intentional_marker(
    project_id: String,
    privacy_mode: &PrivacyMode,
) -> Result<String, AuraError> {
    if matches!(privacy_mode, PrivacyMode::Paused) {
        return Err(AuraError::Privacy(
            "Intentional capture is paused. Resume it before adding context.".to_string(),
        ));
    }

    Ok(project_id)
}

#[tauri::command]
fn record_intentional_capture(
    project_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<application::project_service::CaptureMarker, String> {
    with_store(&state, |store| {
        let privacy_mode = PrivacyMode::from_store(&store.privacy_mode()?)?;
        let approved_project_id = authorize_intentional_marker(project_id, &privacy_mode)?;
        ProjectService::new(store).record_intentional_capture(approved_project_id)
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            std::fs::create_dir_all(&data_dir)?;

            let store = LocalStore::open(&data_dir.join("aura.sqlite3"))
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            app.manage(AppState {
                store: Mutex::new(store),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_workspace_snapshot,
            create_project,
            list_projects,
            update_project,
            archive_project,
            set_privacy_mode,
            record_intentional_capture
        ])
        .run(tauri::generate_context!())
        .expect("error while running Aura");
}

#[cfg(test)]
mod tests {
    use super::{authorize_intentional_marker, PrivacyMode};

    #[test]
    fn paused_privacy_mode_blocks_intentional_capture() {
        let result = authorize_intentional_marker("project-id".to_string(), &PrivacyMode::Paused);

        assert_eq!(
            result
                .expect_err("paused mode must reject capture")
                .to_string(),
            "Intentional capture is paused. Resume it before adding context."
        );
    }

    #[test]
    fn focused_privacy_mode_authorizes_a_manual_marker() {
        let project_id =
            authorize_intentional_marker("project-id".to_string(), &PrivacyMode::Focused)
                .expect("focused mode should permit intentional capture");

        assert_eq!(project_id, "project-id");
    }
}
