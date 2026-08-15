# AURA JARVIS — Architecture & Implementation Roadmap

**Status:** Approved for implementation (Eternal, Aug 15 2026)
**Scope:** v1 hybrid (local-first + optional cloud) hotkey-activated AI assistant built on the existing Aura Desktop codebase
**Author:** Manus AI (Architecture) — reviewed intent with Eternal

---

## 1. Vision and Non-Negotiables

Aura becomes a **Jarvis-class personal assistant**: press a hotkey anywhere on Windows, a small overlay wakes, you speak or type, it understands, acts, and speaks back. The existing memory system (captures, projects, retention, export) becomes the assistant's **long-term memory module** — the piece that makes Aura different from generic chatbots, because it answers from *your* private context, not a generic knowledge base.

Five non-negotiables carry over from the privacy mandate and govern every design decision below:

1. **Local-first.** Nothing leaves the machine unless the user explicitly configures a cloud brain, and even then only the specific request payload travels, with secrets stored only in the encrypted local store.
2. **The user decides.** Every action with an external effect (delete, send, modify) requires explicit confirmation. The assistant never silently acts on the user's behalf.
3. **Offline-capable core.** The assistant must degrade gracefully, not fail, when the network is down or no cloud key is configured.
4. **One data store.** The SQLite database from the capture era remains the single source of truth; the assistant reads and writes the same tables.
5. **Every increment ships through CI.** The three required merge gates (Renderer quality, Native quality, Tauri Windows build) remain mandatory; no feature reaches `main` without a passing Windows binary.

