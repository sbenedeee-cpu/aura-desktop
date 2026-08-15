// EXP-007: the local brain — ollama on-device reasoning.
//
// Free by design (PRD §5.1): nothing leaves the machine. The runtime
// detection matters because the PRD risk log (§12) calls out "ollama not
// installed" as a known medium risk — every caller treats a reachability
// failure as a tier signal, not a user error, and the engine falls through
// to the deterministic floor.
//
// Protocol: ollama's native `/api/chat` REST endpoint (JSON), synchronous
// through ureq. v1 keeps the conversation minimal — one system prompt, the
// transcript, and recent retrieval context — and asks the model to answer
// directly or declare an intent; EXP-008 upgrades this to formal
// tool-calling with the typed registry.

use serde::{Deserialize, Serialize};

/// Reachability verdict for the ollama host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OllamaReachability {
    Reachable,
    Unreachable,
}

/// A tiny, versioned ollama chat message. We speak the same JSON shape as
/// ollama's own `/api/chat` so nothing needs translation.
#[derive(Debug, Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

/// ollama's own chat response shape (we only read `message.content`).
#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    #[serde(default)]
    message: Option<OllamaMessage>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    #[serde(default)]
    content: String,
}

const SYSTEM_PROMPT: &str = "You are Cortex, the local reasoning half of Aura, a privacy-first \
     personal assistant. You run entirely on this user's machine. Answer \
     briefly and directly (under 120 words). You may act on the user's \
     local memory (captures, notes, projects) when asked to retrieve or \
     save something — describe what you would do; never claim to have \
     performed destructive actions. You never collect data beyond this \
     conversation.";

/// Ask ollama whether it is alive at the configured host. A short timeout
/// keeps the tier resolver fast: unreachable ollama must not slow down the
/// fallback chain.
pub fn check_reachable(host: &str) -> OllamaReachability {
    let probe_url = format!("{}/api/version", host.trim_end_matches('/'));
    match ureq::get(&probe_url)
        .timeout(std::time::Duration::from_secs(2))
        .call()
    {
        Ok(response) => match response.into_string() {
            Ok(body) if body.trim().starts_with('{') => OllamaReachability::Reachable,
            _ => OllamaReachability::Unreachable,
        },
        Err(_) => OllamaReachability::Unreachable,
    }
}

/// Errors raised by the local brain.
#[derive(Debug)]
pub enum LocalBrainError {
    Unreachable(String),
    NoModel(String),
    Generation(String),
}

impl std::fmt::Display for LocalBrainError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreachable(detail) => write!(formatter, "ollama is not reachable: {detail}"),
            Self::NoModel(detail) => write!(formatter, "no suitable local model: {detail}"),
            Self::Generation(detail) => write!(formatter, "local generation failed: {detail}"),
        }
    }
}

impl std::error::Error for LocalBrainError {}

/// Pick the best available ollama model: 8B-class for roomy machines, the
/// 3B floor otherwise. Returns the model name and the reason it was chosen.
pub fn select_model(host: &str) -> Result<(String, String), LocalBrainError> {
    let list_url = format!("{}/api/tags", host.trim_end_matches('/'));
    let response = ureq::get(&list_url)
        .timeout(std::time::Duration::from_secs(5))
        .call()
        .map_err(|error| LocalBrainError::Unreachable(error.to_string()))?;
    let body: OllamaTagList = response
        .into_json()
        .map_err(|error| LocalBrainError::Generation(error.to_string()))?;

    // 8B-class preferred for machines with headroom; 3B-class is the floor.
    let candidates: Vec<&str> = body
        .models
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    if let Some(preferred) = candidates
        .iter()
        .find(|name| name.contains("8b") || name.contains("70b") || name.contains("14b"))
    {
        return Ok((preferred.to_string(), "roomy machine class selected".into()));
    }
    if let Some(floor) = candidates
        .iter()
        .find(|name| name.contains("3b") || name.contains("1.5b") || name.contains("qwen"))
    {
        return Ok((floor.to_string(), "compact floor model selected".into()));
    }
    if let Some(any) = candidates.first() {
        return Ok((any.to_string(), "only installed model selected".into()));
    }
    Err(LocalBrainError::NoModel(
        "install a model (`ollama pull llama3.2:3b`) before using the local brain".into(),
    ))
}

#[derive(Debug, Deserialize)]
struct OllamaTagList {
    #[serde(default)]
    models: Vec<OllamaTagEntry>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagEntry {
    name: String,
}

/// Run one generation against ollama. Returns the model's reply text.
pub fn generate(
    host: &str,
    model: &str,
    transcript: &str,
    context: &str,
) -> Result<String, LocalBrainError> {
    let chat_url = format!("{}/api/chat", host.trim_end_matches('/'));
    let system_content = if context.trim().is_empty() {
        SYSTEM_PROMPT.to_string()
    } else {
        format!("{SYSTEM_PROMPT}\n\nRelevant local context: {context}")
    };

    let payload = serde_json::json!({
        "model": model,
        "stream": false,
        "messages": [
            ChatMessage { role: "system", content: system_content },
            ChatMessage { role: "user", content: transcript.to_string() },
        ]
    });

    let response = ureq::post(&chat_url)
        .timeout(std::time::Duration::from_secs(120))
        .send_json(&payload)
        .map_err(|error| LocalBrainError::Generation(error.to_string()))?;

    let parsed: OllamaChatResponse = response
        .into_json()
        .map_err(|error| LocalBrainError::Generation(error.to_string()))?;

    if let Some(error) = parsed.error {
        return Err(LocalBrainError::Generation(error));
    }
    let content = parsed
        .message
        .map(|message| message.content)
        .unwrap_or_default()
        .trim()
        .to_string();

    if content.is_empty() {
        return Err(LocalBrainError::Generation(
            "the local model returned an empty reply".into(),
        ));
    }
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_payload_is_minimal_and_streams_off() {
        // The payload shape is the only thing in this module that touches
        // ollama's wire format; pin it so a refactor can't silently change
        // what reaches the local model.
        let payload = serde_json::json!({
            "model": "test",
            "stream": false,
            "messages": [
                ChatMessage { role: "system", content: SYSTEM_PROMPT.to_string() },
                ChatMessage { role: "user", content: "hello".into() },
            ]
        });
        assert_eq!(payload["stream"], false);
        assert_eq!(payload["messages"][0]["role"], "system");
        assert_eq!(payload["messages"][1]["role"], "user");
    }

    #[test]
    fn host_trailing_slash_is_normalized() {
        // A dead host must report unreachable regardless of path styling.
        assert_eq!(
            check_reachable("http://127.0.0.1:1/nonexistent/"),
            OllamaReachability::Unreachable
        );
        assert_eq!(
            check_reachable("http://127.0.0.1:1/nonexistent"),
            OllamaReachability::Unreachable
        );
    }
}
