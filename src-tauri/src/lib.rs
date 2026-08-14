mod application;
mod db;
mod domain;

mod security;

use application::project_service::{parse_project_status, ProjectService, WorkspaceSnapshot};
use db::{repositories::projects::UpdateProject, LocalStore};
use domain::{
    capture::{CaptureClassification, CaptureKind, CaptureRecord, CaptureRetention},
    claim::{ClaimConfidence, DecisionClaim},
    project::{AuraError, Project},
    settings::{
        CreateExclusionRuleInput, ExclusionRule, PrivacyMode, PrivacyPreferences,
        SetExclusionEnabledInput, UpdatePrivacyPreferencesInput,
    },
};
use security::key_vault::{KeyVault, KeyVaultStatus};
use serde::Deserialize;
use std::sync::Mutex;
use tauri::Manager;

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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateManualCaptureInput {
    project_id: String,
    kind: String,
    label: String,
    content: String,
    classification: String,
    retention: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateDecisionInput {
    project_id: String,
    title: String,
    rationale: String,
    confidence: String,
    source_labels: Vec<String>,
}

struct AppState {
    store: Mutex<LocalStore>,
    key_vault: Mutex<KeyVault>,
}

fn with_store_mut<T>(
    state: &tauri::State<'_, AppState>,
    operation: impl FnOnce(&mut LocalStore) -> Result<T, AuraError>,
) -> Result<T, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "Aura could not access its local workspace safely.".to_string())?;
    operation(&mut store).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_workspace_snapshot(state: tauri::State<'_, AppState>) -> Result<WorkspaceSnapshot, String> {
    with_store_mut(&state, |store| {
        ProjectService::new(store).workspace_snapshot()
    })
}

