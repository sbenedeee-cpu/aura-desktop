# Aura — Product Requirements Document (Jarvis PRD)

| Field | Value |
|---|---|
| **Product** | Aura — a Jarvis-class personal AI assistant for Windows |
| **Platform** | Windows 10/11 desktop (Tauri 2 / React 19 / Rust) |
| **Brain system** | Neural Cortex (Cortex Reasoning + Cortex Memory) |
| **Document status** | Approved source of truth (v1.0, Aug 15 2026) |
| **Author** | Manus AI — architecture and PM |
| **Owner / approver** | Eternal (founder, sole v1 user) |
| **Supersedes** | `docs/AURA_V0_PRD_FOR_ANTIGRAVITY.md`; governs increments EXP-005 onward |
| **Anchor documents** | `docs/architecture/AURA_JARVIS_ARCHITECTURE.md`; ADR-001 through ADR-007 |

---

## 1. Vision

Aura is a **Jarvis-class personal assistant**: press a hotkey anywhere on Windows, a small overlay wakes, you speak or type, and it understands, acts, and speaks back. What makes Aura different from every generic chatbot is that it answers from **your private context** — your captures, your projects, your decisions, your reminders — stored entirely on your machine. The assistant does not open with a knowledge base; it opens with *you*.

The existing capture-and-memory application is not replaced by this pivot; it is absorbed. Manual capture, project continuity, retention review, and encrypted export become the memory half of the assistant's brain. Capture is a first-class core feature on equal footing with the assistant, and remains fully usable by keyboard and mouse — the assistant is an additional way to reach everything Capture already does.

## 2. Problem Statement

Personal AI assistants today present a forced choice: powerful but cloud-bound (everything you say and see travels to a vendor), or private but useless (local tools that cannot reason). Meanwhile, project work collapses when context is lost — a person resumes work by reconstructing the "why" from fragments, which no generic assistant can do because the context lives on the person's machine.

Aura's answer is a **hybrid local-first assistant**: a reasoning brain that runs locally by default, upgrades to cloud quality only when the user configures it, and is grounded in a memory store that never leaves the machine.

## 3. Target User

The v1 user is a single power user — the founder, Eternal — running Aura daily on real Windows work context: student coursework at Lagos State University, studio design work, church media, and multi-project coordination. The v1 feedback loop is a structured **friction-test week** at the end of each major increment: run the assistant on real work for a week, log what hurt, and turn the debrief into the next increment's spec.

| User | Role | Need |
|---|---|---|
| Eternal | Founder + sole v1 user | A daily-use assistant that knows his real context, respects his privacy, and ships through real builds |
| (Future) power users | Expansion, not v1 | Same contract: local-first, user-decides, offline-capable |

## 4. Non-Negotiables (Product Laws)

These five laws carry over from the privacy mandate and govern every decision in this document. Any proposed feature that violates one is rejected, not negotiated:

1. **Local-first.** Nothing leaves the machine unless the user explicitly configures a cloud brain — and even then, only the specific request payload travels. Secrets are stored only in the encrypted local store.
2. **The user decides.** Every action with an external effect (delete, send, modify) requires explicit confirmation. The assistant never silently acts on the user's behalf.
3. **Offline-capable core.** The assistant degrades gracefully, never dead-ends, when the network is down or no cloud key is configured.
4. **One data store.** The existing SQLite database remains the single source of truth; the assistant reads and writes the same tables as the capture app.
5. **Every increment ships through CI.** Renderer quality, Native quality, and the Tauri Windows build are mandatory merge gates on `main`; no feature reaches `main` without a passing Windows binary.

## 5. The Brain: Neural Cortex

The reasoning engine and its memory database ship as a single branded system called **Neural Cortex**. Two names, one unit.

| Half | What it is | What it does |
|---|---|---|
| **Cortex Reasoning** | The physical render of the brain | The full pipeline: voice in → intent → LLM tool-calling → speech out, running in local, cloud, or deterministic mode |
| **Cortex Memory** | The database half | The existing SQLite store: captures, projects, decisions, reminders, settings — the assistant's long-term memory |

