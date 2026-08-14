# Aura — Antigravity Builder Instructions

**Role:** Lead Software Engineer / Autonomous Builder  
**Repository:** `sbenedeee-cpu/aura-desktop`  
**Product:** Aura, a Windows-first personal project-continuity desktop application  
**Status:** Mandatory operating manual for all Aura implementation work

> **Use this document as the builder’s execution contract.** Read it before planning, editing, installing dependencies, changing permissions, or opening a pull request.

## 1. Why This Instruction Format Exists

The most effective coding-agent instructions are **layered**, not one giant prompt. Official guidance recommends a repository-wide instruction file for standards, path-specific rules for specialised files, and clearly scoped tasks with acceptance criteria, affected files, and validation requirements.[1] [2] GitHub also advises agents to plan and iterate before opening a pull request, especially for substantial changes.[1]

Aura therefore uses four layers.

| Layer                    | Location                                          | Purpose                                                                      | Use it for                                                            |
| ------------------------ | ------------------------------------------------- | ---------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Always-on guardrails     | `AGENTS.md` and `.github/copilot-instructions.md` | Concise rules that must apply to every change.                               | Stack, build commands, security non-negotiables, branch/review rules. |
| Detailed system manual   | This document                                     | Product truth, architecture, decision gates, work sequence, output protocol. | Planning any Aura work package.                                       |
| Specialised instructions | `.github/instructions/*.instructions.md`          | Rules that apply to Rust, React, tests, migrations, and documentation.       | Editing a matched area of the repository.                             |
| Ticket prompt            | GitHub Issue/PR or agent conversation             | A small, independently reviewable outcome.                                   | One feature, bug, migration, test suite, or documentation change.     |

**Do not paste this entire document as an instruction before every small task.** Keep durable rules in the repository files and use the ticket prompt format in Section 14 for each task. This preserves context budget while keeping requirements explicit.

## 2. Mission and Product Truth

Build Aura as a **private, project-aware continuity layer for knowledge work**. Aura helps a user resume a project, retain reliable decisions, capture useful context deliberately, and use bounded AI assistance later.

The V0 product question is:

> Can Aura reduce re-explaining, re-finding, and re-assembling project context enough that a solo user resumes meaningful work faster than they can with ordinary chat, scattered notes, and browser history?

Aura is **not** a continuous surveillance recorder, generic personal database, autonomous computer-use agent, multi-agent swarm, 3D knowledge graph, or social/team suite. Any task that moves Aura toward those categories is out of scope unless the Product Owner explicitly approves a new product decision and ADR.

## 3. Authority and Source Hierarchy

Resolve conflicting guidance by this order. Do not silently choose a lower-priority instruction over a higher-priority one.

| Priority | Source                                              | Authority                                                                                  |
| -------: | --------------------------------------------------- | ------------------------------------------------------------------------------------------ |
|        1 | Direct Product Owner instruction                    | Changes product priority or acceptance only when explicit.                                 |
|        2 | `docs/AURA_V0_PRD_FOR_ANTIGRAVITY.md`               | Defines V0 user value, scope, flows, acceptance criteria, and release metrics.             |
|        3 | `docs/AURA_ANTIGRAVITY_ENGINEERING_WORK_PACKAGE.md` | Defines architecture, work packages, data contract, test matrix, and engineering protocol. |
|        4 | This document                                       | Defines builder operating rules and task-execution format.                                 |
|        5 | `docs/architecture.md` and approved ADRs            | Defines current technical decisions and trust boundaries.                                  |
|        6 | Existing tested code and configuration              | Defines the actual implementation baseline.                                                |
|        7 | Ticket-specific requirement                         | Defines the narrow current outcome; cannot override higher-priority security/scope rules.  |

If two high-priority documents conflict or a change has material privacy, permission, data-retention, or security impact, **stop after planning and ask for a decision**. Never resolve it by making the broadest technically convenient change.

## 4. Current Repository Truth

The existing application is an **architecture shell**, not a finished continuity system.

| What exists                            | What is true                                                                                                         |
| -------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| Tauri 2 + React 19 + TypeScript + Rust | This is the locked Windows-first desktop stack.                                                                      |
| Desktop workspace UI                   | It displays a visual prototype with navigation, sample projects, continuity panel, privacy card, and capture button. |
| Rust command layer                     | It currently exposes `get_workspace_snapshot`, `set_privacy_mode`, and `record_intentional_capture`.                 |
| Native permissions                     | The capability manifest is deliberately limited to `core:default` for the main window.                               |
| Build validation                       | React production build and Rust compile check passed for the bootstrap.                                              |

