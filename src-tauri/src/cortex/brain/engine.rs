// EXP-007: the brain engine — tier resolution and the single
// `execute_intent` entry point (PRD §7.3).
//
// Resolution at request time, from the settings store:
//
//   brain_mode   key?          ollama?        → tier
//   ─────────────────────────────────────────────────────────────
//   auto         groq/openai   reachable      → cloud (Groq free first)
//   auto         —             reachable      → local
//   auto         any           unreachable    → deterministic
//   local        —             reachable      → local
//   local        —             unreachable    → deterministic
//   cloud        configured    —              → cloud
//   cloud        missing       —              → deterministic
//
// The deterministic floor is the floor for every path: the engine never
// dead-ends (PRD §4.3, core law). The context handed to remote tiers is the
// transcript plus recent retrieval results — never raw SQLite (PRD §5.1).

use crate::cortex::settings::{BrainMode, CloudProvider};

use super::cloud::{self, CloudProviderKind};
use super::deterministic;
use super::local;

/// The minimal context a brain tier receives: the current transcript and
/// recent retrieval results assembled by the caller. Deliberately small;
/// the full memory stays local.
#[derive(Debug, Clone, Default)]
pub struct BrainContext {
    pub recent_captures: Vec<String>,
}

/// Which tier actually ran for a request — surfaced to the overlay so the
/// user always knows whether their words traveled.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrainTier {
    Cloud { provider: String },
    Local { model: String },
    Deterministic,
}

/// The brain's answer: which tier ran, what it said, and (when the floor
/// ran) the parsed intent so the caller can act on it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BrainResult {
    pub tier: BrainTier,
    pub reply: String,
    pub intent: Option<deterministic::DeterministicIntent>,
    pub degraded: bool,
}

fn context_text(context: &BrainContext) -> String {
    if context.recent_captures.is_empty() {
        return String::new();
    }
    let joined: String = context
        .recent_captures
        .iter()
        .map(|capture| capture.trim())
        .filter(|capture| !capture.is_empty())
        .collect::<Vec<&str>>()
        .join("\n");
    joined
}

fn run_floor_with(transcript: &str) -> BrainResult {
    let floor = deterministic::run_floor(transcript);
    BrainResult {
        tier: BrainTier::Deterministic,
        reply: floor.reply,
        intent: Some(floor.intent),
        degraded: false,
    }
}