From the user's perspective, Cortex is what Aura *is*; everything else (hotkey, overlay, voice pipeline) is the interface through which Cortex is summoned. All brain code lives under `src-tauri/src/cortex/`.

### 5.1 Modes of reasoning

Cortex Reasoning operates in three tiers, resolved by the settings store at request time:

| Tier | Provider | Privacy | Cost | Fallback |
|---|---|---|---|---|
| **Local** | ollama on `localhost:11434` (Llama 3.2 3B / Qwen 2.5 3B floor; Llama 3.1 8B for 16 GB+ machines) | Nothing leaves the machine | Free (electricity) | — |
| **Cloud** | OpenAI `gpt-4o-mini` or Gemini free tier; Groq Whisper for STT (~$0/min) | Transcript + minimal context per request only | $0–$0.006/min | Degrades to local or deterministic |
| **Deterministic** | Built-in keyword/intent parser over the tool registry | Fully local | Free | Never dead-ends |

The user configures the tier via a **Brain** settings card (brain mode ∈ {auto, local, cloud}; cloud provider; STT mode; voice on/off). The context sent to any remote brain is deliberately minimal: the current transcript, recent session notes, and the tool schema. Raw memory contents are queried locally; only retrieval *results* travel — never the raw database, and only in cloud mode.

## 6. User Stories

### EPIC A — Summon and converse (the Jarvis feel)

| ID | Story | Acceptance | Increment |
|---|---|---|---|
| A1 | As a user, I can press a global hotkey from any app and summon the assistant instantly | Alt+Space opens the overlay in <500 ms from any focused window | EXP-005 ✅ |
| A2 | As a user, I can type a command and get a reply without touching the mouse | Enter submits, Esc dismisses, input auto-focused | EXP-005 ✅ |
| A3 | As a user, I can speak my command instead of typing | Mic button records push-to-talk; transcript streams into the command line | EXP-006 |
| A4 | As a user, I can hear the reply spoken back | Reply spoken through Windows SAPI; toggleable in settings | EXP-009 |
| A5 | As a user, the assistant works on an airplane | Fully offline: local STT + local brain + local memory | EXP-006/007 |

### EPIC B — Know my world (Cortex Memory as long-term memory)

| ID | Story | Acceptance | Increment |
|---|---|---|---|
| B1 | As a user, I can ask "what did I note about X?" and get an answer from my captures | `query_memory` tool answers from the existing captures store | EXP-008 |
| B2 | As a user, I can dictate a note hands-free and have it saved to memory | `save_note` inserts into captures with retention applied; shown in reply, undo-able | EXP-008 |
| B3 | As a user, I can ask about my projects and update their status | `list_projects` / `update_project` over the projects store | EXP-008 |
| B4 | As a user, I can set reminders by voice ("remind me at 9pm to…") | `create_reminder` writes to reminders table; fires through TTS at due time | EXP-008 |
| B5 | As a user, my captured context ages sensibly | Configurable ageing window (folded EXP-004) with Review Rail decisions | EXP-007 |

### EPIC C — Reach beyond my desk

| ID | Story | Acceptance | Increment |
|---|---|---|---|
| C1 | As a user, I can ask the assistant to look something up | `web_search` via scoped local HTTPS fetch; results summarized, sources cited | EXP-008 |
| C2 | As a user, I choose whether my requests use a cloud brain | Cloud mode requires my API key, stored DPAPI-encrypted; never sent except as auth header | EXP-007 |

### EPIC D — Trust and control