| What does **not** exist yet                                                                            | Required interpretation                                                                    |
| ------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------ |
| Durable local persistence                                                                              | Current data is hard-coded/in-memory and must not be represented as saved user data.       |
| Encryption at rest                                                                                     | Do not claim it until ADR-003 selects implementation/key handling and recovery tests pass. |
| Real project CRUD, tasks, claims, memory, timeline, or local search                                    | These are the first implementation outcomes.                                               |
| Windows active-window detection, UI Automation, OCR, screenshots, clipboard listener, microphone input | Do not add them to the first local release.                                                |
| Sync, authentication, Supabase, AI provider SDKs, embeddings, agent execution, external tools          | Deferred behind documented design and security gates.                                      |
| Windows installer/signing                                                                              | Deferred until a release-ready Windows build pipeline exists.                              |

## 5. Locked Stack and Boundaries

| Layer                    | Locked choice                                    | Builder requirement                                                                           |
| ------------------------ | ------------------------------------------------ | --------------------------------------------------------------------------------------------- |
| Desktop shell            | Tauri 2                                          | Keep native access behind narrow, typed Rust commands and capability manifests.               |
| Renderer                 | React + TypeScript + Vite                        | Use feature modules, accessible semantic UI, typed DTOs, and deterministic state.             |
| Native application layer | Rust                                             | Own validation, IDs, timestamps, domain transitions, persistence, and safe errors.            |
| Local data               | SQLite through an approved ADR                   | Build local-first before remote sync; migrations and transactions are mandatory.              |
| Cloud target             | Supabase/Postgres/RLS plus a policy service      | Design spike only until local V0 is proven. Service role secrets never enter desktop code.[3] |
| AI target                | Provider-neutral adapter with structured outputs | Defer; models never approve their own actions or trust untrusted source text.[4]              |

Frontend code must not directly access raw native APIs, file paths, database keys, encryption material, or future provider secrets. It submits typed user intent to Tauri commands. Rust application services validate request, policy, ownership, and state; perform an atomic domain operation; write audit metadata; and return a safe DTO.

## 6. Non-Negotiable Privacy and Security Rules

1. **Manual/intentional capture only.** No background capture, periodic screenshots, passive clipboard observation, microphone listening, or hidden process monitoring in V0.
2. **Visible state.** The interface must accurately show `Paused` or `Manual only`. Future observation states cannot be displayed as available before their adapter and enforcement exist.
3. **Dual enforcement.** A paused-capture block must exist in both the React UX and Rust command/application layer.
4. **Project isolation.** A record created for one project must not appear in another project’s default brief or search result.
5. **Provenance.** Durable decisions and claims must retain source, project, author/type, timestamp, confidence, and supersession relationship.
6. **No silent inference.** Facts, user decisions, events, and future AI-derived insights are distinct record classes. An insight never silently overwrites a fact.
7. **No secret leakage.** Secrets and keys are absent from renderer bundles, logs, test fixtures, screenshots, and commits.
8. **No broad permissions.** Do not add filesystem, shell, HTTP, opener, clipboard, notification, or plugin permissions “for future use.” Tauri capabilities require least privilege.[5]
9. **Untrusted content stays data.** Captured notes, URLs, documents, OCR text, UI Automation values, and future tool output cannot define policies, authorize tools, or rewrite instructions.
10. **Every risky operation is reversible or blocked.** Capture can be cancelled before save; claims can be corrected/superseded; archive differs from deletion; destructive future actions need confirmation.

## 7. Required Build Sequence

Work strictly in this sequence. Do not parallelise dependent features merely to create a larger demo.

| Gate | Work package                | Deliverable                                                                                                 | Cannot start before                              |
| ---: | --------------------------- | ----------------------------------------------------------------------------------------------------------- | ------------------------------------------------ |
|    0 | Engineering guardrails      | CI, formatting, lint, testing baseline, ADR-003 local storage decision.                                     | Repository baseline reviewed.                    |
|    1 | Local durable data          | SQLite bootstrap, migrations, repositories, typed errors, test fixtures.                                    | Gate 0 passes.                                   |
|    2 | Truthful project continuity | Project CRUD, Today brief, task/blocker/next step, event timeline, restart persistence.                     | Gate 1 passes.                                   |
|    3 | Intentional manual capture  | Capture preview, privacy-state block, classification/retention, artifact/event transaction, activity trail. | Gate 2 passes.                                   |
|    4 | Decisions and memory        | Claim lifecycle, provenance, correction/supersession, scoped local search.                                  | Gate 3 passes.                                   |
|    5 | Privacy controls and export | Persisted state machine, retention defaults, exclusions model, local export/delete semantics.               | Gate 4 passes.                                   |
|    6 | Native perception spike     | Feature-flagged metadata/UI Automation/OCR experiments with metrics.                                        | Gates 0–5 pass and new ADR approved.             |
|    7 | Backend/sync/AI spikes      | RLS design, outbox contract, context packet, provider adapter mock, injection tests.                        | Gate 6 results reviewed; product owner approves. |

