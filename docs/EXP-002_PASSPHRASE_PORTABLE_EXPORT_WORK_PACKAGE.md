# EXP-002 — Passphrase Re-Sealing for Portable Archives

- **Status:** Proposed
- **Branch:** `feat/exp-002-passphrase-portable-export`
- **Date:** 2026-08-14
- **Readme commitment it delivers:** the "next product increment" named in README.md after EXP-001 — passphrase re-sealing for portable archives, built on the DPAPI-bound export envelope and recorded in ADR-005

## 1. Purpose

EXP-001 shipped the sealed-archive export path, but it carries one hard trade-off: the archive only opens on an Aura installation whose key vault can unwrap the envelope. A user who loses `aura.keywrap`, or who wants to move an archive to a different machine or reinstall Windows, currently has **no recovery path** — the archive is cryptographically dead.

EXP-002 closes that gap without weakening the default model: when exporting, the user can choose to **re-seal the archive with a passphrase-derived key** in addition to (replacing) the DPAPI-bound envelope. The same passphrase unlocks the archive on import, on any machine, with no network and no bundled DPAPI material. The DPAPI-only export remains the default and recommended mode; passphrase re-sealing is an explicit opt-in, because it moves the burden of secret custody from the OS boundary to the user's memory.

## 2. Locked Context

| Source                  | What it locks                                                                                                                                                                                                                                                                                   |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ADR-005 (accepted)      | Export envelope strategy, checksum-first import, transactional restore; ADR explicitly defers passphrase re-sealing to EXP-002                                                                                                                                                                  |
| ADR-004                 | Value-level ChaCha20Poly1305 envelope construction with DPAPI-wrapped key; raw key never touches disk                                                                                                                                                                                           |
| ADR-002                 | Intentional capture only; no passive observation; no network sync                                                                                                                                                                                                                               |
| AGENTS.md               | Renderer receives only narrow typed commands; no raw DB access; security-sensitive changes need documented rationale and tests                                                                                                                                                                  |
| EXP-001 (merged)        | `ExportService` (`src-tauri/src/application/export_service.rs`), domain DTOs (`src-tauri/src/domain/export.rs`), envelope `{format_version: 1, …}` shape, `export_workspace`/`import_workspace`/`export_manifest` commands, native-dialog-only paths, append-only `export_metadata` audit table |
| EXP-001 scope exclusion | The "no key export" exclusion is now lifted **only** for the passphrase path: the passphrase-derived key lives in process memory and is never persisted in any recoverable form                                                                                                                 |

## 3. Design

**Key derivation.** A passphrase re-seal derives a 256-bit key with **argon2id** (memory-hard, v1.3, OWASP parameters: 19 MiB memory, 2 iterations, 1 degree of parallelism — deliberately conservative, matching Rust `argon2` default params at OWASP-recommended memory) from a user-supplied passphrase plus a fresh random 16-byte salt. The salt, params, and key identifier are stored inside the envelope so any Aura install can re-derive. The passphrase **never** leaves process memory and is never written to disk, the database, or logs; Rust side clears the key buffer on drop where practical.

**New envelope variant.** The export envelope gains `format_version: 2` with a `sealing` discriminator: `"dpapi"` (EXP-001 behavior, unchanged, default) or `"passphrase"` (re-sealed with the derived key). A passphrase-sealed envelope carries the argon2id salt and params in plaintext next to the manifest (derivation parameters are non-secret), the payload checksum over the ciphertext, and the sealed payload — the passphrase itself appears nowhere in the file. Import detects the variant by reading the envelope JSON; `format_version: 1` continues to be accepted for backward compatibility.

**Opt-in flow.** Export becomes a two-step renderer flow when the user chooses the passphrase mode: (1) the user enters and re-confirms a passphrase in the renderer; (2) the renderer sends only the passphrase to `export_workspace` via a typed command; the native side derives the key, re-seals, and returns the manifest. A weak-passphrase gate (minimum 12 characters, or 8 with mixed case+digits — enforced server-side with a typed error) keeps casual mistakes out.

