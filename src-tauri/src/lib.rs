mod application;
mod cortex;
mod db;
mod domain;

mod security;

use application::export_service::{ExportEnvelope, ExportService};
use application::project_service::{parse_project_status, ProjectService, WorkspaceSnapshot};
use application::retention_service::RetentionService;
use cortex::brain::{self, BrainContext};
use cortex::settings::store::{SettingField, SettingsStore};
use cortex::voice_pipeline::{self, TranscriptionRequest, TranscriptionResult};
use db::{repositories::projects::UpdateProject, LocalStore};
use domain::export::{ExportEvent, ExportManifest, ExportRecordCounts, PASSPHRASE_SEALING};
use domain::{
    capture::{CaptureClassification, CaptureKind, CaptureRecord, CaptureRetention},
    claim::{ClaimConfidence, DecisionClaim},
    project::{AuraError, Project},
    retention::{RetentionSweepResult, ReviewableCapture},
    settings::{
        CreateExclusionRuleInput, ExclusionRule, PrivacyMode, PrivacyPreferences,
        SetExclusionEnabledInput, UpdatePrivacyPreferencesInput,
    },
};
use security::key_vault::{KeyVault, KeyVaultStatus};
use security::passphrase::Passphrase;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

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
    settings_store: Mutex<SettingsStore>,
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

// EXP-003: retention sweep and context-ageing commands. Every transition is
// human-initiated or human-confirmed; the sweep itself never deletes
// anything. Sensitive captures and `until_deleted` captures are never aged
// automatically.
#[tauri::command]
fn run_retention_sweep(state: tauri::State<'_, AppState>) -> Result<RetentionSweepResult, String> {
    with_store_mut(&state, |store| {
        let captures = store.captures().captures_for_retention_sweep()?;
        let service = RetentionService::new(chrono::Utc::now());
        let (reviewable, result) = service.classify_pass(&captures)?;
        for capture in reviewable {
            store.captures().age_capture(&capture.id)?;
        }
        Ok(result)
    })
}