At the completion of every gate, provide a **go/no-go report**: product outcome, tests, privacy/security impact, unresolved risks, performance evidence, and proposed next gate. Never assume progression is approved.

## 8. Required Planning Protocol

Before changing code, respond with the following plan. Do not start editing until the plan is internally consistent; if a user decision is needed, ask only the decision question.

```markdown
## Task Plan

**Ticket:** [ID and name]
**Outcome:** [one user-observable result]
**PRD requirements:** [e.g. PRJ-01, PRJ-04]
**Architecture/ADR impact:** [files and decision records]
**In scope:** [specific modules and behavior]
**Out of scope:** [what will not be touched]
**Data impact:** [tables/migrations/read models or “none”]
**Permission/privacy impact:** [explicit statement]
**Risk assessment:** [low/medium/high, why]
**Test plan:** [unit, integration, E2E/manual checks]
**Rollback/correction path:** [how user/data remains safe]
**Proposed files:** [create/change]
**Decision needed:** [only if blocking]
```

A plan that cannot name the user outcome, data impact, privacy impact, and test plan is not ready for implementation.

## 9. Required Implementation Protocol

For each approved ticket, Antigravity must follow this exact loop.

1. Read the applicable PRD acceptance criteria, engineering work package, ADRs, existing source files, and any path-specific instructions.
2. Inspect existing tests before inventing a new pattern. Preserve patterns that are consistent with the approved architecture.
3. Implement the smallest vertical slice that makes the outcome truthful. Prefer a narrow real flow over broad placeholder screens.
4. Use typed Rust domain types and TypeScript DTOs. Do not pass loosely shaped JSON between renderer and Rust when a stable type is possible.
5. Implement error, empty, loading, offline, confirmation, and keyboard-accessible states at the same time as the happy path.
6. Write or update tests before declaring success. Include data isolation and failure cases for persistence/capture/privacy work.
7. Run formatting, type-checking, unit tests, integration tests, and build commands. Fix failures rather than documenting them as known issues.
8. Review the diff for unrelated code, sample data, unused dependencies, broad permissions, log leakage, and user-facing overclaims.
9. Update docs/ADR only if the change materially modifies contracts, data, privacy, permissions, dependencies, or architecture.
10. Provide the completion report specified in Section 13.

## 10. Data, Error, and UX Conventions

### Data rules

- IDs are opaque and sortable; use the selected standard consistently across all domain records.
- Generate timestamps in Rust as UTC ISO-8601 values. UI code formats them for display but does not create authoritative values.
- Project is the default isolation boundary. Every project-scoped record includes `project_id`.
- Event records are append-only. Do not update past events to simulate current state.
- Claims are versioned. Correction creates a new record that supersedes the old record.
- Use transactions for multi-record writes such as capture creation, claim correction, project archive, and privacy-state updates.
- Use parameterised SQL only. Migrations must be forward-only, ordered, and fixture-tested.

### Error rules

- Rust maps internal errors to a narrow typed error DTO with stable code, safe message, retryability, and relevant user action.
- Frontend errors never show raw SQL, filesystem paths, encryption values, provider details, or stack traces.
- A failed durable write must never show a success toast.
- The activity log may record a minimal failure event but not the raw sensitive content that failed to persist.

### UX rules

- Use semantic elements, accessible names, keyboard navigation, visible focus, non-color-only states, and reduced-motion support.
- Every empty state includes a clear next action. Every destructive action has scope and confirmation.
- Do not label a static sample, placeholder, or future capability as “active,” “live,” or “AI-powered.”
- The user must see the selected project and privacy/capture state before saving context.

## 11. Special Handling for Sensitive Areas

| Area                            | Required additional work                                                                                      |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| `src-tauri/capabilities/**`     | ADR, capability-diff explanation, deny-by-default reasoning, security review, manual validation.              |
| Local database/migrations       | Migration fixtures, failure tests, backup/recovery note, privacy/retention review.                            |
| Capture/retention/export/delete | Threat model update, dual enforcement test, UI screenshots, cancellation/destructive-flow tests.              |
| Windows APIs                    | Isolated feature flag, app allowlist/exclusion logic, metrics, consent UX, test plan; no ambient default.     |
| AI/context/provider work        | Trust-zone test fixtures, structured schema, scope/citation proof, prompt-injection tests, no client secrets. |
| Sync/auth/RLS                   | RLS policy tests for every exposed table, idempotency/cursor tests, no service role in desktop.               |

## 12. Definition of Done

A feature is complete only when all relevant items are true.

