// Test fixtures for Aura V0 Local Storage Architecture

/// Representation of a Project fixture
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ProjectFixture {
    pub id: String,
    pub name: String,
    pub goal: String,
    pub status: String,
    pub current_task: String,
    pub blocker: String,
    pub next_step: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Representation of a Task fixture
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct TaskFixture {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Representation of an Event/Audit fixture
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct EventFixture {
    pub id: String,
    pub project_id: Option<String>,
    pub kind: String,
    pub actor: String,
    pub occurred_at: String,
    pub payload: String,
}

/// Helper function to generate mock domain fixtures for testing
pub fn get_mock_project_fixture() -> ProjectFixture {
    ProjectFixture {
        id: "aura".to_string(),
        name: "Aura Desktop".to_string(),
        goal: "Convert validated architecture shell into a trustworthy local-first system"
            .to_string(),
        status: "In progress".to_string(),
        current_task: "Implement safe DB persistence and tests".to_string(),
        blocker: "Awaiting approval of ADR-003".to_string(),
        next_step: "Establish local SQLite database layer".to_string(),
        created_at: "2026-08-12T14:00:00Z".to_string(),
        updated_at: "2026-08-12T15:30:00Z".to_string(),
    }
}

pub fn get_mock_task_fixture() -> TaskFixture {
    TaskFixture {
        id: "task-001".to_string(),
        project_id: "aura".to_string(),
        title: "Setup rusqlite and migrations".to_string(),
        status: "todo".to_string(),
        created_at: "2026-08-12T14:10:00Z".to_string(),
        updated_at: "2026-08-12T14:10:00Z".to_string(),
    }
}

pub fn get_mock_event_fixture() -> EventFixture {
    EventFixture {
        id: "event-101".to_string(),
        project_id: Some("aura".to_string()),
        kind: "CAPTURE_CREATED".to_string(),
        actor: "user".to_string(),
        occurred_at: "2026-08-12T14:15:00Z".to_string(),
        payload: r#"{"type":"note","title":"Local DB design approved"}"#.to_string(),
    }
}
