# Aura Desktop

Aura is a **privacy-first Windows desktop AI coordination system**. It is being built to help a person resume meaningful project work without reconstructing context from scratch. The initial release starts with an intentional context workflow, project continuity, local memory contracts, and clear user control—before passive observation, cloud intelligence, or autonomous action.

> **V0 principle:** Aura earns trust through visible local value before it receives additional system access.

## Current Foundation

The repository includes a Tauri 2 application with a React and TypeScript workspace, a Rust-native command layer, an intentional-capture privacy state machine, and a project-resumption workspace. Project records, selected-project state, explicit manual captures, user-authored decisions with stated provenance, privacy settings, and project-scoped activity history are stored through a Rust-owned local SQLite boundary; the renderer never opens the database or constructs SQL.

| Area | Initial status |
|---|---|
| Windows desktop shell | Implemented with Tauri 2 |
| Project continuity workspace | Implemented with durable local SQLite records |
| Privacy mode | Implemented; capture can be paused and resumed |
| Explicit manual capture | Implemented with review-before-save, classification, retention, project scope, cancellation, and a native paused-mode block |
| Decision memory | Implemented with user-authored provenance, confidence, project-scoped retrieval, and non-destructive correction/supersession |
| SQLite local store | Implemented with numbered, transactional migrations |
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

## Quality and Safety Net

Before opening a pull request, install locked dependencies and run the complete safety-net suite:

```bash
pnpm install --frozen-lockfile
pnpm quality
```

The suite checks renderer formatting, linting, TypeScript, tests, and production build output; it also checks Rust formatting, Clippy warnings, native unit tests, and high-severity production JavaScript dependency advisories. GitHub Actions runs the same renderer and native checks on pull requests and pushes to `main`. See [CONTRIBUTING.md](CONTRIBUTING.md) for individual commands, privacy-sensitive change rules, and troubleshooting.

## Local Storage and Recovery

Aura opens a single `aura.sqlite3` database in the operating system’s Tauri application-data directory for the current user. The database contains only product records: projects, selected-project state, privacy settings, explicit manual captures, user-authored decision claims and their stated sources, activity history, and migration metadata. It does not collect screenshots, clipboard contents, microphone audio, access tokens, provider credentials, or unapproved desktop captures.

Migrations are numbered, append-only, and applied inside a SQLite transaction. If a migration fails, Aura does not reset the database; it reports that the prior records were left unchanged so the user can restart an updated build or retain a copy of the database before further recovery work. The Windows DPAPI-wrapped data-encryption-key proof of concept remains required before Aura ships persisted user data beyond this controlled V0 engineering milestone. See [ADR-003](docs/decisions/ADR-003-local-storage-and-key-management.md) for the governing decision.

## Repository Guide

```text
src/                         React desktop workspace
src-tauri/                   Rust native core
src-tauri/capabilities/      Tauri permission boundary
src/test/                    Shared renderer-test setup
.github/workflows/           Pull-request and main-branch quality gates
docs/architecture.md         System shape and trust boundary
docs/decisions/              Architecture decision records
CONTRIBUTING.md              Local quality, review, and privacy-change protocol
```

## Security Posture

Aura's initial capability manifest permits only the default desktop runtime. It intentionally excludes direct file access, screen capture, clipboard reads, microphone use, external URL opening, and arbitrary network calls. New native capabilities must be introduced in a dedicated branch with an ADR, user consent language, a capability manifest change, and validation tests.

## Next Build Increment

The next product increment is **settings, local export, and recovery controls**. The **Windows DPAPI key-wrapping and encrypted-storage compatibility proof of concept** remains a mandatory release-security gate under [ADR-003](docs/decisions/ADR-003-local-storage-and-key-management.md) before Aura ships persisted user data. Do not introduce continuous capture, screenshot retention, OCR, provider credentials, cloud synchronization, or computer-use actions until their individual product and security designs are approved.

## Architecture References

Read [the architecture overview](docs/architecture.md), [ADR-001](docs/decisions/ADR-001-tauri-react-rust.md), [ADR-002](docs/decisions/ADR-002-intentional-capture.md), and accepted [ADR-003](docs/decisions/ADR-003-local-storage-and-key-management.md) before adding native system access or data persistence.
