// EXP-007: settings persistence.
//
// `AssistantSettings` is the single source of configuration truth. Field
// updates are atomic at the file level: the whole document is rewritten
// through a staging file with a rename, so a crash mid-save never leaves a
// half-written settings file (the same strategy as the whisper model
// download). Secrets are sealed/unsealed through the KeyVault in place; the
// vault never exposes raw key material through any Tauri command.

use crate::security::key_vault::KeyVault;
use crate::security::key_vault::KeyVaultError;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Minimum and maximum of the configurable context ageing window (days).
pub const AGEING_WINDOW_MIN_DAYS: u32 = 7;
/// Bounded per PRD §7.3: the ageing window is configurable within a range,
/// never unbounded growth and never a window so short that capture is lost.
pub const AGEING_WINDOW_MAX_DAYS: u32 = 365;
/// Sensible default: captures older than three months fold into review.
pub const AGEING_WINDOW_DEFAULT_DAYS: u32 = 90;

/// Which tier the brain resolves to at request time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum BrainMode {
    /// Resolve per request: cloud when a key is configured and online, else
    /// local ollama when reachable, else deterministic. Never dead-ends.
    #[default]
    Auto,
    /// On-device ollama only; falls back to deterministic when unreachable.
    Local,
    /// Cloud provider only; falls back to deterministic when unreachable.
    Cloud,
}

/// Cloud reasoning/STT provider family. Groq is the budget pick (free tier,
/// OpenAI-compatible REST) per the PRD §7.3/§7.2 cost analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum CloudProvider {
    #[default]
    Groq,
    OpenAi,
}

/// STT resolution mode, mirroring the brain's tier philosophy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum SttMode {
    #[default]
    Auto,
    Local,
    Cloud,
}

/// Whether voice replies (TTS, EXP-009 surface) are enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum VoiceReply {
    #[default]
    Off,
    On,
}

/// A sealed secret: either a present sealed blob (the KeyVault's
/// self-describing byte envelope) or explicit absence.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
enum MaybeSealed {
    Sealed(Vec<u8>),
    #[default]
    Absent,
}

impl MaybeSealed {
    fn as_sealed(&self) -> Option<KeyVaultEnvelope> {
        match self {
            Self::Sealed(raw) => KeyVault::decode_sealed(raw).ok(),
            Self::Absent => None,
        }
    }
}

/// Local alias for the vault's envelope type, kept private to this module.
type KeyVaultEnvelope = crate::security::key_vault::SealedValue;

/// Assistant-wide configuration. Secrets arrive sealed; preferences arrive
/// as plain JSON. The document is small by design: it is the only
/// configuration surface the user ever edits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantSettings {
    pub brain_mode: BrainMode,
    pub cloud_provider: CloudProvider,
    /// Sealed Groq API key (groq.com free tier is the budget cloud pick).
    #[serde(default)]
    groq_api_key: MaybeSealed,
    /// Sealed OpenAI API key (paid fallback, ~$0.006/min).
    #[serde(default)]
    openai_api_key: MaybeSealed,
    pub ollama_host: String,
    pub stt_mode: SttMode,
    pub voice_reply: VoiceReply,
    /// EXP-004 (absorbed into EXP-007): captures older than this fold into
    /// the review window. Bounded to [7, 365] days.
    pub ageing_window_days: u32,
    /// Whether context ageing is enabled at all.
    pub ageing_enabled: bool,
}

impl Default for AssistantSettings {
    fn default() -> Self {
        Self {
            brain_mode: BrainMode::default(),
            cloud_provider: CloudProvider::default(),
            groq_api_key: MaybeSealed::Absent,
            openai_api_key: MaybeSealed::Absent,
            ollama_host: "http://localhost:11434".into(),
            stt_mode: SttMode::default(),
            voice_reply: VoiceReply::default(),
            ageing_window_days: AGEING_WINDOW_DEFAULT_DAYS,
            ageing_enabled: true,
        }
    }
}

