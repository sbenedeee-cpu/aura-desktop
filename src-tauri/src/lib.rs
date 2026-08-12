use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum PrivacyMode {
    Focused,
    Paused,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Project {
    id: String,
    name: String,
    status: String,
    signal: String,
    progress: u8,
    updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Signal {
    id: String,
    kind: String,
    title: String,
    detail: String,
    time: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceSnapshot {
    active_project: String,
    continuity_note: String,
    next_step: String,
    privacy_mode: PrivacyMode,
    projects: Vec<Project>,
    signals: Vec<Signal>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CaptureMarker {
    project_id: String,
    captured_at: String,
    source: String,
}

struct AppState {
    privacy_mode: Mutex<PrivacyMode>,
    capture_markers: Mutex<Vec<CaptureMarker>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            privacy_mode: Mutex::new(PrivacyMode::Focused),
            capture_markers: Mutex::new(Vec::new()),
        }
    }
}

#[tauri::command]
fn get_workspace_snapshot(state: tauri::State<'_, AppState>) -> Result<WorkspaceSnapshot, String> {
    let privacy_mode = state
        .privacy_mode
        .lock()
        .map_err(|_| "Aura could not read its local privacy state.".to_string())?
        .clone();

    Ok(WorkspaceSnapshot {
        active_project: "Aura Desktop".to_string(),
        continuity_note: "The architecture scaffold is active. Capture remains intentional while the Windows perception contract is being verified.".to_string(),
        next_step: "Define the first user-authorized context capture flow.".to_string(),
        privacy_mode,
        projects: vec![
            Project {
                id: "aura".to_string(),
                name: "Aura Desktop".to_string(),
                status: "In progress".to_string(),
                signal: "Architecture baseline".to_string(),
                progress: 24,
                updated_at: "Now".to_string(),
            },
            Project {
                id: "ascend".to_string(),
                name: "Ascend".to_string(),
                status: "Paused".to_string(),
                signal: "Awaiting scope review".to_string(),
                progress: 58,
                updated_at: "Yesterday".to_string(),
            },
            Project {
                id: "eternal".to_string(),
                name: "Eternal Studios".to_string(),
                status: "Active".to_string(),
                signal: "Brand-system decisions".to_string(),
                progress: 72,
                updated_at: "2 days ago".to_string(),
            },
        ],
        signals: vec![
            Signal {
                id: "signal-1".to_string(),
                kind: "decision".to_string(),
                title: "Windows-first stack selected".to_string(),
                detail: "Tauri 2, React, TypeScript, and Rust establish the initial desktop boundary.".to_string(),
                time: "Just now".to_string(),
            },
            Signal {
                id: "signal-2".to_string(),
                kind: "context".to_string(),
                title: "Intentional capture is active".to_string(),
                detail: "Aura will not observe or send desktop context until an explicit capture workflow exists.".to_string(),
                time: "Today".to_string(),
            },
            Signal {
                id: "signal-3".to_string(),
                kind: "memory".to_string(),
                title: "Research mandate linked".to_string(),
                detail: "The saved master research mandate is the source of truth for product and technical decisions.".to_string(),
                time: "Today".to_string(),
            },
        ],
    })
}

#[tauri::command]
fn set_privacy_mode(mode: PrivacyMode, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut privacy_mode = state
        .privacy_mode
        .lock()
        .map_err(|_| "Aura could not update its local privacy state.".to_string())?;

    *privacy_mode = mode;
    Ok(())
}

fn build_capture_marker(
    project_id: String,
    privacy_mode: &PrivacyMode,
) -> Result<CaptureMarker, String> {
    if matches!(privacy_mode, PrivacyMode::Paused) {
        return Err("Intentional capture is paused. Resume it before adding context.".to_string());
    }

    Ok(CaptureMarker {
        project_id,
        captured_at: "local-session".to_string(),
        source: "manual-context-marker".to_string(),
    })
}

#[tauri::command]
fn record_intentional_capture(
    project_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<CaptureMarker, String> {
    let privacy_mode = state
        .privacy_mode
        .lock()
        .map_err(|_| "Aura could not read its local privacy state.".to_string())?
        .clone();

    let marker = build_capture_marker(project_id, &privacy_mode)?;

    state
        .capture_markers
        .lock()
        .map_err(|_| "Aura could not save the local context marker.".to_string())?
        .push(CaptureMarker {
            project_id: marker.project_id.clone(),
            captured_at: marker.captured_at.clone(),
            source: marker.source.clone(),
        });

    Ok(marker)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            get_workspace_snapshot,
            set_privacy_mode,
            record_intentional_capture
        ])
        .run(tauri::generate_context!())
        .expect("error while running Aura");
}

#[cfg(test)]
mod tests {
    use super::{build_capture_marker, PrivacyMode};

    #[test]
    fn paused_privacy_mode_blocks_intentional_capture() {
        let result = build_capture_marker("aura".to_string(), &PrivacyMode::Paused);

        assert_eq!(
            result.unwrap_err(),
            "Intentional capture is paused. Resume it before adding context."
        );
    }

    #[test]
    fn focused_privacy_mode_creates_a_manual_marker() {
        let marker = build_capture_marker("aura".to_string(), &PrivacyMode::Focused)
            .expect("focused mode should permit intentional capture");

        assert_eq!(marker.project_id, "aura");
        assert_eq!(marker.source, "manual-context-marker");
    }
}
