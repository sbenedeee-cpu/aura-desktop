// EXP-007: the Brain — Neural Cortex reasoning path.
//
// A single `execute_intent` entry point resolves the reasoning tier from the
// settings store at request time (PRD §5.1) and degrades tier-by-tier so the
// offline-capable core law (PRD §4.3) never dead-ends:
//
//   auto  → cloud when a key is configured AND online
//         → local ollama when reachable
//         → deterministic keyword floor (always available)
//   local → ollama, then deterministic
//   cloud → provider, then deterministic
//
// The context handed to any remote brain is deliberately minimal: the
// current transcript plus recent retrieval results — never the raw SQLite
// database (PRD §5.1). API keys are opened from the sealed settings store
// in-process and never travel through a Tauri command.
//
// Module layout:
//   engine        — public dispatcher (execute_intent)
//   local         — ollama REST chat + tool-calling (free, on-device)
//   cloud         — Groq / OpenAI REST (OpenAI-compatible schemas)
//   deterministic — keyword/intent floor (never dead-ends)

pub mod cloud;
pub mod deterministic;
pub mod engine;
pub mod local;

pub use engine::{execute_intent, probe_status, BrainContext, BrainResult, BrainStatus};
