# Contributing to Aura Desktop

Aura is a Windows-first, local-first desktop system for project continuity. The repository must remain safe to evolve before new perception, persistence, or AI capabilities are introduced.

## Required local setup

Use Node.js 22, pnpm 11, and the stable Rust toolchain with the `rustfmt` and `clippy` components. On Windows, install the Tauri prerequisites described in the project README before running the native app.

```bash
pnpm install --frozen-lockfile
pnpm quality
```

The first native Rust build can take several minutes because Tauri dependencies compile locally. Subsequent runs should reuse the Rust build cache.

| Command | Purpose |
|---|---|
| `pnpm dev` | Run the renderer only for interface work. |
| `pnpm tauri dev` | Run Aura as a local desktop application. |
| `pnpm format` | Apply formatting to maintained renderer, configuration, and workflow files. |
| `pnpm format:check` | Fail if maintained files are not formatted. |
| `pnpm lint` | Run strict renderer linting with warnings treated as failures. |
| `pnpm typecheck` | Check TypeScript without emitting files. |
| `pnpm test` | Run renderer unit tests once. |
| `pnpm test:coverage` | Run renderer unit tests and write local coverage output. |
| `pnpm build` | Build the renderer for production. |
| `pnpm rust:fmt` | Check Rust formatting. |
| `pnpm rust:clippy` | Run Clippy with warnings denied. |
| `pnpm rust:test` | Run native Rust unit tests. |
| `pnpm security:js` | Audit production JavaScript dependencies for high-severity findings. |
| `pnpm quality` | Run the complete local ENG-001 safety-net suite. |

## Branch and commit conventions

Create a focused branch for every non-trivial change. Use a concise Conventional Commit-style subject such as `feat: add local project repository`, `fix: preserve paused privacy mode`, `test: cover migration failure`, or `docs: clarify key-handling boundary`.

Each pull request must have one coherent purpose. Do not combine UI redesigns, Tauri permission changes, data-model changes, and provider work in one pull request. Never commit generated build outputs, local databases, secrets, wrapped keys, screenshots containing user context, or production user data.

## Privacy-sensitive change checklist

A pull request that touches `src-tauri`, Tauri capabilities, local storage, synchronization, Windows APIs, OCR, capture, clipboard, microphone, files, or AI tooling must explicitly answer the following questions in its pull-request description.

1. What user-authorized action enables this behavior?
2. What data is read, generated, stored, transmitted, or deleted?
3. Where does that data remain, for how long, and how can the user remove it?
4. Which Tauri command, capability, or native permission is required, and why is it the narrowest one?
5. What happens when the user pauses capture, denies consent, is offline, or a native call fails?
6. Which tests prove that sensitive work is blocked without authorization?

The pull request must be split or escalated if any answer is unclear. Aura must not silently expand its perception or transmission surface.

## Pull-request completion protocol

Before requesting review, run `pnpm quality` locally and resolve all failures. Explain the product behavior changed, the non-goals preserved, the tests added or updated, and any manual Windows validation performed. Include a screenshot only when it contains fixture/demo content and is necessary to review renderer behavior.

A reviewer should be able to reproduce the checks from this document, identify the user-consent boundary, and understand the rollback or recovery path. Feature work cannot proceed when a required safety-net check is red.

## Troubleshooting

If `pnpm install --frozen-lockfile` fails after a dependency change, regenerate the lockfile only through the approved package manager command, inspect the diff, then rerun the command. If native Rust builds fail on Windows, first verify the supported Microsoft C++ build tools and WebView2 runtime are installed; do not work around native build errors by disabling the native checks. If a test depends on a Tauri command, mock the command in the renderer test and unit-test the relevant pure Rust policy separately.