| ID | Story | Acceptance | Increment |
|---|---|---|---|
| D1 | As a user, I see exactly what the assistant will do before it does it | Confirmation prompts for all write actions in the reply area | EXP-008 |
| D2 | As a user, I can destroy the assistant's access to destructive tools | `delete_item` always requires explicit confirmation; pause mode blocks capture | Existing (SET-001) |
| D3 | As a user, I can export everything and start fresh | Passphrase-encrypted portable export + restore | Existing (EXP-002/003) |
| D4 | As a user, I trust that voice never listens without me | Push-to-talk only; no wake word, no microphone permission at install | EXP-006 |

## 7. Features in Scope for v1

### 7.1 Summon surface — done (EXP-005 ✅)

A global `Alt+Space` hotkey (native registration via `tauri-plugin-global-shortcut`, works unfocused) opens a frameless, transparent, always-on-top overlay (560×380): dark glass surface, pulsing aura orb, "Aura / Neural Cortex" branding, a single command input (Enter submit, Esc dismiss, aria-live reply region), and programmatic `show_overlay`/`hide_overlay` commands. The echo brain currently confirms the summon loop; real reasoning arrives in EXP-007.

### 7.2 Voice in (EXP-006)

Push-to-talk recording in the webview (MediaRecorder), bytes to Rust, STT resolved in priority order: **cloud Whisper** (Groq free tier first; OpenAI Whisper fallback) when a key is configured and online, otherwise **local whisper-rs** (whisper.cpp) with a small model fetched once at first use. Local CPU inference at roughly 1–5 seconds on mid-range hardware is acceptable for v1. The budget pick is Groq (free, fast); OpenAI is $0.006/min only if used.

### 7.3 The brain (EXP-007, absorbs deferred EXP-004)

A single `execute_intent(text, context)` Rust function; tool-calling via ollama's native REST tool API or OpenAI/Gemini schemas; deterministic keyword fallback as the floor. This increment also ships the **settings overhaul** — brain mode, provider, STT mode, voice toggle, hotkey rebinding, and the **configurable ageing window** (bounded range) for context retention, surfacing in the Data Lifecycle card.

### 7.4 Actions (EXP-008)

Tools are typed Rust functions registered with name, description, and JSON schema; the LLM's tool-calling output maps directly onto them. The v1 registry:

| Tool | Data source | Side effect | Confirm required |
|---|---|---|---|
| `query_memory` | captures SQL | none (read) | no |
| `save_note` | captures insert | write | shown in reply, undo-able |
| `list_projects` / `update_project` | projects | write | only on status changes |
| `create_reminder` / `check_reminders` | new `reminders` table | write + timer | no (create), yes (fire) |
| `web_search` | local HTTPS fetch | none | no |
| `delete_item` | captures/projects | destructive | **always, explicit** |

Migration 7 adds the `reminders` table (`id`, `project_id`, `title`, `due_at`, `fired_at`, `created_at`). A lightweight in-app scheduler polls on a 30-second heartbeat and fires reminders through the TTS channel even with the overlay closed.

### 7.5 Voice out (EXP-009)

**Windows SAPI** (free, offline, zero downloads) via `windows-sys` bindings — the same dependency family already proven by the DPAPI work. Web Speech API in the webview remains a zero-code fallback. Voice output is toggleable per user preference.

### 7.6 Friction loop (EXP-010)

A full week of assembled-assistant usage on real work, with a structured friction log (test checklist, friction-log template, debrief questions), converting pain into the EXP-011 spec.

## 8. Explicitly Out of Scope (v1)

| Item | Why out | Possible future |
|---|---|---|
| Always-listening wake word | Highest privacy surface; contradicts push-to-talk law | v2 with explicit consent ADR |
| Screen vision ("see my screen") | Excluded since ADR-002; needs on-demand-only design + consent ADR | EXP-vision candidate |
| PC control (keyboard/mouse automation, app control) | Agent territory — only after the brain is proven | EXP-agent candidate |
| Cloud sync / multi-device | Violates local-first single-store law for v1 | Later, with end-to-end encryption design |
| Screenshots, OCR, clipboard monitoring, background observation | Explicitly excluded since ADR-002 | Separate consent-gated feature |
| Multi-user / team | Single-user v1 | Far future |
| Code signing | Deferrable; documented SmartScreen workaround | At real distribution |