impl AssistantSettings {
    /// The sealed Groq key bytes, if present (opaque to all callers except
    /// the settings store's own secret opener).
    pub(crate) fn groq_key(&self) -> Option<&Vec<u8>> {
        match &self.groq_api_key {
            MaybeSealed::Sealed(raw) => Some(raw),
            MaybeSealed::Absent => None,
        }
    }

    /// The sealed OpenAI key bytes, if present.
    pub(crate) fn openai_key(&self) -> Option<&Vec<u8>> {
        match &self.openai_api_key {
            MaybeSealed::Sealed(raw) => Some(raw),
            MaybeSealed::Absent => None,
        }
    }

    /// True when a cloud tier can run: a key for the configured provider is
    /// present in the store (presence ≠ validity — the call itself decides).
    pub fn cloud_configured(&self) -> bool {
        match self.cloud_provider {
            CloudProvider::Groq => self.groq_api_key.as_sealed().is_some(),
            CloudProvider::OpenAi => self.openai_api_key.as_sealed().is_some(),
        }
    }

    /// The bounded ageing window, clamped defensively even if the file was
    /// hand-edited outside the allowed range.
    pub fn ageing_window_days_bounded(&self) -> u32 {
        self.ageing_window_days
            .clamp(AGEING_WINDOW_MIN_DAYS, AGEING_WINDOW_MAX_DAYS)
    }
}

/// One field-level setting mutation. Splitting updates by field keeps the
/// renderer from ever round-tripping secrets: the key input sends only the
/// new key bytes, and `get` never returns the secret back.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "field", content = "value", rename_all = "snake_case")]
pub enum SettingField {
    BrainMode(BrainMode),
    CloudProvider(CloudProvider),
    GroqApiKey(String),
    OpenAiApiKey(String),
    OllamaHost(String),
    SttMode(SttMode),
    VoiceReply(VoiceReply),
    AgeingWindowDays(u32),
    AgeingEnabled(bool),
}

/// Errors raised by the settings store.
#[derive(Debug)]
pub enum SettingsError {
    Vault(KeyVaultError),
    Serialization(String),
    Storage(String),
    Validation(String),
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Vault(error) => write!(formatter, "{error}"),
            Self::Serialization(detail) => write!(formatter, "settings format error: {detail}"),
            Self::Storage(detail) => write!(formatter, "settings storage error: {detail}"),
            Self::Validation(detail) => write!(formatter, "settings rejected: {detail}"),
        }
    }
}

impl std::error::Error for SettingsError {}

impl From<KeyVaultError> for SettingsError {
    fn from(error: KeyVaultError) -> Self {
        Self::Vault(error)
    }
}

/// Persistent settings store. Reads and rewrites one JSON document, sealing
/// secrets through the supplied key vault.
#[derive(Clone)]
pub struct SettingsStore {
    vault: KeyVault,
    settings_path: std::path::PathBuf,
}

const SETTINGS_FILE_NAME: &str = "aura-settings.json";

impl SettingsStore {
    pub fn new(vault: KeyVault) -> Self {
        let settings_path = vault.data_directory().join(SETTINGS_FILE_NAME);
        Self {
            vault,
            settings_path,
        }
    }

    /// Load the stored settings, or return defaults when no file exists yet
    /// (first run) — never an error for a missing file.
    pub fn load(&self) -> Result<AssistantSettings, SettingsError> {
        match std::fs::read_to_string(&self.settings_path) {
            Ok(content) => {
                if content.trim().is_empty() {
                    return Ok(AssistantSettings::default());
                }
                serde_json::from_str(&content)
                    .map(|settings: AssistantSettings| AssistantSettings {
                        ageing_window_days: settings.ageing_window_days_bounded(),
                        ..settings
                    })
                    .map_err(|error| SettingsError::Serialization(error.to_string()))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(AssistantSettings::default())
            }
            Err(error) => Err(SettingsError::Storage(error.to_string())),
        }
    }