**Restore parity.** `import_workspace` accepts either variant. For the passphrase variant, the renderer collects the passphrase via the same confirmation UX and passes it to the command; the native side derives the key, verifies the checksum, and applies the transaction as before. Wrong-passphrase and tampered archives both fail before any record is written.

**Scope discipline.** No network, no cloud, no OS keyring, no bundled DPAPI blob, no passphrase hints or recovery email. The manifest remains plaintext and never leaks passphrase material or record content.

## 4. Scope

**In scope:**

1. New `src-tauri/src/security/passphrase.rs` — argon2id derivation with typed params and `#[repr(transparent)]`-friendly key wrapper that zeroes on drop
2. `ExportService` extended: `seal_variant` parameter (`Dpapi` | `Passphrase(passphrase)`), envelope version bumped to 2 with `sealing` discriminator
3. Import extended: variant detection, passphrase derivation, checksum-first verification, typed `WrongPassphrase` error
4. `export_workspace` and `import_workspace` commands accept an optional typed `Passphrase` payload; `export_manifest` unchanged (derivation params are excluded from the manifest view)
5. Renderer: passphrase modal (enter + confirm, strength gate, toggle between DPAPI default and passphrase portable mode) inside the existing Export & recovery card
6. Unit tests: derivation determinism (same salt+passphrase → same key), variant roundtrip for both sealings, wrong-passphrase rejection, tampered-archive rejection on passphrase variant, version-1 backward compatibility, envelope contains no passphrase trace
7. README + this work package status updates; ADR-005 status → Accepted with EXP-002 note

**Out of scope (explicit non-goals):**

- Passphrase storage, OS keyring integration, or any recovery/escape-hatch mechanism (losing the passphrase means losing the archive)
- Changing the default export mode — DPAPI remains the default
- Scheduled backups, partial exports, compression, or any network behavior

## 5. Acceptance Criteria

| #   | Criterion                                                                                                                                                                    |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | A passphrase-sealed export opens and restores on a freshly created vault with a different DPAPI boundary (proven by test: derive fresh vault, import the passphrase archive) |
| 2   | A wrong passphrase fails authentication **before** any record is written, with a typed error                                                                                 |
| 3   | A tampered passphrase-sealed archive fails the checksum before any record is written                                                                                         |
| 4   | The exported file contains the argon2id salt and params, the manifest, and the sealed payload — and no trace of the passphrase bytes                                         |
| 5   | `format_version: 1` DPAPI envelopes still import cleanly (backward compatibility)                                                                                            |
| 6   | The renderer never stores the passphrase in state visible to tests/serializers beyond the command call, and the weak-passphrase gate is enforced on the native side          |
| 7   | At least 8 new native tests pass; `pnpm quality` fully green; CI green on the PR                                                                                             |
| 8   | ADR-005 is updated: the open question closes with the EXP-002 resolution, and README tracks EXP-002 as delivered with the following next increment                           |

## 6. Branch and Delivery

- Branch `feat/exp-002-passphrase-portable-export` from `main` at the EXP-001 merge (`e035f21`)
- Commit message: `feat: add passphrase re-sealing for portable archives (EXP-002)`
- PR targeting `main`; merge after CI passes
- README "Next Build Increment" updated to the following item (candidate: EXP-003 — retention sweep / context-ageing, or revisit per roadmap)

## 7. Handoff Notes

`ExportService::assemble_export` currently takes a DPAPI-only sealing path via `self.key_vault.seal()`; EXP-002 introduces `Chacha20Poly1305::new(&derived_key)` sealing with `key_vault.open()` replaced by the derived-key open on the import path. The envelope JSON structure must remain parseable by the version-1 reader (detect `sealing` key; absent = `"dpapi"`). Renderer changes extend the Export & recovery card in `SettingsForm` (`App.tsx`), following SET-001/EXP-001 conventions (typed command mocks in `App.test.tsx`, `field-error`, `quiet-action` CSS classes).
