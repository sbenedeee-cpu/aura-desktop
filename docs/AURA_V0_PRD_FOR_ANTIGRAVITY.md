# Aura Windows Desktop V0 — Product Requirements Document

**Audience:** Antigravity, Lead Software Engineer  
**Author:** Manus AI  
**Status:** Build-ready product specification  
**Product:** Aura Desktop  
**Target:** Windows 10/11 desktop, single-user V0 pilot  
**Primary repository:** `sbenedeee-cpu/aura-desktop`  
**Source of truth:** This document, `docs/architecture.md`, ADR-001, ADR-002, and the saved Aura research mandate.

> **Product decision:** Aura V0 is a privacy-first, project-continuity system for a single knowledge worker. It is not an ambient surveillance recorder, a generic second brain, or an autonomous computer-use agent.

## 1. Executive Product Definition

Aura helps a user **resume and advance active projects without reconstructing context from scratch**. It does this by collecting only deliberately shared context, preserving project facts and decisions with provenance, and making the current state of work readable in one desktop workspace.

The initial product test is deliberately narrow:

> Given an active project, can Aura help the user recover the current goal, latest decisions, blockers, and next step faster—and with fewer manual context transfers—than a normal AI chat workflow?

The answer must be demonstrated through a V0 pilot before Aura expands into continuous history, broad native perception, autonomous agent workflows, mobile parity, or a graph-heavy interface.

| Product dimension | V0 decision |
|---|---|
| Primary user | Eternal / a solo creative technologist handling multiple active projects. |
| Primary job | Resume a project, preserve an important decision, or prepare concise project context for the next action. |
| Primary device | Windows desktop/laptop. |
| Data posture | Local-first, explicit capture, visible state, reversible records. |
| Intelligence posture | Evidence-backed assistance and drafts; no self-authorizing actions. |
| Core proof | Faster, more trustworthy project resumption than ad hoc chat and scattered notes. |

## 2. Current Implementation Baseline

Antigravity must treat the existing repository as a **validated architecture shell**, not as a partially completed feature set. The current code intentionally contains sample data and a narrow native command surface. Preserve the trust boundary while replacing the temporary implementation with tested services.

| Existing component | Current state | Engineering implication |
|---|---|---|
| Tauri 2 desktop shell | Implemented | Retain as the desktop platform. |
| React/TypeScript workspace | Implemented | Use as the presentation layer; split the current single component into feature modules. |
| Rust native core | Implemented | Extend through explicit, typed commands only. |
| Privacy mode | In-memory toggle only | Replace with durable per-device settings and an observable state machine. |
| Intentional capture marker | In-memory placeholder only | Replace with a project-scoped capture flow and a persistent event record. |
| Projects and memory signals | Hard-coded sample records | Replace with local repository-backed data and empty/loading/error states. |
| Native capabilities | `core:default` only | Do not add privileged capabilities without a focused ADR, consent UX, test plan, and review. |
| Build checks | React production build and Rust compile check passed | Maintain these checks in CI from the first implementation increment. |

## 3. Target User, Jobs, and Product Principles

Aura is initially designed for a knowledge worker who moves among design, research, product, AI, documents, browser tabs, implementation work, and decisions across several projects. The user does not need more raw capture; they need a reliable way to understand what matters now and why.

| Job to be done | User trigger | Desired Aura outcome |
|---|---|---|
| Resume a project | Returning after a break or context switch | The user sees the project goal, active task, decisions, blockers, recent evidence, and next step. |
| Preserve a decision | A design, product, technical, or operational choice is made | Aura stores a reviewable decision record with source, time, project, confidence, and correction path. |
| Capture useful context | The user wants to preserve a note, selected text, link, or current app context | Aura shows exactly what will be recorded, where it will go, and how long it will be retained. |
| Ask about project state | The user needs synthesis or a handoff packet | Aura constructs a scoped evidence bundle and returns a cited, project-bound answer. |
| Inspect activity | The user needs to know what Aura captured, synced, suggested, or sent to an AI provider | Aura presents a readable activity trail with source and policy details. |

The following principles govern every implementation decision.

1. **Control precedes intelligence.** A user must be able to see, pause, delete, and correct data before Aura uses it for assistance.
2. **Project scope precedes global recall.** Context and retrieval default to one project; cross-project context requires a visible user decision.
3. **Evidence precedes inference.** Aura labels facts, user-authored decisions, observations, and derived hypotheses differently.
4. **Local processing precedes cloud escalation.** Sensitive raw desktop content is not a default cloud payload.
5. **The model is not the policy engine.** Models can propose structured output; deterministic code validates data scope, permissions, and approvals.
6. **Every feature needs a rollback path.** Records can be corrected, superseded, expired, or deleted; risky writes require preview and approval.

