# Aura Desktop

Aura is a **privacy-first Windows desktop AI coordination system**. It is being built to help a person resume meaningful project work without reconstructing context from scratch. The initial release starts with an intentional context workflow, project continuity, local memory contracts, and clear user control—before passive observation, cloud intelligence, or autonomous action.

> **V0 principle:** Aura earns trust through visible local value before it receives additional system access.

## Current Foundation

The initial repository includes a Tauri 2 application with a React and TypeScript workspace, a Rust-native command layer, an intentional-capture privacy state machine, and an opinionated project dashboard. The application currently contains sample local workspace data so the product shell and command boundary can be exercised without requesting sensitive Windows permissions.

| Area | Initial status |
|---|---|
| Windows desktop shell | Implemented with Tauri 2 |
| Project continuity workspace | Implemented with local sample state |
| Privacy mode | Implemented; capture can be paused and resumed |
| Manual context marker | Implemented as a local native command |
| SQLite local store | Planned next |
| Active-window metadata | Planned after consent and exclusions design |
| Screen capture and OCR | Explicitly excluded from V0 |
| AI provider and cloud sync | Explicitly excluded from V0 |

## Technology Stack

| Layer | Technology |
|---|---|
| Desktop application | Tauri 2 |
| User interface | React 19, TypeScript, Vite |
| Native core | Rust |
| Package manager | pnpm |
| Initial target | Windows 10/11 |

## Run Locally on Windows

Install the current LTS version of Node.js, Rust stable, Microsoft C++ Build Tools, and WebView2. Then clone the private repository and run:

```bash
pnpm install
pnpm tauri dev
```

To create a production installer after the Windows prerequisites are installed:

```bash
pnpm tauri build
```

## Repository Guide

```text
src/                         React desktop workspace
src-tauri/                   Rust native core
src-tauri/capabilities/      Tauri permission boundary
docs/architecture.md         System shape and trust boundary
docs/decisions/              Architecture decision records
```

## Security Posture

Aura's initial capability manifest permits only the default desktop runtime. It intentionally excludes direct file access, screen capture, clipboard reads, microphone use, external URL opening, and arbitrary network calls. New native capabilities must be introduced in a dedicated branch with an ADR, user consent language, a capability manifest change, and validation tests.

## Next Build Increment

The next safe implementation slice is **durable local project memory**: add a SQLite repository for projects, decisions, context markers, settings, and retention controls. After that, prototype a single user-initiated active-window metadata capture path on Windows. Do not introduce continuous capture, screenshot retention, OCR, provider credentials, or computer-use actions until their individual product and security designs are approved.

## Architecture References

Read [the architecture overview](docs/architecture.md), [ADR-001](docs/decisions/ADR-001-tauri-react-rust.md), and [ADR-002](docs/decisions/ADR-002-intentional-capture.md) before adding native system access or data persistence.