## 9. Privacy Contract (Summary)

| Surface | Local mode | Cloud mode |
|---|---|---|
| Voice recording | Processed on-device (whisper-rs) | Audio bytes to Whisper API only |
| Conversation | Never leaves device | Transcript + minimal context per request |
| API keys | DPAPI-encrypted at rest | Never sent except as auth header |
| Tool results | Local SQLite only | Retrieval results only, never raw DB |
| Offline | Fully functional | Graceful degradation to local/deterministic |

The assistant never collects screenshots, clipboard contents, microphone audio (beyond push-to-talk sessions), access tokens, provider credentials, or unapproved desktop captures.

## 10. Success Metrics (v1)

| Metric | Target | Measurement |
|---|---|---|
| Summon latency | < 500 ms overlay from any context | Friction-test timing |
| Voice accuracy | ≥ 90 % transcript fidelity on natural dictation | Weekly log review |
| Action safety | Zero unconfirmed external effects; zero destructive actions without explicit confirmation | Friction week audit |
| Offline usability | Assistant fully usable with no network | Unplug test in friction week |
| Daily use | Assistant used ≥ 4 days/week in friction week | Self-report log |

## 11. Build Sequence and Timeline

Each increment is a PR through the three mandatory CI gates, each ending with a downloadable Windows binary for friction testing. Realistic total: **2–3 weeks of focused work**.

| Increment | What ships | Estimate | Status |
|---|---|---|---|
| **EXP-005** | Hotkey summon + overlay + new visual identity (echo brain) | ~1–2 days | ✅ Merged (PR #17) |
| **EXP-006** | Voice pipeline: local whisper-rs + optional cloud STT | ~3–4 days | Next |
| **EXP-007** | Brain: ollama + cloud + deterministic fallback + settings (absorbs EXP-004 ageing window) | ~4–5 days | — |
| **EXP-008** | Action registry + reminders + scheduler | ~4–5 days | — |
| **EXP-009** | SAPI speech-out + toggles + polish | ~2–3 days | — |
| **EXP-010** | Friction-test week of the assembled assistant → EXP-011 | 1 week | — |

After EXP-010, the validated candidates for v1.1 are screen vision (on-demand only), PC control, and a persistent desktop widget — each requiring its own ADR and consent design.

## 12. Risks and Mitigations

| Risk | Severity | Mitigation |
|---|---|---|
| Whisper-rs inflates binary (~100–500 MB) and CI time | Medium | Gate behind a Cargo feature; ship local STT as on-demand model download at first use |
| Local model quality on modest hardware | Medium | 3B-class floor; cloud covers quality-sensitive use; deterministic fallback covers zero-setup |
| ollama not installed | Medium | Runtime detection + one-click "install helper" screen; product remains usable without it |
| Code signing / SmartScreen friction | Low | Documented workaround ("More info → Run anyway"); deferrable until distribution |
| Feature creep into agent territory | High (process) | EPIC-D laws + per-feature ADRs; agent features blocked until EXP-010 debrief |
| Sandbox network flakiness slowing Rust builds | Low | Retry loops + CI always verifies on real Windows runners |

## 13. Decision Record Index

| ADR | Decision | Relevance |
|---|---|---|
| ADR-001 | Tauri + React + Rust stack | Foundation |
| ADR-002 | Intentional capture first; no passive observation | Privacy laws origin |
| ADR-003 | Local SQLite store + key management | Cortex Memory |
| ADR-004 | Encryption strategy (DPAPI wrapping) | Secret storage |
| ADR-005 | Export/recovery DPAPI binding | EPIC D |
| ADR-006 | Retention sweep + context ageing | Cortex Memory policy |
| ADR-007 | Jarvis pivot: Neural Cortex, hotkey-first, push-to-talk, Capture preserved | The pivot itself |
