# Aura Desktop Architecture

## Purpose

Aura is a **Windows-first personal coordination system** that helps the user retain project continuity, preserve deliberate context, and later coordinate AI assistance without turning into an unbounded surveillance layer. This repository starts with the trusted local shell required to validate that proposition before expanding into automated perception or cloud intelligence.

> **V0 architectural principle:** Aura should be useful through intentional project context and local continuity before it asks for passive observation, background capture, external model access, or autonomous actions.

## System Shape

```text
React + TypeScript workspace
        │
        │ typed Tauri commands
        ▼
Rust application core
        │
        ├── Local workspace and project state
        ├── Privacy-mode state machine
        ├── Intentional capture markers
        └── Future capability-gated services
              ├── Windows perception adapter
              ├── Local memory store
              ├── AI-provider gateway
              └── Sync service
```

The React layer owns presentation and transient interaction state. The Rust core owns native state, command validation, and all future OS interactions. Nothing in the browser-facing UI receives direct system, file, screen, or network permission by default.

## Stack Decisions

| Layer                  | Selected technology                    | Reason for the V0 decision                                                                                             |
| ---------------------- | -------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| Desktop shell          | Tauri 2                                | Keeps the native boundary in Rust and exposes only explicit frontend commands.                                         |
| User interface         | React 19 + TypeScript + Vite           | Provides a rapid, typed interface foundation for the project workspace.                                                |
| Native core            | Rust                                   | Suitable for Windows APIs, local processing, secure command boundaries, and eventual performance-sensitive perception. |
| State in V0            | In-memory local state                  | Validates the workspace and privacy contract before committing to a migration-heavy database design.                   |
| Local persistence next | SQLite with encrypted sensitive fields | Supports offline project continuity and auditable local data ownership.                                                |
| Cloud services later   | Provider-agnostic, opt-in gateway      | Keeps API keys, model routing, and synchronisation outside the initial desktop trust boundary.                         |

## V0 Trust Boundary

Aura V0 intentionally permits only the default application runtime capability. It does **not** include filesystem browsing, screen capture, clipboard access, notification access, microphone input, arbitrary external URLs, or an opener plugin. Each of those requires its own design, threat model, user explanation, consent interface, capability grant, and test plan.

| Capability                        | V0 status | Conditions for introduction                                                                               |
| --------------------------------- | --------- | --------------------------------------------------------------------------------------------------------- |
| Intentional manual context marker | Included  | Stored locally through a typed native command.                                                            |
| Project workspace state           | Included  | Mocked locally while the durable local store is designed.                                                 |
| Passive screen or window capture  | Excluded  | Only after Windows API prototype, exclusion rules, redaction, and visible session status are implemented. |
| Clipboard access                  | Excluded  | Only with explicit capture action and no silent monitoring.                                               |
| Local OCR                         | Excluded  | Only after asset-retention and PII-filter rules are approved.                                             |
| Model-provider calls              | Excluded  | Only with user-controlled provider configuration, spend limits, redaction, and audit logs.                |
| AI actions or computer use        | Excluded  | Only after human approval, least-privilege tools, and prompt-injection controls are in place.             |

## Repository Layout

```text
src/                         React workspace UI
src-tauri/                   Rust/Tauri native application core
src-tauri/capabilities/      Explicit native permission manifests
docs/                        Product, architecture, and decision records
docs/decisions/              Architecture decision records (ADRs)
```

## Delivery Sequence

The next implementation increment should add a local SQLite repository for projects, decisions, context markers, and user-controlled capture settings. The subsequent increment should prototype one **explicitly initiated** Windows context capture route, starting with active-window metadata rather than screenshots. Continuous capture, vision analysis, multi-provider orchestration, cloud synchronisation, and agentic action belong to later validated milestones—not the initial build.
