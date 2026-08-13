use serde::Serialize;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub goal: Option<String>,
    pub status: ProjectStatus,
    pub current_task: Option<String>,
    pub blocker: Option<String>,
    pub next_step: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    Active,
    Paused,
    Archived,
}

impl ProjectStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Archived => "archived",
        }
    }

    pub fn from_store(value: &str) -> Result<Self, AuraError> {
        match value {
            "active" => Ok(Self::Active),
            "paused" => Ok(Self::Paused),
            "archived" => Ok(Self::Archived),
            _ => Err(AuraError::Storage(
                "Aura found a project with an unsupported local status.".to_string(),
            )),
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Paused => "Paused",
            Self::Archived => "Archived",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum AuraError {
    InvalidInput(String),
    NotFound(String),
    Privacy(String),
    Storage(String),
}

impl std::fmt::Display for AuraError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(message)
            | Self::NotFound(message)
            | Self::Privacy(message)
            | Self::Storage(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for AuraError {}
