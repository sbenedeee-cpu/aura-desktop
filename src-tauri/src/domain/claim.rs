use crate::domain::project::AuraError;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimConfidence {
    Low,
    Medium,
    High,
}

impl ClaimConfidence {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    pub fn from_store(value: &str) -> Result<Self, AuraError> {
        match value {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            _ => Err(AuraError::Storage(
                "Aura found a decision with an unsupported local confidence value.".to_string(),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimStatus {
    Confirmed,
    Superseded,
}

impl ClaimStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::Superseded => "superseded",
        }
    }

    pub fn from_store(value: &str) -> Result<Self, AuraError> {
        match value {
            "confirmed" => Ok(Self::Confirmed),
            "superseded" => Ok(Self::Superseded),
            _ => Err(AuraError::Storage(
                "Aura found a decision with an unsupported local lifecycle state.".to_string(),
            )),
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClaimSource {
    pub id: String,
    pub label: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DecisionClaim {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub rationale: String,
    pub confidence: String,
    pub author_type: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub supersedes_claim_id: Option<String>,
    pub superseded_by_claim_id: Option<String>,
    pub sources: Vec<ClaimSource>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DecisionSummary {
    pub id: String,
    pub title: String,
    pub confidence: String,
    pub status: String,
    pub created_at: String,
}

impl From<&DecisionClaim> for DecisionSummary {
    fn from(value: &DecisionClaim) -> Self {
        Self {
            id: value.id.clone(),
            title: value.title.clone(),
            confidence: value.confidence.clone(),
            status: value.status.clone(),
            created_at: value.created_at.clone(),
        }
    }
}