#[tauri::command]
fn create_project(
    input: CreateProjectInput,
    state: tauri::State<'_, AppState>,
) -> Result<Project, String> {
    with_store_mut(&state, |store| {
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
    with_store_mut(&state, |store| store.projects().list_active())
}

#[tauri::command]
fn select_project(
    project_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Project, String> {
    with_store_mut(&state, |store| {
        ProjectService::new(store).select_project(project_id)
    })
}

#[tauri::command]
fn update_project(
    project_id: String,
    input: UpdateProjectInput,
    state: tauri::State<'_, AppState>,
) -> Result<Project, String> {
    with_store_mut(&state, |store| {
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
    with_store_mut(&state, |store| {
        ProjectService::new(store).archive_project(project_id)
    })
}

#[tauri::command]
fn set_privacy_mode(
    mode: PrivacyMode,
    state: tauri::State<'_, AppState>,
) -> Result<PrivacyPreferences, String> {
    with_store_mut(&state, |store| {
        let default_capture_retention = store.privacy_preferences()?.default_capture_retention;
        ProjectService::new(store).update_privacy_preferences(UpdatePrivacyPreferencesInput {
            privacy_mode: mode,
            default_capture_retention,
        })
    })
}

#[tauri::command]
fn get_privacy_preferences(
    state: tauri::State<'_, AppState>,
) -> Result<PrivacyPreferences, String> {
    with_store_mut(&state, |store| {
        ProjectService::new(store).privacy_preferences()
    })
}

#[tauri::command]
fn update_privacy_preferences(
    input: UpdatePrivacyPreferencesInput,
    state: tauri::State<'_, AppState>,
) -> Result<PrivacyPreferences, String> {
    with_store_mut(&state, |store| {
        ProjectService::new(store).update_privacy_preferences(input)
    })
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
    with_store_mut(&state, |store| {
        let privacy_mode = store.privacy_preferences()?.privacy_mode;
        let approved_project_id = authorize_intentional_marker(project_id, &privacy_mode)?;
        ProjectService::new(store).record_intentional_capture(approved_project_id)
    })
}

#[tauri::command]
fn create_manual_capture(
    input: CreateManualCaptureInput,
    state: tauri::State<'_, AppState>,
) -> Result<CaptureRecord, String> {
    with_store_mut(&state, |store| {
        let privacy_mode = store.privacy_preferences()?.privacy_mode;
        let project_id = authorize_intentional_marker(input.project_id, &privacy_mode)?;
        let retention = match input.retention {
            Some(value) => Some(parse_capture_retention(value)?),
            None => None,
        };
        ProjectService::new(store).create_manual_capture(
            project_id,
            parse_capture_kind(input.kind)?,
            input.label,
            input.content,
            parse_capture_classification(input.classification)?,
            retention,
        )
    })
}

#[tauri::command]
fn list_decisions(
    project_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<DecisionClaim>, String> {
    with_store_mut(&state, |store| {
        ProjectService::new(store).decisions_for_project(project_id)
    })
}

#[tauri::command]
fn create_decision(
    input: CreateDecisionInput,
    state: tauri::State<'_, AppState>,
) -> Result<DecisionClaim, String> {
    with_store_mut(&state, |store| {
        ProjectService::new(store).create_decision(
            input.project_id,
            input.title,
            input.rationale,
            parse_claim_confidence(input.confidence)?,
            input.source_labels,
        )
    })
}

#[tauri::command]
fn correct_decision(
    decision_id: String,
    input: CreateDecisionInput,
    state: tauri::State<'_, AppState>,
) -> Result<DecisionClaim, String> {
    with_store_mut(&state, |store| {
        ProjectService::new(store).correct_decision(
            input.project_id,
            decision_id,
            input.title,
            input.rationale,
            parse_claim_confidence(input.confidence)?,
            input.source_labels,
        )
    })
}

#[tauri::command]
fn create_exclusion_rule(
    input: CreateExclusionRuleInput,
    state: tauri::State<'_, AppState>,
) -> Result<ExclusionRule, String> {
    with_store_mut(&state, |store| {
        ProjectService::new(store).create_exclusion_rule(input)
    })
}

#[tauri::command]
fn set_exclusion_enabled(
    exclusion_id: String,
    input: SetExclusionEnabledInput,
    state: tauri::State<'_, AppState>,
) -> Result<ExclusionRule, String> {
    with_store_mut(&state, |store| {
        ProjectService::new(store).set_exclusion_enabled(&exclusion_id, input)
    })
}

fn parse_claim_confidence(value: String) -> Result<ClaimConfidence, AuraError> {
    ClaimConfidence::from_store(&value).map_err(|_| {
        AuraError::InvalidInput(
            "Choose low, medium, or high confidence before saving a decision.".to_string(),
        )
    })
}

fn parse_capture_kind(value: String) -> Result<CaptureKind, AuraError> {
    CaptureKind::from_store(&value).map_err(|_| {
        AuraError::InvalidInput("Choose a supported manual capture type before saving.".to_string())
    })
}

fn parse_capture_classification(value: String) -> Result<CaptureClassification, AuraError> {
    CaptureClassification::from_store(&value).map_err(|_| {
        AuraError::InvalidInput(
            "Choose a supported capture classification before saving.".to_string(),
        )
    })
}

fn parse_capture_retention(value: String) -> Result<CaptureRetention, AuraError> {
    CaptureRetention::from_store(&value).map_err(|_| {
        AuraError::InvalidInput("Choose a supported retention setting before saving.".to_string())
    })
}

// SEC-001: diagnostic verification commands. None of them expose raw key
// material or the wrapped blob; they exist only to prove the envelope
// boundary through the typed command layer.
#[tauri::command]
fn seal_secret(state: tauri::State<'_, AppState>, secret: String) -> Result<Vec<u8>, String> {
    let key_vault = state
        .key_vault
        .lock()
        .map_err(|_| "Aura could not access its key vault safely.".to_string())?;
    let sealed = key_vault
        .seal(secret.as_bytes())
        .map(|value| KeyVault::encode_sealed(&value));
    sealed.map_err(|error| error.to_string())
}

#[tauri::command]
fn open_secret(state: tauri::State<'_, AppState>, sealed_bytes: Vec<u8>) -> Result<String, String> {
    let key_vault = state
        .key_vault
        .lock()
        .map_err(|_| "Aura could not access its key vault safely.".to_string())?;
    let sealed = KeyVault::decode_sealed(&sealed_bytes).map_err(|error| error.to_string())?;
    let plaintext = key_vault.open(&sealed).map_err(|error| error.to_string())?;
    String::from_utf8(plaintext).map_err(|_| "sealed secret is not valid UTF-8".to_string())
}

#[tauri::command]
fn key_vault_status(state: tauri::State<'_, AppState>) -> Result<KeyVaultStatus, String> {
    let key_vault = state
        .key_vault
        .lock()
        .map_err(|_| "Aura could not access its key vault safely.".to_string())?;
    Ok(key_vault.status())
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
            let key_vault = KeyVault::new(&data_dir)
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            app.manage(AppState {
                store: Mutex::new(store),
                key_vault: Mutex::new(key_vault),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            seal_secret,
            open_secret,
            key_vault_status,
            get_workspace_snapshot,
            create_project,
            list_projects,
            select_project,
            update_project,
            archive_project,
            set_privacy_mode,
            get_privacy_preferences,
            update_privacy_preferences,
            record_intentional_capture,
            create_manual_capture,
            list_decisions,
            create_decision,
            correct_decision,
            create_exclusion_rule,
            set_exclusion_enabled
        ])
        .run(tauri::generate_context!())
        .expect("error while running Aura");
}

#[cfg(test)]
mod tests {
    use super::authorize_intentional_marker;
    use crate::domain::settings::PrivacyMode;

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
    fn manual_only_privacy_mode_authorizes_a_manual_marker() {
        let project_id =
            authorize_intentional_marker("project-id".to_string(), &PrivacyMode::ManualOnly)
                .expect("manual-only mode should permit intentional capture");

        assert_eq!(project_id, "project-id");
    }
}