    /// Persist the document atomically: write to a staging file, then rename.
    fn persist(&self, settings: &AssistantSettings) -> Result<(), SettingsError> {
        let content = serde_json::to_string_pretty(settings)
            .map_err(|error| SettingsError::Serialization(error.to_string()))?;
        // The data directory is owned by the app (not created lazily by the
        // vault); ensure it exists before writing so first-run saves work.
        std::fs::create_dir_all(self.settings_path.parent().unwrap())
            .map_err(|error| SettingsError::Storage(error.to_string()))?;
        let staging_path = self.settings_path.with_extension("json.tmp");
        std::fs::write(&staging_path, &content)
            .map_err(|error| SettingsError::Storage(error.to_string()))?;
        std::fs::rename(&staging_path, &self.settings_path)
            .map_err(|error| SettingsError::Storage(error.to_string()))?;
        Ok(())
    }

    /// Apply one field-level mutation. Secret fields are sealed in place;
    /// the rendered settings document returned contains the sealed blob.
    pub fn update(&self, field: SettingField) -> Result<AssistantSettings, SettingsError> {
        let mut settings = self.load()?;
        match field {
            SettingField::BrainMode(value) => settings.brain_mode = value,
            SettingField::CloudProvider(value) => settings.cloud_provider = value,
            SettingField::GroqApiKey(plaintext) => {
                let sealed = self.seal(&plaintext)?;
                settings.groq_api_key = sealed;
            }
            SettingField::OpenAiApiKey(plaintext) => {
                let sealed = self.seal(&plaintext)?;
                settings.openai_api_key = sealed;
            }
            SettingField::OllamaHost(value) => settings.ollama_host = value,
            SettingField::SttMode(value) => settings.stt_mode = value,
            SettingField::VoiceReply(value) => settings.voice_reply = value,
            SettingField::AgeingWindowDays(days) => {
                if !(AGEING_WINDOW_MIN_DAYS..=AGEING_WINDOW_MAX_DAYS).contains(&days) {
                    return Err(SettingsError::Validation(format!(
                        "the ageing window must sit between {AGEING_WINDOW_MIN_DAYS} and \
                         {AGEING_WINDOW_MAX_DAYS} days"
                    )));
                }
                settings.ageing_window_days = days;
            }
            SettingField::AgeingEnabled(value) => settings.ageing_enabled = value,
        }
        self.persist(&settings)?;
        Ok(settings)
    }

    /// Snapshot of the current document, including which secrets are present
    /// (masked — the renderer never receives plaintext back).
    pub fn snapshot(&self) -> Result<SettingsSnapshot, SettingsError> {
        let settings = self.load()?;
        Ok(SettingsSnapshot {
            brain_mode: settings.brain_mode,
            cloud_provider: settings.cloud_provider,
            groq_key_present: settings.groq_api_key.as_sealed().is_some(),
            openai_key_present: settings.openai_api_key.as_sealed().is_some(),
            ollama_host: settings.ollama_host.clone(),
            stt_mode: settings.stt_mode,
            voice_reply: settings.voice_reply,
            ageing_window_days: settings.ageing_window_days_bounded(),
            ageing_enabled: settings.ageing_enabled,
        })
    }

    fn seal(&self, plaintext: &str) -> Result<MaybeSealed, SettingsError> {
        if plaintext.trim().is_empty() {
            return Ok(MaybeSealed::Absent);
        }
        let sealed = self.vault.seal(plaintext.as_bytes())?;
        Ok(MaybeSealed::Sealed(KeyVault::encode_sealed(&sealed)))
    }

    /// Open a secret for an in-process consumer (the STT/brain tiers). The
    /// plaintext exists only in this call's stack; it is never returned
    /// through a Tauri command.
    pub(crate) fn open_secret(
        &self,
        sealed: Option<&Vec<u8>>,
    ) -> Result<Option<String>, SettingsError> {
        match sealed {
            None => Ok(None),
            Some(raw) => {
                let envelope = KeyVault::decode_sealed(raw).map_err(SettingsError::Vault)?;
                let bytes = self.vault.open(&envelope)?;
                String::from_utf8(bytes)
                    .map(Some)
                    .map_err(|error| SettingsError::Serialization(error.to_string()))
            }
        }
    }

    /// Absolute path of the settings document (used by tests).
    #[allow(dead_code)]
    pub fn settings_path(&self) -> &Path {
        &self.settings_path
    }
}