## 2. System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Windows desktop                                            │
│                                                             │
│  ┌─ GLOBAL HOTKEY (Alt+Space, tauri-plugin-global-shortcut) │
│  │   ▼                                                      │
│  │  ┌─────────────────────────────────────────────────┐     │
│  │  │ OVERLAY WINDOW (React)                          │     │
│  │  │   • Voice button (mic) → record via Web Audio   │     │
│  │  │   • Text input fallback                         │     │
│  │  │   • Transcript + assistant reply                │     │
│  │  │   • Confirmation prompts for actions            │     │
│  │  └─────────────────────────────────────────────────┘     │
│  │        │ invoke Tauri commands                            │
│  │        ▼                                                  │
│  │  ┌─────────────────────────────────────────────────┐     │
│  │  │ RUST CORE                                       │     │
│  │  │                                                 │     │
│  │  │  VoicePipeline  ── STT ──► Text                │     │
│  │  │    • local:  whisper-rs (whisper.cpp)          │     │
│  │  │    • cloud:  Groq/OpenAI Whisper (optional)    │     │
│  │  │                                                 │     │
│  │  │  Brain        ── intent + plan ──► Tools       │     │
│  │  │    • local:  ollama REST (tool-calling models) │     │
│  │  │    • cloud:  OpenAI/Gemini API (optional key)  │     │
│  │  │                                                 │     │
│  │  │  ActionRegistry (typed Rust tools)             │     │
│  │  │    • query_memory, save_note, list_projects    │     │
│  │  │    • create_reminder, check_reminders          │     │
│  │  │    • web_search (HTTP, minimal scope)          │     │
│  │  │                                                 │     │
│  │  │  TTS ──► speech                              │     │
│  │  │    • local:  Windows SAPI (free, offline)      │     │
│  │  │    • fallback: Web Speech API in the webview   │     │
│  │  └─────────────────────────────────────────────────┘     │
│  │        │                                                  │
│  │        ▼                                                  │
│  │  ┌──────────┐  ┌───────────┐  ┌───────────────────┐      │
│  │  │ SQLite   │  │ Key Vault │  │ Settings store    │      │
│  │  │ captures │  │ (DPAPI)   │  │ (brain choice,    │      │
│  │  │ projects │  │ API keys  │  │  hotkey, voice on)│      │
│  │  │ reminders│  │           │  │                   │      │
│  │  └──────────┘  └───────────┘  └───────────────────┘      │
└─────────────────────────────────────────────────────────────┘
```

## 3. Module-by-Module Design

### 3.1 Hotkey & Overlay (EXP-005)

The `tauri-plugin-global-shortcut` plugin registers `Alt+Space` globally on Windows (works even when Aura is not focused) [1]. The handler locates or creates an always-on-top, undecorated overlay window and brings it to front with focus on the input. Overlay UI (React) keeps the existing design system so Aura still feels like one product. Voice is **push-to-talk** (hold mic / click to record), never always-listening — always-listening wake words are explicitly out of scope for v1 as a privacy decision.

### 3.2 Voice Pipeline (EXP-006)

Recording happens in the webview via the MediaRecorder API (PCM/WAV), passed to Rust as bytes. STT resolves in priority order: **cloud Whisper** if a key is configured and online (Groq Whisper API is free and fast — the budget pick; OpenAI Whisper API at ~$0.006/min is the fallback), otherwise **local whisper-rs** [2] with a small base/small model bundled or downloaded once at first use. Local inference is slower on CPU-only machines (small model: roughly 1–5 s on mid-range hardware), which is acceptable for v1 and improves with hardware. The same preference pattern as the memory module applies: settings store decides, user controls.

### 3.3 Brain (EXP-007)

The brain is a single `execute_intent(text, context)` Rust function that resolves to either a local or cloud LLM based on the settings. Local mode calls **ollama's REST API** on `localhost:11434` [3], which supports tool/function calling natively — so the model itself proposes which tool to run and with what arguments [4]. Recommended models: **Llama 3.2 3B / Qwen 2.5 3B** for modest machines, **Llama 3.1 8B** if the user has 16 GB+ RAM. Cloud mode uses OpenAI `gpt-4o-mini` or Gemini (generous free tier) with the API key stored in the existing DPAPI-backed key vault. If neither is available, the app falls back to a **deterministic command parser** (simple keyword/intent rules over the tool registry) — the assistant never dead-ends.

Context sent to any remote brain is deliberately minimal: the current transcript, recent session notes, and the tool schema. Raw memory contents are queried locally; only retrieval *results* (not the full database) travel, and only in cloud mode.

### 3.4 Action Registry (EXP-008)

Tools are plain typed Rust functions registered with a name, description, and JSON schema; the LLM's tool-calling output maps directly onto them. v1 registry:

| Tool | Data source | Side effect | Confirm required |
|---|---|---|---|
| `query_memory` | captures SQL | none (read) | no |
| `save_note` | captures insert | write | shown in reply, undo-able |
| `list_projects` / `update_project` | projects | write | only on status changes |
| `create_reminder` / `check_reminders` | new `reminders` table | write + timer | no (create), yes (fire) |
| `web_search` | local HTTPS fetch | none | no |
| `delete_item` | captures/projects | destructive | **always, explicit** |

A new `reminders` table (Migration 7) stores due timestamps; a lightweight in-app scheduler polls on a 30-second heartbeat and fires reminders through the TTS channel even when the overlay is closed.

### 3.5 Speech Out (EXP-009)

Text-to-speech uses **Windows SAPI** (free, offline, no downloads) via `windows-sys` bindings — the same technique proven by SEC-001's DPAPI work, same dependency already in the project. Web Speech API in the webview remains a zero-code fallback. Voice output is toggleable per user preference.

## 4. Data Model Additions

Migration 7 adds a single `reminders` table (`id`, `project_id`, `title`, `due_at`, `fired_at`, `created_at`). Brain configuration lives in the existing `settings` key-value store (`brain_mode` ∈ {auto, local, cloud}, `cloud_provider`, `stt_mode`, `voice_enabled`, `hotkey`). No new cloud column in captures; the memory schema is untouched — the assistant reads what already exists.

## 5. Privacy Controls Summary

| Surface | Local mode | Cloud mode |
|---|---|---|
| Voice recording | processed on-device (whisper-rs) | audio bytes sent to Whisper API only |
| Conversation | never leaves device | transcript + minimal context per request |
| API keys | DPAPI-encrypted at rest | never sent except as auth header |
| Tool results | local SQLite only | retrieval results only, never raw DB |
| Offline | fully functional | graceful degradation to local/deterministic |

## 6. Build Sequence (roadmap increments)

Each increment is a PR with the three CI gates; each ends with a fresh Windows binary for friction testing. EXP-004 (configurable ageing window) is **merged into EXP-007's settings work** rather than shipped alone, to avoid a half-baked settings UI.

| Increment | What ships | Estimate |
|---|---|---|
| **EXP-005** | Global hotkey + overlay window + echo test (typing only) | ~1–2 days |
| **EXP-006** | Voice pipeline: local whisper-rs + optional cloud STT | ~3–4 days |
| **EXP-007** | Brain: ollama local + cloud provider + deterministic fallback + settings UI (absorbs EXP-004) | ~4–5 days |
| **EXP-008** | Action registry + reminders table + scheduler | ~4–5 days |
| **EXP-009** | SAPI speech-out + toggle preferences + polish | ~2–3 days |
| **EXP-010** | Full friction-test week of the assembled assistant; debrief → EXP-011 | 1 week |

Realistic total: **2–3 weeks of focused work**, sequenced so each PR is independently demoable. EXP-005 is the first — it produces the Jarvis "feel" (hotkey → popup → reply) immediately, and everything after plugs into it.

## 7. Risks and Mitigations

**Windows binary size and build time.** whisper-rs compiles whisper.cpp natively, adding minutes to CI and ~100–500 MB to the bundle. Mitigation: ship local STT as an optional on-demand download (first use fetches the model), keep the base binary small, and gate whisper-rs behind a Cargo feature so the default CI build stays fast. **Local model quality on modest hardware.** Mitigation: 3B-class models are the floor; cloud mode covers quality-sensitive users; deterministic fallback covers the zero-setup case. **Ollama dependency.** Mitigation: the app detects ollama at runtime and shows a one-click "install helper" screen with the download link — never silently fails; cloud/deterministic modes keep the product usable without it. **Code signing (SmartScreen).** Unchanged from the capture era: unsigned binary, documented workaround, deferrable until real distribution.

## 8. Decision Record References

This document supersedes the EXP-004 roadmap item and becomes the pivot anchor. A new ADR (`ADR-007-jarvis-pivot`) will be filed with the EXP-005 PR documenting: hotkey activation as the primary interface, local-first hybrid brain, push-to-talk voice (no always-listening), and preservation of the existing memory store as the assistant's long-term memory.

## References

[1]: https://v2.tauri.app/plugin/global-shortcut/ "Tauri v2 — Global Shortcut plugin"
[2]: https://crates.io/crates/whisper-rs "crates.io — whisper-rs (Rust bindings for whisper.cpp)"
[3]: https://ollama.com/ "Ollama — local LLM runner with REST API"
[4]: https://docs.ollama.com/capabilities/tool-calling "Ollama documentation — Tool calling"

- [1] Tauri v2 Global Shortcut plugin — https://v2.tauri.app/plugin/global-shortcut/
- [2] whisper-rs on crates.io — https://crates.io/crates/whisper-rs
- [3] Ollama — https://ollama.com/
- [4] Ollama tool calling docs — https://docs.ollama.com/capabilities/tool-calling