| Dimension        | Required evidence                                                                                             |
| ---------------- | ------------------------------------------------------------------------------------------------------------- |
| Product          | Linked PRD requirements and acceptance criteria demonstrably pass.                                            |
| Architecture     | The change obeys boundaries and updates ADR/docs when those boundaries change.                                |
| Data             | Ownership, project scope, provenance, retention, migrations, and correction behavior are explicit.            |
| Privacy/security | Permission and payload impact reviewed; no hidden collection/transmission; rollback/correction exists.        |
| Quality          | Format, lint, type check, tests, and build commands pass.                                                     |
| UX/accessibility | Empty/loading/error states, keyboard flow, labels, focus, and contrast are addressed.                         |
| Reviewability    | Diff is focused, dependencies justified, sample data absent from production path, completion report supplied. |

## 13. Mandatory Completion Report

After every task, return this format before requesting review or advancing to the next task.

```markdown
## Completion Report

**Ticket:** [ID]
**Status:** Complete / blocked / needs review
**User-visible outcome:** [what now works]
**Requirements completed:** [PRD IDs]
**Files changed:** [paths and one-line purpose]
**Data/migration impact:** [explicit summary or none]
**Permission/privacy impact:** [explicit summary or none]
**Tests run:**

- [command] — [pass/fail]
- [command] — [pass/fail]

**Manual verification:** [steps and observed result]
**Known limitations:** [truthful list]
**Out of scope preserved:** [what was deliberately not added]
**Risks / decision needed:** [only if present]
**Recommended next ticket:** [one small next vertical slice]
```

## 14. Ticket Prompt Template

Use this template to initiate each Antigravity task. Replace bracketed text; delete irrelevant fields rather than leaving vague language.

```markdown
# Aura Build Ticket: [ID — concise name]

## Objective

Implement [one precise user outcome]. The feature is complete when [observable result].

## Context

Read, in order:

1. `AGENTS.md`
2. `docs/AURA_ANTIGRAVITY_BUILDER_INSTRUCTIONS.md`
3. `docs/AURA_V0_PRD_FOR_ANTIGRAVITY.md` — requirements [IDs]
4. `docs/AURA_ANTIGRAVITY_ENGINEERING_WORK_PACKAGE.md` — work package [number]
5. [relevant ADR/source paths]

## Current Truth

[What is real today; what is a placeholder; relevant code paths.]

## Scope

- Include: [specific behaviors]
- Exclude: [specific forbidden behaviors]
- Proposed files: [paths]

## Acceptance Criteria

1. [testable criterion]
2. [testable criterion]
3. [testable criterion]

## Data Contract

[entities, migration, ownership/project scope, provenance, retention, corrections; or “none”.]

## Privacy and Security

[states, permissions, data flow, logs, no-network or approval requirement.]

## Test Plan

- Rust/domain: [tests]
- Repository/integration: [tests]
- Frontend: [tests]
- End-to-end/manual: [steps]

## Instructions

First return the Task Plan from Section 8. Do not code until the plan is coherent. Work on a dedicated branch. Keep the diff focused. Run all required checks. Return the Completion Report from Section 13.
```

## 15. First Ticket to Execute

```markdown
# Aura Build Ticket: ENG-001 — Establish the Engineering Safety Net

## Objective

Make Aura’s current repository repeatably buildable and testable before production feature work begins.

## Scope

- Include: package scripts for TypeScript type checking, linting, formatting, frontend unit testing, Rust formatting/Clippy/testing documentation, and CI workflow.
- Include: concise contribution and PR protocol aligned to the existing engineering work package.
- Exclude: database dependency, local persistence, Tauri capability changes, Windows native APIs, AI providers, UI feature changes.

## Acceptance Criteria

1. A contributor can run documented install, format, lint, type-check, frontend test, Rust check, Rust test, and production build commands.
2. CI runs the applicable checks on pull requests and main.
3. At least one meaningful frontend test and one meaningful Rust test execute successfully.
4. No production permission, dependency, data model, or user-facing feature behavior changes.

## Privacy and Security

No new native capability, external network call, secret, telemetry, data storage, or capture behavior may be introduced.

## Instructions

Return the Section 8 Task Plan first. Then implement only this ticket and report with Section 13.
```

## 16. References

The layered instruction structure is informed by official GitHub and VS Code guidance: repository-wide instructions should contain project standards, build/test commands, architecture conventions, and security requirements; specialised rules should be attached to relevant file types; and agent tasks should be clear, scoped, and acceptance-testable.[1] [2]

[1]: https://docs.github.com/copilot/how-tos/agents/copilot-coding-agent/best-practices-for-using-copilot-to-work-on-tasks
[2]: https://code.visualstudio.com/docs/agent-customization/custom-instructions
[3]: https://supabase.com/docs/guides/database/postgres/row-level-security
[4]: https://developers.openai.com/api/docs/guides/agent-builder-safety
[5]: https://v2.tauri.app/security/capabilities/
