# ADR-007: Jarvis pivot — hotkey assistant interface and Neural Cortex system

**Status:** Proposed (Aug 15, 2026)
**Context:** EXP-005 PR — Aura Jarvis architecture (PR #16) approved by Eternal

## Decision

Aura Desktop pivots from a capture/memory application to a **Jarvis-class AI assistant**. The primary interface becomes a **global hotkey (Alt+Space)** that summons a compact overlay; the assistant accepts voice (push-to-talk) or text input, reasons over the user's private context, executes tools, and replies by speech and text.

The reasoning engine and its memory database ship as one branded system, **Neural Cortex**, with two named halves:

- **Cortex Reasoning** — the physical render of the brain: the STT → intent → LLM tool-calling → TTS pipeline, operating in local (ollama), cloud (API key), or deterministic-fallback mode.
- **Cortex Memory** — the database half: the existing SQLite store (captures, projects, reminders, settings), living in `src-tauri/src/cortex/`.

**Capture remains a first-class core feature**, equal to the assistant: manual capture, retention review, and passphrase export are permanent product surface, and capture is additionally available by voice through the assistant (`save_note` feeds Cortex Memory directly).

## Consequences

1. The roadmap shifts from EXP-004 (configurable ageing window) to the EXP-005–EXP-010 assistant sequence; EXP-004's settings work is absorbed by EXP-007.
2. New dependency surface: `tauri-plugin-global-shortcut` (hotkey), `whisper-rs` (local STT, gated behind a Cargo feature), SAPI bindings (TTS, via existing `windows-sys`), and optional runtime ollama / cloud API keys.
3. Privacy model extends rather than changes: local-first is unchanged; cloud mode sends only the per-request transcript and minimal context; nothing leaves the machine without user intent.
4. Always-listening wake words are explicitly **out of scope for v1** (privacy decision); push-to-talk is the voice activation model.
5. Every increment continues to ship through the three required merge gates (Renderer quality, Native quality, Tauri Windows build).

## Supersedes

The EXP-004 roadmap item (configurable ageing window) as a standalone increment; its settings work is merged into EXP-007.
