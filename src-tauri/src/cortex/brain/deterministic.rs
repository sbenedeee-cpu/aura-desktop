// EXP-007: the deterministic floor — a keyword/intent parser that never
// dead-ends.
//
// This is the tier of last resort (PRD §5.1): when no LLM is reachable,
// Cortex still answers the small set of intents it can parse reliably from
// the transcript alone. The v1 registry is deliberately narrow; EXP-008
// grows the typed tool registry and this floor extends with it.
//
// Design note: patterns are lowercase-prefix matches on the trimmed
// transcript. No regex crate is added for v1 — the full list lives here and
// the surface is small enough that explicit branches read better than a
// regex table.

use serde::{Deserialize, Serialize};

/// Intents the deterministic floor recognizes in v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeterministicIntent {
    /// "note …" / "remember …" / "save this: …" → capture the rest as a note.
    SaveNote { content: String },
    /// "what did I …" / "find …" / "search my …" → read-only memory query.
    QueryMemory { query: String },
    /// "who are you" / "what is aura" → identity, no data needed.
    Identity,
    /// "what time is it" / "time" → wall-clock answer, no data needed.
    CurrentTime,
    /// Nothing matched; the caller should surface a graceful fallback line.
    Unrecognized,
}

/// The floor's answer: an intent plus a self-contained reply the overlay can
/// render verbatim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeterministicResult {
    pub intent: DeterministicIntent,
    pub reply: String,
}

fn trimmed_lower(text: &str) -> String {
    text.trim().to_lowercase()
}

/// Recognize a deterministic intent from the transcript. The first matching
/// branch wins, so higher-specificity patterns (notes with markers) sit
/// above generic queries.
pub fn parse_intent(transcript: &str) -> DeterministicIntent {
    let lower = trimmed_lower(transcript);
    if lower.is_empty() {
        return DeterministicIntent::Unrecognized;
    }

    // Note capture: explicit markers beat the generic query branch.
    for marker in ["note", "remember", "save this", "capture"] {
        if let Some(content) = lower.strip_prefix(marker) {
            let content = content
                .trim()
                .trim_start_matches([':', ' ', '-'])
                .to_string();
            if !content.is_empty() {
                return DeterministicIntent::SaveNote { content };
            }
        }
    }

    // Read-only memory queries.
    for trigger in ["what did i", "find", "search my", "look up my", "show my"] {
        if let Some(query) = lower.strip_prefix(trigger) {
            let query = query.trim().to_string();
            return DeterministicIntent::QueryMemory { query };
        }
    }

    if lower.contains("who are you")
        || lower.starts_with("what is aura")
        || lower.starts_with("what are you")
    {
        return DeterministicIntent::Identity;
    }

    if lower.starts_with("time") || lower.starts_with("what time") || lower.contains("current time")
    {
        return DeterministicIntent::CurrentTime;
    }

    DeterministicIntent::Unrecognized
}

/// Turn a recognized intent into the floor's reply. Pure and side-effect
/// free so it can be exercised in a unit table.
pub fn floor_reply(intent: &DeterministicIntent) -> String {
    match intent {
        DeterministicIntent::SaveNote { content } => {
            format!("Cortex captured a note: “{content}” — saved to your local memory.")
        }
        DeterministicIntent::QueryMemory { query } => {
            format!("Cortex would search your memory for “{query}” — the memory tools arrive in the next build.")
        }
        DeterministicIntent::Identity => {
            "I'm Aura — your Neural Cortex. Local-first by default, cloud when you opt in.".into()
        }
        DeterministicIntent::CurrentTime => {
            format!(
                "It's {} on your machine.",
                chrono::Local::now().format("%H:%M")
            )
        }
        DeterministicIntent::Unrecognized => {
            "Cortex heard you, but that's not something it can answer yet — the full brain ships in this build; try a simpler phrasing or bring ollama online.".into()
        }
    }
}

/// Run the full deterministic floor: parse, then reply.
pub fn run_floor(transcript: &str) -> DeterministicResult {
    let intent = parse_intent(transcript);
    let reply = floor_reply(&intent);
    DeterministicResult { intent, reply }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_markers_capture_the_remainder() {
        let intent = parse_intent("Note: finish the EXP-007 design before Friday");
        assert_eq!(
            intent,
            DeterministicIntent::SaveNote {
                content: "finish the exp-007 design before friday".into(),
            }
        );

        let intent = parse_intent("Remember to call the bank tomorrow");
        assert!(matches!(intent, DeterministicIntent::SaveNote { .. }));
    }

    #[test]
    fn query_triggers_are_read_only() {
        let intent = parse_intent("What did I note about the voice pipeline?");
        assert!(matches!(intent, DeterministicIntent::QueryMemory { .. }));

        let intent = parse_intent("Search my projects");
        assert!(matches!(intent, DeterministicIntent::QueryMemory { .. }));
    }

    #[test]
    fn identity_and_time_have_no_data_dependency() {
        let intent = parse_intent("Who are you?");
        assert_eq!(intent, DeterministicIntent::Identity);

        let intent = parse_intent("What time is it");
        assert_eq!(intent, DeterministicIntent::CurrentTime);
    }

    #[test]
    fn empty_and_unknown_inputs_never_crash() {
        assert_eq!(parse_intent(""), DeterministicIntent::Unrecognized);
        assert_eq!(parse_intent("   "), DeterministicIntent::Unrecognized);
        assert_eq!(
            parse_intent("make me a sandwich"),
            DeterministicIntent::Unrecognized
        );
    }

    #[test]
    fn floor_reply_is_always_non_empty() {
        let cases = [
            "",
            "note something",
            "what did I save last week",
            "who are you",
            "time",
            "unrecognized input xyz",
        ];
        for transcript in cases {
            let result = run_floor(transcript);
            assert!(!result.reply.is_empty());
        }
    }
}
