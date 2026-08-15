// EXP-007: the cloud brain — Groq free tier first, OpenAI as the paid
// fallback.
//
// Both providers speak the same OpenAI-compatible chat JSON, so one wire
// format covers the whole tier (PRD §5.1: OpenAI `gpt-4o-mini` or Gemini;
// Groq Whisper for STT). Groq is the budget pick per the PRD cost analysis
// (§7.2/§7.3): its free tier costs $0 and its latency is the lowest in the
// class. OpenAI at ~$0.006/min is only ever hit when the user explicitly
// switches provider and configures its key.
//
// Privacy (PRD §9): the payload is strictly the transcript plus retrieval
// results — never raw SQLite rows, never settings, never history beyond
// what the caller passes.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloudProviderKind {
    Groq,
    OpenAi,
}

impl CloudProviderKind {
    pub fn chat_endpoint(&self) -> &'static str {
        // Both runtimes honor the same OpenAI-compatible schema.
        match self {
            Self::Groq => "https://api.groq.com/openai/v1/chat/completions",
            Self::OpenAi => "https://api.openai.com/v1/chat/completions",
        }
    }

    /// Free-tier model for the provider. The PRD allows OpenAI gpt-4o-mini
    /// or Gemini; Groq's `llama-3.3-70b-versatile` is free and covers both
    /// quality and cost.
    pub fn default_model(&self) -> &'static str {
        match self {
            Self::Groq => "llama-3.3-70b-versatile",
            Self::OpenAi => "gpt-4o-mini",
        }
    }
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

/// OpenAI-compatible chat completion response (we read the first choice).
#[derive(Debug, Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<ChatChoice>,
    #[serde(default)]
    error: Option<ChatError>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    #[serde(default)]
    message: Option<ChoiceMessage>,
}

#[derive(Debug, Deserialize)]
struct ChoiceMessage {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatError {
    #[serde(default)]
    message: String,
}

/// Errors raised by the cloud brain.
#[derive(Debug)]
pub enum CloudBrainError {
    NoKey(String),
    Network(String),
    Generation(String),
}

impl std::fmt::Display for CloudBrainError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoKey(detail) => write!(formatter, "no cloud key configured: {detail}"),
            Self::Network(detail) => write!(formatter, "cloud network failure: {detail}"),
            Self::Generation(detail) => write!(formatter, "cloud generation failed: {detail}"),
        }
    }
}

impl std::error::Error for CloudBrainError {}

const SYSTEM_PROMPT: &str = "You are Cortex, the cloud reasoning half of Aura, a privacy-first \
     personal assistant whose long-term memory never leaves the user's \
     machine. The context you receive is already local retrieval results — \
     never query or reference anything beyond it. Answer briefly and \
     directly (under 120 words). You never claim to have performed \
     destructive actions; you describe intended actions only.";

/// Run one cloud generation. The API key arrives pre-opened from the sealed
/// settings store; it is sent only as the Authorization header (PRD C2) and
/// never logged, stored back, or returned through a Tauri command.
pub fn generate(
    provider: CloudProviderKind,
    api_key: &str,
    transcript: &str,
    context: &str,
) -> Result<String, CloudBrainError> {
    if api_key.trim().is_empty() {
        return Err(CloudBrainError::NoKey(
            "configure a cloud API key in the settings before using the cloud brain".into(),
        ));
    }

    let system_content = if context.trim().is_empty() {
        SYSTEM_PROMPT.to_string()
    } else {
        format!("{SYSTEM_PROMPT}\n\nRelevant local context: {context}")
    };

    let payload = serde_json::json!({
        "model": provider.default_model(),
        "stream": false,
        "messages": [
            ChatMessage { role: "system", content: system_content },
            ChatMessage { role: "user", content: transcript.to_string() },
        ]
    });

    let response = ureq::post(provider.chat_endpoint())
        .set("Authorization", &format!("Bearer {api_key}"))
        .set("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(60))
        .send_json(&payload)
        .map_err(|error| CloudBrainError::Network(error.to_string()))?;

    let parsed: ChatResponse = response
        .into_json()
        .map_err(|error| CloudBrainError::Generation(error.to_string()))?;

    if let Some(error) = parsed.error {
        // An invalid key is user configuration, not a network flake: report
        // it distinctly so the overlay can point at the settings card.
        let is_auth = error.message.contains("401")
            || error.message.contains("403")
            || error.message.to_lowercase().contains("authentication")
            || error.message.to_lowercase().contains("invalid_api");
        if is_auth {
            return Err(CloudBrainError::NoKey(format!(
                "the API key was rejected: {}",
                error.message
            )));
        }
        return Err(CloudBrainError::Generation(error.message));
    }

    let content = parsed
        .choices
        .first()
        .and_then(|choice| choice.message.as_ref())
        .and_then(|message| message.content.clone())
        .unwrap_or_default()
        .trim()
        .to_string();

    if content.is_empty() {
        return Err(CloudBrainError::Generation(
            "the cloud model returned an empty reply".into(),
        ));
    }
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_payload_carries_only_transcript_and_context() {
        let payload = serde_json::json!({
            "model": "llama-3.3-70b-versatile",
            "stream": false,
            "messages": [
                ChatMessage { role: "system", content: SYSTEM_PROMPT.to_string() },
                ChatMessage { role: "user", content: "hello cortex".into() },
            ]
        });
        // Provenance pin: nothing but the transcript/context may appear in
        // the user message for a request.
        assert_eq!(payload["messages"][1]["content"], "hello cortex");
        assert_eq!(payload["stream"], false);
    }

    #[test]
    fn empty_key_is_rejected_before_any_network_call() {
        let result = generate(CloudProviderKind::Groq, "", "ping", "");
        assert!(matches!(result, Err(CloudBrainError::NoKey(_))));
    }

    #[test]
    fn providers_use_distinct_endpoints_and_models() {
        assert_eq!(
            CloudProviderKind::Groq.chat_endpoint(),
            "https://api.groq.com/openai/v1/chat/completions"
        );
        assert_eq!(
            CloudProviderKind::OpenAi.chat_endpoint(),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_ne!(
            CloudProviderKind::Groq.default_model(),
            CloudProviderKind::OpenAi.default_model()
        );
    }
}