/// Read-only mask of the settings document. Secrets are reported only as
/// presence flags; the renderer builds its "key configured" affordances
/// from these without ever touching key material.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SettingsSnapshot {
    pub brain_mode: BrainMode,
    pub cloud_provider: CloudProvider,
    pub groq_key_present: bool,
    pub openai_key_present: bool,
    pub ollama_host: String,
    pub stt_mode: SttMode,
    pub voice_reply: VoiceReply,
    pub ageing_window_days: u32,
    pub ageing_enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_vault(directory: &Path) -> KeyVault {
        KeyVault::new(directory).expect("test vault creation failed")
    }

    fn temp_directory() -> tempfile::TempDir {
        tempfile::tempdir().expect("temp dir creation failed")
    }

    #[test]
    fn defaults_are_local_first_and_privacy_preserving() {
        let defaults = AssistantSettings::default();
        assert_eq!(defaults.brain_mode, BrainMode::Auto);
        assert_eq!(defaults.stt_mode, SttMode::Auto);
        assert_eq!(defaults.voice_reply, VoiceReply::Off);
        assert!(!defaults.cloud_configured());
    }

    #[test]
    fn ageing_window_clamps_hand_edited_extremes() {
        let vault = temp_vault(temp_directory().path());
        let store = SettingsStore::new(vault);

        // Persist an out-of-range value by hand-editing the document.
        let mut settings = AssistantSettings::default();
        settings.ageing_window_days = 9_999;
        store.persist(&settings).unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(loaded.ageing_window_days_bounded(), AGEING_WINDOW_MAX_DAYS);
    }

    #[test]
    fn out_of_range_ageing_window_is_rejected_on_update() {
        let vault = temp_vault(temp_directory().path());
        let store = SettingsStore::new(vault);
        let result = store.update(SettingField::AgeingWindowDays(3));
        assert!(result.is_err());
    }

    #[test]
    fn secret_field_roundtrips_sealed_and_is_never_readable_back() {
        let directory = temp_directory();
        let vault = temp_vault(directory.path());
        let store = SettingsStore::new(vault);

        store
            .update(SettingField::GroqApiKey("gsk_test_secret".into()))
            .unwrap();

        let snapshot = store.snapshot().unwrap();
        assert!(snapshot.groq_key_present);
        // The file must not contain the plaintext anywhere.
        let file_content = std::fs::read_to_string(store.settings_path()).unwrap();
        assert!(!file_content.contains("gsk_test_secret"));

        // But an in-process consumer can still open it for the brain/STT tier.
        let settings = store.load().unwrap();
        let opened = store.open_secret(settings.groq_key()).unwrap();
        assert_eq!(opened, Some("gsk_test_secret".to_string()));
    }

    #[test]
    fn empty_key_clears_the_secret() {
        let directory = temp_directory();
        let vault = temp_vault(directory.path());
        let store = SettingsStore::new(vault);

        store
            .update(SettingField::OpenAiApiKey("sk_test".into()))
            .unwrap();
        assert!(store.snapshot().unwrap().openai_key_present);

        store
            .update(SettingField::OpenAiApiKey(String::new()))
            .unwrap();
        assert!(!store.snapshot().unwrap().openai_key_present);
    }

    #[test]
    fn snapshot_hides_cloud_provider_key_when_absent() {
        let vault = temp_vault(temp_directory().path());
        let store = SettingsStore::new(vault);

        let mut settings = AssistantSettings::default();
        settings.cloud_provider = CloudProvider::OpenAi;
        store.persist(&settings).unwrap();

        let snapshot = store.snapshot().unwrap();
        assert_eq!(snapshot.cloud_provider, CloudProvider::OpenAi);
        assert!(!snapshot.openai_key_present);
    }

    #[test]
    fn file_rewrite_is_atomic_through_a_staging_copy() {
        let directory = temp_directory();
        let vault = temp_vault(directory.path());
        let store = SettingsStore::new(vault);

        store
            .update(SettingField::BrainMode(BrainMode::Local))
            .unwrap();
        assert!(!directory.path().join("aura-settings.json.tmp").exists());
        assert!(directory.path().join("aura-settings.json").exists());
    }
}