#[tauri::command]
fn list_reviewable_captures(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ReviewableCapture>, String> {
    with_store_mut(&state, |store| {
        let captures = store.captures().captures_for_retention_sweep()?;
        let service = RetentionService::new(chrono::Utc::now());
        let (reviewable, _result) = service.classify_pass(&captures)?;
        Ok(reviewable)
    })
}

#[tauri::command]
fn keep_capture(
    capture_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<ReviewableCapture, String> {
    with_store_mut(&state, |store| {
        let capture = store.captures().keep_capture(&capture_id)?;
        Ok(reviewable_from_lifecycle(&capture))
    })
}

#[tauri::command]
fn expire_capture(
    capture_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<ReviewableCapture, String> {
    with_store_mut(&state, |store| {
        let capture = store.captures().delete_capture(&capture_id)?;
        Ok(reviewable_from_lifecycle(&capture))
    })
}

// EXP-005: renderer-facing overlay controls. The hotkey itself is handled
// natively so the summon works while Aura is not focused; these commands
// let the overlay close itself and let other views open it programmatically.

// EXP-006: the Voice Pipeline surface. The overlay records push-to-talk
// audio in the webview, resamples it to 16 kHz float PCM, and hands it to
// these commands. Transcription resolves cloud-first when the user
// configured a key, otherwise entirely on-device via whisper-rs (the model
// is downloaded once at first use). The overlay never learns which tier ran.
#[tauri::command]
fn transcribe_audio(
    app: tauri::AppHandle,
    request: TranscriptionRequest,
) -> Result<TranscriptionResult, String> {
    // Transcription can take several seconds on CPU; never block the event
    // loop. The overlay shows its own thinking state while this runs.
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    std::thread::spawn(move || voice_pipeline::transcribe(request, &data_dir, None, None))
        .join()
        .map_err(|_| "Aura could not run its transcription pipeline safely.".to_string())?
        .map_err(|error| format!("Aura could not transcribe that recording: {error}."))
}

/// Whether the on-device speech model is installed and which STT tier will
/// run. The overlay uses this to show "download the speech model" as a
/// one-time first-run action and to label transcripts (local vs cloud).

// EXP-007: the Hybrid Brain — a single `run_brain` entry point runs the
// typed transcript through the tiered reasoning engine (local ollama,
// Groq/OpenAI cloud, deterministic floor). Settings are resolved at
// request time; secrets are opened from the sealed store and never travel
// through the typed command layer. The overlay also reads brain and
// settings status to build its "bring ollama online" and key affordances.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunBrainInput {
    transcript: String,
    /// Recent retrieval results — the only memory that ever leaves the
    /// machine. Raw captures, never the full store.
    #[serde(default)]
    recent_captures: Vec<String>,
}

#[tauri::command]
fn run_brain(
    _app: tauri::AppHandle,
    input: RunBrainInput,
    state: tauri::State<'_, AppState>,
) -> Result<brain::BrainResult, String> {
    let transcript = input.transcript.trim().to_string();
    if transcript.is_empty() {
        return Err("Cortex needs something to reason about — type or dictate first.".into());
    }

    let settings_store = state
        .settings_store
        .lock()
        .map_err(|_| "Aura could not access its settings safely.".to_string())?
        .clone();

    // The engine may wait up to two minutes on the local tier; never block
    // the Tauri event loop. The overlay renders its own thinking state.
    std::thread::spawn(move || {
        let settings = settings_store
            .load()
            .map_err(|error| format!("Aura could not read its settings: {error}"))?;
        let context = BrainContext {
            recent_captures: input.recent_captures,
        };
        let result = brain::execute_intent(&transcript, &context, &settings, &settings_store);
        Ok::<brain::BrainResult, String>(result)
    })
    .join()
    .map_err(|_| "Aura could not run its brain safely.".to_string())?
}

/// Reachability/status probe: does ollama have anything to run, and is a
/// cloud tier configured? The overlay uses this for the "install ollama"
/// one-time action and the key-status cards.
#[tauri::command]
fn get_brain_status(state: tauri::State<'_, AppState>) -> Result<brain::BrainStatus, String> {
    let key_vault = state
        .key_vault
        .lock()
        .map_err(|_| "Aura could not access its key vault safely.".to_string())?;
    let settings_store = SettingsStore::new(key_vault.clone());
    let settings = settings_store
        .load()
        .map_err(|error| format!("Aura could not read its settings: {error}"))?;
    Ok(brain::probe_status(&settings))
}

#[tauri::command]
fn get_settings(
    state: tauri::State<'_, AppState>,
) -> Result<cortex::settings::store::SettingsSnapshot, String> {
    let key_vault = state
        .key_vault
        .lock()
        .map_err(|_| "Aura could not access its key vault safely.".to_string())?;
    let settings_store = SettingsStore::new(key_vault.clone());
    settings_store
        .snapshot()
        .map_err(|error| format!("Aura could not read its settings: {error}"))
}

/// One field at a time, by name: the renderer never round-trips secrets —
/// key fields accept new key bytes only and the snapshot reports presence
/// without the value.
#[tauri::command]
fn save_setting(
    field: SettingField,
    state: tauri::State<'_, AppState>,
) -> Result<cortex::settings::store::SettingsSnapshot, String> {
    let key_vault = state
        .key_vault
        .lock()
        .map_err(|_| "Aura could not access its key vault safely.".to_string())?;
    let settings_store = SettingsStore::new(key_vault.clone());
    settings_store
        .update(field)
        .and_then(|_| settings_store.snapshot())
        .map_err(|error| format!("Aura could not save that setting: {error}"))
}

#[tauri::command]
fn get_stt_status(app: tauri::AppHandle) -> Result<SttStatus, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    let model_available = cortex::stt_local::is_model_available(&data_dir);
    Ok(SttStatus {
        model_available,
        prefer_cloud: false,
    })
}

#[derive(Debug, serde::Serialize)]
struct SttStatus {
    model_available: bool,
    prefer_cloud: bool,
}

#[tauri::command]
fn show_overlay(app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("overlay")
        .ok_or("The Neural Cortex overlay window is not available on this platform.")?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn hide_overlay(app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("overlay")
        .ok_or("The Neural Cortex overlay window is not available on this platform.")?;
    window.hide().map_err(|error| error.to_string())?;
    Ok(())
}

fn reviewable_from_lifecycle(
    capture: &db::repositories::captures::LifecycleCapture,
) -> ReviewableCapture {
    ReviewableCapture {
        id: capture.id.clone(),
        project_id: capture.project_id.clone(),
        label: capture.label.clone(),
        classification: capture.classification.clone(),
        created_at: capture.created_at.clone(),
        aged_at: capture.lifecycle_updated_at.clone(),
        days_aged: chrono::Utc::now()
            .signed_duration_since(
                capture
                    .lifecycle_updated_at
                    .parse::<chrono::DateTime<chrono::Utc>>()
                    .unwrap_or_else(|_| chrono::Utc::now()),
            )
            .num_days()
            .max(0),
    }
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

// EXP-001: native filesystem dialog support. The renderer never chooses
// paths itself; export and import destinations come from the native
// save/open dialogs wired into the three typed commands below.
#[tauri::command]
fn export_workspace(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ExportManifest, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "Aura could not access its local workspace safely.".to_string())?;
    let key_vault = state
        .key_vault
        .lock()
        .map_err(|_| "Aura could not access its key vault safely.".to_string())?;

    let service = ExportService::new(&mut store, &key_vault);

    service
        .record_export_event(
            ExportEvent::ExportRequested,
            &ExportRecordCounts::default(),
            "",
        )
        .ok();

    let envelope: ExportEnvelope = service
        .assemble_export()
        .map_err(|error| error.to_string())?;

    let manifest = ExportService::envelope_manifest(
        &serde_json::to_vec(&envelope)
            .map_err(|_| "Aura could not encode its export envelope.".to_string())?,
    )
    .map_err(|error| error.to_string())?;

    let envelope_json = serde_json::to_vec(&envelope)
        .map_err(|_| "Aura could not encode its export envelope.".to_string())?;

    // SAFETY: the dialog runs on the app handle captured from state via the
    // Manager trait; a blocking native dialog is intentional here so the
    // export write completes before the command returns.
    let result = std::thread::spawn(move || {
        let timestamp = db::migrations::utc_timestamp()
            .replace(|c: char| !c.is_ascii_alphanumeric(), "-")
            .replace(' ', "T")
            .replace(':', "-");
        app.dialog()
            .file()
            .set_title("Save the Aura workspace archive")
            .add_filter("Aura workspace archive", &["aura-export"])
            .set_file_name(format!("aura-workspace-{timestamp}.aura"))
            .blocking_save_file()
            .and_then(|file_path| match file_path.as_path() {
                Some(path) => std::fs::write(path, &envelope_json)
                    .map_err(|error| error.to_string())
                    .map(|_| path.to_path_buf())
                    .ok(),
                None => Some(PathBuf::new()),
            })
            .map(|path| {
                if path.as_os_str().is_empty() {
                    Err("The chosen export destination is not a filesystem path.".to_string())
                } else {
                    Ok(path)
                }
            })
    })
    .join()
    .map_err(|_| "Aura could not complete the workspace export safely.".to_string())?;

    match result {
        Some(Ok(path)) => {
            let detail = format!("Aura exported its local workspace to {}.", path.display());
            service
                .record_export_event(
                    ExportEvent::ExportCompleted,
                    &manifest.record_counts,
                    &detail,
                )
                .map_err(|error| error.to_string())?;
            Ok(manifest)
        }
        Some(Err(error)) => {
            service
                .record_export_event(
                    ExportEvent::ExportFailed,
                    &manifest.record_counts,
                    &format!("Aura could not write the archive: {error}."),
                )
                .ok();
            Err(format!("Aura could not write the export archive: {error}."))
        }
        None => {
            service
                .record_export_event(
                    ExportEvent::ExportFailed,
                    &manifest.record_counts,
                    "The export destination was cancelled.",
                )
                .ok();
            Err("The export was cancelled before Aura could save the archive.".to_string())
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportInput {
    passphrase: Option<String>,
}

#[tauri::command]
fn import_workspace(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    input: Option<ImportInput>,
) -> Result<ExportManifest, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "Aura could not access its local workspace safely.".to_string())?;
    let key_vault = state
        .key_vault
        .lock()
        .map_err(|_| "Aura could not access its key vault safely.".to_string())?;

    let chosen = std::thread::spawn(move || {
        app.dialog()
            .file()
            .set_title("Choose an Aura workspace archive")
            .add_filter("Aura workspace archive", &["aura"])
            .blocking_pick_file()
            .and_then(|file_path| file_path.as_path().map(std::path::Path::to_path_buf))
    })
    .join()
    .map_err(|_| "Aura could not complete the workspace recovery safely.".to_string())?;

    let source_path = chosen.ok_or_else(|| {
        "The import was cancelled before Aura could read the archive.".to_string()
    })?;

    let raw = std::fs::read(&source_path)
        .map_err(|error| format!("Aura could not read that archive: {error}."))?;

    let manifest = ExportService::envelope_manifest(&raw).map_err(|error| error.to_string())?;

    let mut service = ExportService::new(&mut store, &key_vault);
    service
        .record_export_event(ExportEvent::ImportRequested, &manifest.record_counts, "")
        .ok();

    // EXP-002: a passphrase-sealed archive can only open with the
    // passphrase that sealed it; a DPAPI archive ignores this input.
    if manifest.sealing == PASSPHRASE_SEALING {
        let passphrase_text = input
            .and_then(|value| value.passphrase)
            .ok_or_else(|| {
                "That archive was sealed with a passphrase. Enter the passphrase before Aura can restore it.".to_string()
            })?;
        service.set_passphrase(Some(Passphrase::new(passphrase_text)));
    }

    let counts = service.apply_import(&raw).map_err(|error| {
        service
            .record_export_event(
                ExportEvent::ImportFailed,
                &manifest.record_counts,
                &format!("Aura could not recover its local workspace: {error}."),
            )
            .ok();
        error.to_string()
    })?;

    service
        .record_export_event(
            ExportEvent::ImportCompleted,
            &counts,
            &format!(
                "Aura recovered its local workspace from {}.",
                source_path.display()
            ),
        )
        .map_err(|error| error.to_string())?;

    Ok(manifest)
}

#[tauri::command]
fn export_manifest(source_path: String) -> Result<ExportManifest, String> {
    let raw = std::fs::read(&source_path)
        .map_err(|error| format!("Aura could not read that export file: {error}."))?;
    ExportService::envelope_manifest(&raw).map_err(|error| error.to_string())
}

// EXP-002: passphrase re-sealing for portable archives. The regular
// `export_workspace` command keeps the DPAPI-only behavior; this command
// seals the same envelope with a passphrase-derived key so the archive
// opens on any Aura installation.
#[tauri::command]
fn export_workspace_with_passphrase(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    passphrase: String,
) -> Result<ExportManifest, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "Aura could not access its local workspace safely.".to_string())?;
    let key_vault = state
        .key_vault
        .lock()
        .map_err(|_| "Aura could not access its key vault safely.".to_string())?;

    let service = ExportService::new(&mut store, &key_vault);

    service
        .record_export_event(
            ExportEvent::ExportRequested,
            &ExportRecordCounts::default(),
            "",
        )
        .ok();

    let envelope: ExportEnvelope = service
        .assemble_passphrase_export(Passphrase::new(passphrase))
        .map_err(|error| error.to_string())?;

    let manifest = ExportService::envelope_manifest(
        &serde_json::to_vec(&envelope)
            .map_err(|_| "Aura could not encode its export envelope.".to_string())?,
    )
    .map_err(|error| error.to_string())?;

    let envelope_json = serde_json::to_vec(&envelope)
        .map_err(|_| "Aura could not encode its export envelope.".to_string())?;

    let result = std::thread::spawn(move || {
        let timestamp = db::migrations::utc_timestamp()
            .replace(|c: char| !c.is_ascii_alphanumeric(), "-")
            .replace(' ', "T")
            .replace(':', "-");
        app.dialog()
            .file()
            .set_title("Save the passphrase-protected Aura archive")
            .add_filter("Aura workspace archive", &["aura"])
            .set_file_name(format!("aura-workspace-{timestamp}.aura"))
            .blocking_save_file()
            .and_then(|file_path| match file_path.as_path() {
                Some(path) => std::fs::write(path, &envelope_json)
                    .map_err(|error| error.to_string())
                    .map(|_| path.to_path_buf())
                    .ok(),
                None => Some(PathBuf::new()),
            })
            .map(|path| {
                if path.as_os_str().is_empty() {
                    Err("The chosen export destination is not a filesystem path.".to_string())
                } else {
                    Ok(path)
                }
            })
    })
    .join()
    .map_err(|_| "Aura could not complete the passphrase export safely.".to_string())?;

    match result {
        Some(Ok(path)) => {
            let detail = format!(
                "Aura exported its local workspace, sealed with a passphrase, to {}.",
                path.display()
            );
            service
                .record_export_event(
                    ExportEvent::ExportCompleted,
                    &manifest.record_counts,
                    &detail,
                )
                .map_err(|error| error.to_string())?;
            Ok(manifest)
        }
        Some(Err(error)) => {
            service
                .record_export_event(
                    ExportEvent::ExportFailed,
                    &manifest.record_counts,
                    &format!("Aura could not write the archive: {error}."),
                )
                .ok();
            Err(format!("Aura could not write the export archive: {error}."))
        }
        None => {
            service
                .record_export_event(
                    ExportEvent::ExportFailed,
                    &manifest.record_counts,
                    "The export destination was cancelled.",
                )
                .ok();
            Err("The export was cancelled before Aura could save the archive.".to_string())
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
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
                key_vault: Mutex::new(key_vault.clone()),
                settings_store: Mutex::new(SettingsStore::new(key_vault)),
            });

            // EXP-005: the Neural Cortex summon hotkey. Alt+Space opens (or
            // refocuses) the overlay window from anywhere on the desktop.
            // The toggle command is the single entry point; voice and the
            // real brain plug into it in later increments.
            let overlay_label = "overlay".to_string();
            app.global_shortcut()
                .on_shortcut("Alt+Space", move |app_handle, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        let label = overlay_label.clone();
                        let handle = app_handle.clone();
                        std::thread::spawn(move || {
                            // Best-effort: showing the overlay must never panic
                            // the app on platforms or states we did not expect.
                            if let Some(window) = handle.get_webview_window(&label) {
                                let visible = window.is_visible().unwrap_or(false);
                                if !visible {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                } else {
                                    let _ = window.set_focus();
                                }
                            }
                        });
                    }
                })
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            seal_secret,
            open_secret,
            key_vault_status,
            export_workspace,
            import_workspace,
            export_manifest,
            export_workspace_with_passphrase,
            run_retention_sweep,
            list_reviewable_captures,
            keep_capture,
            expire_capture,
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
            set_exclusion_enabled,
            show_overlay,
            hide_overlay,
            transcribe_audio,
            get_stt_status,
            run_brain,
            get_brain_status,
            get_settings,
            save_setting,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Aura");
}

// EXP-005 tests live in application/overlay_service.rs; the raw window
// commands cannot be unit-tested without a live AppHandle, so they are
// covered by the Windows build and the friction-test loop.

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
