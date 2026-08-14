use serde::Serialize;

use super::project::AuraError;

/// Local lifecycle state of a manual capture, owned exclusively by the
/// retention sweep. Sensitive captures are never moved out of `active`
/// automatically; `deleted` is reached only through a deliberate user action
/// (keep / expire / delete), never through background automation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    Active,
    Aged,
    Deleted,
}

impl LifecycleState {
    /// Serialises the lifecycle state for the captures table. Kept even when
    /// no caller references it yet, because the repository layer persists
    /// `LifecycleState` through this function.
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Aged => "aged",
            Self::Deleted => "deleted",
        }
    }

    /// Parses the lifecycle state stored in the captures table. Kept even when
    /// no caller references it yet, because the repository layer maps database
    /// rows into `LifecycleState` through this function.
    #[allow(dead_code)]
    pub fn from_store(value: &str) -> Result<Self, AuraError> {
        match value {
            "active" => Ok(Self::Active),
            "aged" => Ok(Self::Aged),
            "deleted" => Ok(Self::Deleted),
            _ => Err(AuraError::Storage(
                "Aura found an unsupported capture lifecycle state.".to_string(),
            )),
        }
    }
}

/// Outcome of a single retention-sweep pass. A pass never deletes anything on
/// its own; it only surfaces aged, standard-classification captures for
/// human review.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetentionSweepResult {
    pub swept_at: String,
    pub captures_reviewed: u64,
    pub captures_aged_now: u64,
    pub captures_already_aged: u64,
    pub captures_protected: u64,
}

/// A capture that the retention sweep has surfaced for human review. Its
/// `expires_after` window is advisory: the renderer decides whether to keep
/// or expire it, and every transition is written as an audit event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewableCapture {
    pub id: String,
    pub project_id: String,
    pub label: String,
    pub classification: String,
    pub created_at: String,
    pub aged_at: String,
    pub days_aged: i64,
}
