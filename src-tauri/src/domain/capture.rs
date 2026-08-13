use crate::domain::project::AuraError;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureKind {
    ManualNote,
    PastedText,
    Url,
}

impl CaptureKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ManualNote => "manual_note",
            Self::PastedText => "pasted_text",
            Self::Url => "url",
        }
    }

    pub fn from_store(value: &str) -> Result<Self, AuraError> {
        match value {
            "manual_note" => Ok(Self::ManualNote),
            "pasted_text" => Ok(Self::PastedText),
            "url" => Ok(Self::Url),
            _ => Err(AuraError::Storage(
                "Aura found an unsupported local capture type.".to_string(),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CaptureClassification {
    Standard,
    Sensitive,
}

impl CaptureClassification {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Sensitive => "sensitive",
        }
    }

    pub fn from_store(value: &str) -> Result<Self, AuraError> {
        match value {
            "standard" => Ok(Self::Standard),
            "sensitive" => Ok(Self::Sensitive),
            _ => Err(AuraError::Storage(
                "Aura found an unsupported local capture classification.".to_string(),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureRetention {
    UntilDeleted,
    ReviewIn30Days,
}

impl CaptureRetention {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UntilDeleted => "until_deleted",
            Self::ReviewIn30Days => "review_in_30_days",
        }
    }

    pub fn from_store(value: &str) -> Result<Self, AuraError> {
        match value {
            "until_deleted" => Ok(Self::UntilDeleted),
            "review_in_30_days" => Ok(Self::ReviewIn30Days),
            _ => Err(AuraError::Storage(
                "Aura found an unsupported local capture retention setting.".to_string(),
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRecord {
    pub id: String,
    pub project_id: String,
    pub kind: CaptureKind,
    pub label: String,
    pub content: String,
    pub classification: CaptureClassification,
    pub retention: CaptureRetention,
    pub created_at: String,
}
