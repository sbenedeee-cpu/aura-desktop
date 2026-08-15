# ADR-009: Hybrid Brain — Tiered Reasoning with a Deterministic Floor

**Status:** Accepted
**Supersedes:** —
**Related:** ADR-008 (tiered voice pipeline), PRD §5 (Cortex Reasoning), PRD §7.3 (command dispatch), PRD §4.3 (never dead-end core law)
**Implemented in:** EXP-007

## Context

After EXP-006, Aura could hear the user (local STT via whisper.cpp) but still could not think: the `run_brain` surface ran an echo placeholder. The PRD demands a brain that is **local-first by default**, **cloud-boosted when API keys exist**, and — above all — **never dead-ends**. The user must always know whether their words traveled to a remote service, and secrets must never be written to disk as plaintext.

Several viable designs existed:

1. **Single provider, hard-coded.** Simplest, but couples the product to one vendor and fails hard offline.
2. **Auto-only routing with opaque fallbacks.** The router hides which tier answered; the user cannot audit whether input left the machine.
3. **Tiered resolution, explicit floor, transparent reporting.** Resolve at request time from stored settings; when no tier can run, the deterministic floor answers; every reply reports the tier that produced it.

## Decision

We adopted option 3 — a three-tier brain with a **deterministic floor** and **per-reply tier transparency**:

| Tier | Trigger | Failure behaviour |
|---|---|---|
| **Cloud** (Groq free tier first, OpenAI fallback) | `brain_mode = cloud`, or `auto` with a configured key | Degrade to local; if the *configured* key is rejected, say so explicitly instead of silent degradation |
| **Local** (Ollama REST, model auto-selected: `llama3` → `qwen3` → `qwen2.5`) | `brain_mode = local`, or `auto` with a reachable Ollama and no usable cloud key | Degrade to the floor |
| **Deterministic floor** | Every path; also direct answers for `capture`, `recall`, `help`, `status`, `clear` | Never dead-ends — the floor always replies |

Key supporting decisions:

- **Settings live in one JSON document** (`%APPDATA%/com.sbenedeve.aura/aura-settings.json` on Windows): preferences plain, secrets sealed through the existing KeyVault (DPAPI envelope per ADR-003) as a self-describing byte blob. The renderer never round-trips a secret; it sends raw key bytes on save and reads presence-only flags on load.
- **Tier resolution happens at request time**, not at boot: a user can change `brain_mode` or install Ollama and the next ask picks it up without restart.
- **The command loop runs off the Tauri event thread** (`std::thread::spawn`), so a slow local model never freezes the overlay.
- **The overlay renders a tier tag** (`☁ cloud` / `● local` / `◂ floor`) so the privacy boundary is always visible.
- **Context handed to tiers is minimal**: the current transcript plus a handful of recent capture texts. Raw SQLite never leaves the process (PRD §5.1).

## Consequences

**Positive.** Offline-by-default satisfies the privacy law; cloud is pure additive value once the user pastes a free Groq key. Transparent tier reporting builds trust and makes degraded-mode bugs immediately diagnosable. The floor keeps the "ask anything" surface alive even on a machine with no LLM installed.

**Negative.** Ollama reachability is probed synchronously at request time (cheap, but a first ask after a network change may pay a short probe latency — acceptable for v1). The deterministic floor handles only the first-class command verbs; everything else gets a polite "use a real model" pointer, so the floor must degrade gracefully rather than fake competence. `brain_mode = auto` with a configured key prefers cloud — documented in the settings panel so the user can force local-only when desired.

**Follow-up.** EXP-008 (action registry) will let the floor/brain replies trigger real actions; the brain's reply is currently always user-facing text.