## 4. Scope Boundary

The V0 scope is intentionally staged. The first build increment is a durable local continuity application. Remote sync, active-window metadata, AI assistance, and additional perception are introduced only after their predecessor releases meet the stated test and privacy gates.

| Delivery class | In scope for V0 | Explicitly not in scope |
|---|---|---|
| Project continuity | Projects, tasks, goals, decisions, blockers, activity, next step, timeline. | Team workspaces, role management beyond one pilot user, full PM-suite workflows. |
| Capture | Manual note, pasted/selected text, URL, and a deliberate capture marker linked to a project. | Continuous screen recording, passive clipboard monitoring, ambient microphone capture. |
| Memory | Reviewable event, artifact, claim/decision, project-state records, correction and expiry. | Unreviewed global personality profiling or opaque long-term personal inference. |
| Perception | Architecture and spike for user-authorized active-window metadata. | Broad screenshot analysis, background OCR, unfenced accessibility scraping. |
| AI | One provider-neutral, read-only project-answer flow after local pilot gates. | Autonomous tools, browser control, email/posting, filesystem writes outside Aura. |
| Visualisation | Project timeline and filtered list views. | 3D Cortex; a 2D Cortex is a later, measured experiment. |
| Sync | Local outbox design and an implementation plan. | Cross-device production sync before local data and conflict behavior are proven. |

## 5. Product Surfaces and Required Flows

### 5.1 Navigation

The implemented shell currently labels views `Now`, `Projects`, `Memory`, `Cortex`, and `Controls`. The production information architecture shall use the following user-facing destinations. Antigravity may retain `Now` as the route name if needed, but the visible labels and navigation behavior must align with this model.

| Surface | Primary purpose | V0 release order |
|---|---|---|
| **Today** | Resume work: active project, continuity brief, next step, recent activity, pending memory review. | Release 1 |
| **Projects** | Browse and edit project state, tasks, decisions, artifacts, and timeline. | Release 1 |
| **Capture** | Intentionally add a note, selected text, link, or supported context to a chosen project. | Release 1 |
| **Memory** | Search, inspect provenance, confirm, correct, supersede, expire, or delete claims. | Release 2 |
| **Activity** | Review local capture, sync, model, approval, and error events. | Release 2 |
| **Settings** | Control privacy mode, exclusions, local retention, AI/provider state, and diagnostics. | Release 1, expanded later |

### 5.2 Flow A — Create and Resume a Project

The user creates a project with a name, optional one-line goal, and initial next step. The Projects surface must then show an empty-state guide that allows the user to add a task, a decision, or intentional context. When the user selects a project, Today must render a continuity brief based on stored project records—not an LLM-generated assertion.

**Acceptance criteria**

| ID | Requirement | Testable acceptance criterion |
|---|---|---|
| PRJ-01 | Create project | User creates a project with name; app validates non-empty, trims whitespace, and persists it after restart. |
| PRJ-02 | Edit project state | User can edit goal, current task, blocker, and next step; each change adds a project event. |
| PRJ-03 | Project isolation | Records created for Project A never appear in Project B’s brief, default search, or capture destination. |
| PRJ-04 | Resume brief | A selected project renders goal, current task, next step, latest decisions, unresolved blockers, and recent activity. Empty sections say so plainly. |
| PRJ-05 | Delete/Archive | User can archive a project after confirmation; archive hides it from default views without erasing audit history. Hard delete is deferred. |

### 5.3 Flow B — Intentional Capture

The Capture surface is the trust-critical interaction. The user chooses a project, sees a capture type, reviews the exact content to be stored, and confirms. The initial capture types are manual note, pasted/selected text, and URL. A future active-window capture is not permitted to silently trigger this flow.

**Acceptance criteria**

| ID | Requirement | Testable acceptance criterion |
|---|---|---|
| CAP-01 | Privacy state | When paused, all capture controls are disabled or show a clear pause explanation; no local event is created. |
| CAP-02 | Scope disclosure | Before save, user sees destination project, capture type, text/link preview, classification, and retention setting. |
| CAP-03 | Explicit consent | Saving requires a deliberate action; closing/canceling creates no capture record. |
| CAP-04 | Durable event | A confirmed capture becomes an immutable event and optional artifact with timestamp, source, project, and user-chosen label. |
| CAP-05 | Sensitive data warning | The interface warns that text may contain sensitive data and lets the user edit/remove it before storage. Filtering is assistive, not a guarantee. |
| CAP-06 | No unintended transmission | During local-only releases, capture must make no network request. Add a test that blocks or logs attempted outbound calls. |

