use crate::domain::capture::CaptureRetention;
use crate::domain::project::AuraError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyMode {
    ManualOnly,
    Paused,
}

impl PrivacyMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ManualOnly => "manual_only",
            Self::Paused => "paused",
        }
    }

    pub fn from_store(value: &str) -> Result<Self, AuraError> {
        match value {
            "manual_only" => Ok(Self::ManualOnly),
            "paused" => Ok(Self::Paused),
            _ => Err(AuraError::Storage(
                "Aura found an unsupported local privacy mode.".to_string(),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionKind {
    Application,
    Domain,
    Project,
}

impl ExclusionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Application => "application",
            Self::Domain => "domain",
            Self::Project => "project",
        }
    }

    pub fn from_store(value: &str) -> Result<Self, AuraError> {
        match value {
            "application" => Ok(Self::Application),
            "domain" => Ok(Self::Domain),
            "project" => Ok(Self::Project),
            _ => Err(AuraError::Storage(
                "Aura found an unsupported local exclusion rule type.".to_string(),
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExclusionRule {
    pub id: String,
    pub kind: ExclusionKind,
    pub value: String,
    pub is_enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyPreferences {
    pub privacy_mode: PrivacyMode,
    pub default_capture_retention: CaptureRetention,
    pub exclusions: Vec<ExclusionRule>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePrivacyPreferencesInput {
    pub privacy_mode: PrivacyMode,
    pub default_capture_retention: CaptureRetention,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateExclusionRuleInput {
    pub kind: ExclusionKind,
    pub value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetExclusionEnabledInput {
    pub is_enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn privacy_mode_serializes_to_supported_values() {
        let mode = PrivacyMode::ManualOnly;

        assert_eq!(mode.as_str(), "manual_only");
        assert_eq!(PrivacyMode::Paused.as_str(), "paused");
        assert_eq!(PrivacyMode::from_store("manual_only").expect("mode"), mode);
        assert_eq!(
            PrivacyMode::from_store("focused"),
            Err(AuraError::Storage(
                "Aura found an unsupported local privacy mode.".to_string()
            ))
        );
    }

    #[test]
    fn exclusion_kind_roundtrips_through_the_store_format() {
        for kind in [
            ExclusionKind::Application,
            ExclusionKind::Domain,
            ExclusionKind::Project,
        ] {
            assert_eq!(
                ExclusionKind::from_store(kind.as_str()).expect("kind"),
                kind
            );
        }

        assert!(ExclusionKind::from_store("unknown").is_err());
    }
}