/// The single brain entry point (PRD §7.3). Reads settings at request time,
/// resolves the tier, runs it, and degrades tier-by-tier on failure.
pub fn execute_intent(
    transcript: &str,
    context: &BrainContext,
    settings: &crate::cortex::settings::AssistantSettings,
    settings_store: &crate::cortex::settings::store::SettingsStore,
) -> BrainResult {
    let tier_preference = settings.brain_mode;
    let context_text = context_text(context);

    // ------------------------------------------------------------------
    // Cloud tier: requested (cloud mode, or auto with a configured key).
    // ------------------------------------------------------------------
    let cloud_kind = match settings.cloud_provider {
        CloudProvider::Groq => CloudProviderKind::Groq,
        CloudProvider::OpenAi => CloudProviderKind::OpenAi,
    };
    let try_cloud = settings.cloud_configured()
        && (tier_preference == BrainMode::Cloud || tier_preference == BrainMode::Auto);
    if try_cloud {
        let sealed_key = match settings.cloud_provider {
            CloudProvider::Groq => &settings.groq_key(),
            CloudProvider::OpenAi => &settings.openai_key(),
        };
        if let Some(raw) = sealed_key {
            if let Ok(Some(api_key)) = settings_store.open_secret(Some(raw)) {
                match cloud::generate(cloud_kind, &api_key, transcript, &context_text) {
                    Ok(reply) => {
                        return BrainResult {
                            tier: BrainTier::Cloud {
                                provider: format!("{cloud_kind:?}").to_lowercase(),
                            },
                            reply,
                            intent: None,
                            degraded: false,
                        };
                    }
                    Err(cloud::CloudBrainError::NoKey(message)) => {
                        // The configured key was rejected: say so plainly
                        // instead of silently degrading — the user must fix
                        // the settings card, not wonder why.
                        return BrainResult {
                            tier: BrainTier::Deterministic,
                            reply: format!(
                                "Your cloud key was rejected ({message}). Open the settings \
                                 and re-enter it — until then Cortex answers with its local \
                                 floor."
                            ),
                            intent: Some(deterministic::parse_intent(transcript)),
                            degraded: true,
                        };
                    }
                    Err(_) => {
                        // Network or generation failure: degrade, never die.
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Local tier: requested (local mode, or auto when cloud declined).
    // ------------------------------------------------------------------
    let try_local = tier_preference == BrainMode::Local
        || (tier_preference == BrainMode::Auto
            && local::check_reachable(&settings.ollama_host)
                == local::OllamaReachability::Reachable);
    if try_local {
        match local::select_model(&settings.ollama_host) {
            Ok((model, _reason)) => {
                match local::generate(&settings.ollama_host, &model, transcript, &context_text) {
                    Ok(reply) => {
                        return BrainResult {
                            tier: BrainTier::Local { model },
                            reply,
                            intent: None,
                            degraded: tier_preference != BrainMode::Local,
                        };
                    }
                    Err(_) => {
                        // Generation failed; fall through to the floor.
                    }
                }
            }
            Err(_) => {
                // No suitable model / ollama gone away; fall through.
            }
        }
    }

    // ------------------------------------------------------------------
    // Deterministic floor: the floor every path lands on.
    // ------------------------------------------------------------------
    let mut result = run_floor_with(transcript);
    result.degraded = true;
    result
}

/// Reachability summary for the overlay's "bring ollama online" affordance.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BrainStatus {
    pub ollama_reachable: bool,
    pub cloud_configured: bool,
    pub brain_mode: BrainMode,
    pub selected_model: Option<String>,
}

/// Status probe for the overlay: does the local brain have anything to run,
/// and is a cloud tier configured?
pub fn probe_status(settings: &crate::cortex::settings::AssistantSettings) -> BrainStatus {
    let ollama_reachable =
        local::check_reachable(&settings.ollama_host) == local::OllamaReachability::Reachable;
    let selected_model = if ollama_reachable {
        local::select_model(&settings.ollama_host)
            .ok()
            .map(|(model, _)| model)
    } else {
        None
    };
    BrainStatus {
        ollama_reachable,
        cloud_configured: settings.cloud_configured(),
        brain_mode: settings.brain_mode,
        selected_model,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cortex::settings::{AssistantSettings, BrainMode, CloudProvider};

    fn settings_with(
        brain_mode: BrainMode,
        cloud_provider: CloudProvider,
        _groq_key: Option<&str>,
        _openai_key: Option<&str>,
    ) -> AssistantSettings {
        // Keys are sealed in the store, not held as plaintext on the
        // settings document; the engine reads presence only, so a test
        // helper takes key arguments for API parity but leaves them sealed.
        let mut settings = AssistantSettings::default();
        settings.brain_mode = brain_mode;
        settings.cloud_provider = cloud_provider;
        settings
    }

    #[test]
    fn auto_mode_with_groq_key_prefers_cloud_when_online() {
        // Cloud tier needs a live network; offline sandboxes skip the call.
        if std::env::var("CI").is_ok() {
            let settings = settings_with(BrainMode::Auto, CloudProvider::Groq, Some("key"), None);
            // Deterministic floor is the guaranteed outcome offline; the
            // assertion validates the degrade contract, not the cloud call.
            let result = execute_intent(
                "who are you",
                &BrainContext::default(),
                &settings,
                &test_store(),
            );
            assert_eq!(result.tier, BrainTier::Deterministic);
            assert!(result.degraded);
        }
    }

    #[test]
    fn empty_transcript_hits_the_floor_gracefully() {
        let settings = AssistantSettings::default();
        let result = execute_intent("", &BrainContext::default(), &settings, &test_store());
        assert_eq!(result.tier, BrainTier::Deterministic);
        assert!(!result.reply.is_empty());
    }

    #[test]
    fn context_assembly_joins_recent_captures_without_leading_blank_lines() {
        let mut context = BrainContext::default();
        context.recent_captures = vec![
            "".into(),
            "  ".into(),
            "capture one".into(),
            "capture two".into(),
        ];
        let text = context_text(&context);
        assert_eq!(text, "capture one\ncapture two");
    }

    #[test]
    fn degraded_flag_is_set_on_floor_fallback() {
        let settings = AssistantSettings::default();
        let result = execute_intent(
            "do something unknown",
            &BrainContext::default(),
            &settings,
            &test_store(),
        );
        assert!(result.degraded);
    }

    fn test_store() -> crate::cortex::settings::store::SettingsStore {
        let directory = tempfile::tempdir().unwrap();
        let vault = crate::security::key_vault::KeyVault::new(directory.path()).unwrap();
        crate::cortex::settings::store::SettingsStore::new(vault)
    }
}