### 5.4 Flow C — Memory and Decision Lifecycle

Aura distinguishes a raw event from a durable memory. A user-added decision is a structured claim. It must retain provenance and support later correction without silently rewriting history.

| Record type | Example | V0 authority | Lifecycle |
|---|---|---|---|
| Event | “Manual note captured at 14:03.” | Immutable system/user event | Created → retained/expired/deleted under policy. |
| Artifact | URL, note body, imported text | User-owned source | Created → versioned or deleted. |
| Claim / decision | “Tauri is selected for Aura V0.” | User-confirmed factual/project record | Draft → confirmed → superseded/expired/deleted. |
| Project state | Current goal, task, blocker, next step | User-editable state | Updated with event history. |
| Derived insight | Future AI-generated pattern | Never authoritative in V0 | Candidate → review → accepted/rejected/expired. |

**Acceptance criteria**

| ID | Requirement | Testable acceptance criterion |
|---|---|---|
| MEM-01 | Create decision | User can create a decision title, rationale, project, confidence, and sources. |
| MEM-02 | Provenance | Every claim displays author/type, creation time, sources, project, and its previous/superseded version when present. |
| MEM-03 | Correction | Correcting a claim creates a new version and marks the old one superseded; it does not destroy the audit link. |
| MEM-04 | Review queue | Future generated candidates appear as non-authoritative candidates and require explicit confirm/reject. |
| MEM-05 | Expiry/delete | User can set expiry or delete a record; the UI explains whether related event metadata remains for audit. |

### 5.5 Flow D — Search and Continuity Retrieval

Release 1 search is deterministic local text search over the selected project’s records. Semantic/vector retrieval is a later V0 increment, only after the underlying data model and relevance test set exist. The user must always see why a result was included.

**Acceptance criteria**

| ID | Requirement | Testable acceptance criterion |
|---|---|---|
| RET-01 | Default project scope | Search defaults to selected project and visibly indicates the scope. |
| RET-02 | Filterable record types | User can filter events, artifacts, decisions, tasks, and project state. |
| RET-03 | Explainability | Each result shows source type, project, created/updated time, and linked record/provenance. |
| RET-04 | Empty and error states | No-result, first-use, and repository failure states are actionable and not silently blank. |
| RET-05 | Hybrid retrieval gate | Vector search may not ship until a seeded relevance benchmark, project filtering, and citation rendering pass. |

### 5.6 Flow E — Privacy, Settings, and Activity

Privacy is a first-class feature, not a settings afterthought. The user must be able to understand Aura’s current observation state in a few seconds.

**Acceptance criteria**

| ID | Requirement | Testable acceptance criterion |
|---|---|---|
| PRV-01 | Visible state | Today and Settings display one of: `Paused`, `Manual only`, `Authorized metadata capture`, `Filtered`, or `Error`. |
| PRV-02 | Persistent setting | Capture mode and retention preferences persist after restart and are reflected by Rust command responses. |
| PRV-03 | Exclusions design | Settings includes a data model and interface placeholder for app/domain/project exclusions before native observation is introduced. |
| PRV-04 | Activity trail | Every capture, edit, delete, failed operation, and later sync/model request produces a minimally sufficient activity event. |
| PRV-05 | Export/delete plan | The implementation exposes a local export path and deletion semantics in the data layer before any cloud sync is enabled. |

## 6. AI and Native Perception Gates

No AI provider, Windows UI Automation call, Graphics Capture call, OCR engine, or external connector belongs in the initial local-continuity release. The feature may start only when the following gates are met.

| Capability | Required preconditions | V0 runtime policy |
|---|---|---|
| Active-window metadata | Windows-specific prototype; visible state; app exclusions; idle CPU/latency measurement; failure modes documented. | User-initiated or approved event-driven only. |
| UI Automation | Allowlist of tested applications; structured extraction schema; error handling; privacy disclosure. | Prefer over screenshot/OCR where the app exposes structure. |
| OCR | Curated accuracy/latency test corpus; local retention behavior; PII/redaction test. | Local fallback only; raw image remains ephemeral unless saved explicitly. |
| AI project answer | Provider-neutral interface; context-disclosure sheet; project filter; citations; rate/cost controls; prompt-injection test. | Read-only answer and draft generation only. |
| Internal write draft | Structured schema; deterministic validation; preview/diff; rollback. | User confirmation before any durable write. |
| External action/tool | Scope, policy gateway, approval token, audit replay, test suite. | Deferred from V0. |

## 7. Non-Functional Requirements

| Area | Requirement |
|---|---|
| Offline behavior | Core project, capture, and local search workflows must work without network access. |
| Performance | Local project open and manual capture save should feel immediate; establish measured p50/p95 budgets in the engineering package before implementation. |
| Accessibility | Keyboard navigation, visible focus, semantic controls, screen-reader labels, readable contrast, reduced-motion support, and no color-only state meaning. |
| Resilience | Failed persistence cannot report success. Writes must be atomic; errors must offer retry or safe cancellation. |
| Security | No provider secrets in frontend or Tauri bundle; no arbitrary native command; least-privilege capabilities; locked dependencies and CI checks. |
| Observability | Record minimal event metadata and errors locally; do not log raw capture bodies by default. |
| Data ownership | Every record has a project, user/device owner, classification, source/provenance, and retention behavior. |

## 8. Metrics and Release Criteria

The V0 pilot cannot be judged by downloads, screen-capture volume, or model call count. It succeeds only if it improves continuity without undermining trust.

| Metric | Pilot target / decision use |
|---|---|
| Resume-task time | Compare the time to resume representative project work with Aura versus current chat/notes workflow. |
| Manual context transfers | Track count of copy/paste/re-explanation steps required before meaningful work begins. |
| Continuity brief accuracy | User-rated accuracy and omission rate for goal, latest decision, blocker, and next step. |
| Memory correction rate | Track incorrect or stale claims and time-to-correction. A high rate blocks automatic promotion. |
| Trust/control rating | User rates clarity of capture state, ability to inspect/delete data, and confidence in local-only behavior. |
| Local reliability | Capture-save and project-load success rate; unrecoverable errors block pilot expansion. |
| Security gate | No known bypass that permits capture while paused, cross-project data leakage, or unapproved external data transfer. |

**Pilot release gate:** Do not add automated perception, provider-backed AI, or sync until the local continuity flow is stable, the user can inspect/correct all stored project records, and a short set of resume tasks shows credible value.

## 9. Product Backlog, Ordered by Value and Risk

| Order | Feature slice | Why now | Definition of done |
|---:|---|---|---|
| 1 | Local data repository and migrations | Replaces hard-coded UI; unlocks all truthful behavior. | Durable projects, events, tasks, claims, settings, repository tests. |
| 2 | Projects + Today continuity brief | Core product hypothesis without privacy expansion. | Create/edit/resume project; real empty states; event timeline. |
| 3 | Manual capture + activity | Establishes explicit capture contract. | Disclosed, project-scoped local capture and readable audit entry. |
| 4 | Decisions/memory review | Makes continuity trustworthy and correctable. | Provenance, correction/supersession, expiry/delete, scoped search. |
| 5 | Settings + privacy state machine | Makes data-control product-grade. | Persisted pause/manual-only state, retention, exclusions model. |
| 6 | Local search benchmark | Proves data can be found before embedding complexity. | Search test corpus, result provenance, measurable relevance. |
| 7 | Active-window metadata spike | Tests native-perception feasibility in a narrow, reversible way. | Metrics, consent UX, allowlist/exclusions, no ambient default. |
| 8 | Backend/sync design spike | Validates identity, RLS, outbox, and event contract. | Migrations/RLS review, no secrets in client, sync test plan. |
| 9 | Read-only AI project answer | Tests the actual continuity hypothesis. | Scoped evidence packet, cited answer, injection tests, telemetry. |

## 10. Out of Scope and Change Control

Antigravity must reject or explicitly escalate any request that attempts to add continuous capture, hidden monitoring, a global personal profile, arbitrary browser/system automation, external messaging, financial action, broad filesystem access, background audio, third-party sync, or unsandboxed AI tooling.

Every work item that changes capture, retention, permissions, tool schemas, local encryption, RLS, prompts, provider payloads, or audit behavior requires: an ADR; a focused test plan; acceptance criteria; a privacy/security review; and a clear rollback or user-correction route.

## 11. References

This PRD operationalizes the existing Aura research foundation and uses its evidence base for platform and security decisions. Primary references include Microsoft’s Windows capture and UI Automation documentation, Tauri’s capability model, Supabase RLS guidance, and current agent safety guidance.[1] [2] [3] [4] [5]

[1]: https://learn.microsoft.com/en-us/windows/apps/develop/media-authoring-processing/screen-capture
[2]: https://learn.microsoft.com/en-us/windows/win32/winauto/entry-uiauto-win32
[3]: https://v2.tauri.app/security/capabilities/
[4]: https://supabase.com/docs/guides/database/postgres/row-level-security
[5]: https://developers.openai.com/api/docs/guides/agent-builder-safety
