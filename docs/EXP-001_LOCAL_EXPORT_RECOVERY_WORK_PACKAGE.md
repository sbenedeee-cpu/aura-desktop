# EXP-001 — Local Export and Recovery Controls

- **Status:** Proposed
- **Branch:** `feat/exp-001-local-export-recovery`
- **Date:** 2026-08-14
- **Readme commitment it delivers:** the "next product increment" named in README.md after SEC-001 — local export and recovery controls, built on the sealed-value boundary

## 1. Purpose

Aura's privacy promise cuts both ways. "Everything stays local" protects the user from unwanted egress, but it also means the user must be able to **take their data with them** and **restore it when something goes wrong** — without Aura ever phoning home, uploading a backup, or introducing a cloud dependency.

EXP-001 delivers the user-facing side of that promise as a narrow, vertically complete milestone: a Settings section where the user can export their entire local workspace to a single encrypted archive file, verify what the export contains through a plain manifest, and restore (re-import) it later on the same or another machine. It closes the "lock-in" concern that a privacy-first product otherwise invites: Aura must prove the user owns their data completely.

## 2. Locked Context

| Source           | What it locks                                                                                                                                                                                                                      |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ADR-003          | Persistence is local SQLite in the Tauri app-data directory; migrations are numbered, append-only, and transactional                                                                                                               |
| ADR-004          | Value-level envelope encryption with DPAPI-wrapped key is the V0 boundary; the raw key never leaves process memory; only the wrapped blob is persisted                                                                             |
| ADR-002          | Intentional capture only; no passive observation; no screenshots, clipboard, microphone, or network sync                                                                                                                           |
| AGENTS.md        | Renderer receives only narrow typed Tauri commands; no raw DB access from the renderer; no dependencies added "for future use"; security-sensitive changes need documented rationale and tests                                     |
| SEC-001 (merged) | The `KeyVault` (`src-tauri/src/security/key_vault.rs`) exposes `seal`/`open` for value envelopes and `key_vault_status`. Export payloads must stay inside the typed command layer — the renderer never reads the database directly |
| SET-001 (merged) | Privacy settings, retention defaults, and exclusion rules are stored in `settings`/`exclusion_rules` and must be included in the export                                                                                            |

## 3. Design

**Export.** A single Tauri command `export_workspace(destination_path)` writes the entire user workspace — projects, settings (privacy mode, default retention, exclusions), manual captures, decision claims with provenance, and migration metadata — to one file at a user-chosen path (via the native file dialog, not a renderer-provided path). The file format is a versioned, self-describing JSON envelope signed with the DPAPI-wrapped data-encryption key: `{ "format_version": 1, "exported_at", "record_counts", "checksum", "payload": <ChaCha20Poly1305 sealed JSON> }`. The manifest (format version, export time, record counts, checksum) is plaintext so a human can inspect it without decrypting; all record content stays sealed. The exported key is **not** bundled: the archive can only be opened by an Aura whose key vault can unwrap the envelope, preserving DPAPI user binding.

**Import.** A paired command `import_workspace(source_path)` reads an archive, validates the envelope structure and checksum, decrypts and deserializes the payload with `KeyVault::open`, and writes it through the **existing typed repositories inside a single transaction** — reusing the same validation paths as normal writes. Import never overwrites silently: it requires explicit confirmation (renderer-visible consent step), and on conflict Aura fails with a descriptive error rather than merging half-applied data. The import transaction either fully applies or fully rolls back.

**Scope discipline.** No network, no upload, no cloud reference anywhere in the implementation. The renderer only gets three typed commands: `export_workspace`, `import_workspace`, and an `export_manifest(source_path)` preview command that returns the plaintext manifest plus a human-readable record inventory (no decrypted record content).

**Path handling.** Destination/source paths come from the native file dialog only (`dialog::save` / `dialog::open`), so the renderer cannot point Aura at arbitrary filesystem locations — the capability boundary stays minimal.

**Format versioning.** The envelope carries `format_version: 1`. The import path validates the version and rejects unknown versions with a typed error, so a future format change is a controlled migration rather than a silent break.

## 4. Scope

**In scope:**

1. New `src-tauri/src/domain/export.rs` — typed export payload DTOs (never raw DB types)
2. New `src-tauri/src/application/export_service.rs` — export assembly (repository reads) and import application (transactional repository writes), including checksum validation
3. Three typed commands: `export_workspace`, `import_workspace`, `export_manifest`
4. Migration 5 (no-op schema-wise): registers `export_metadata` table recording each export/import event (append-only, auditable)
5. Settings renderer section: "Export & recovery" with Export action (native save dialog + progress), Manifest preview, and Import action (native open dialog + explicit confirmation step)
6. Unit tests: envelope roundtrip, manifest integrity, import transaction rollback on conflict, version rejection, renderer command payload assertions
7. README + this work package status updates

**Out of scope (explicit non-goals):**

- Key export or portable-key recovery (the archive is bound to the DPAPI key — losing the key means losing the archive; that trade-off is documented, not papered over)
- Scheduled/automatic backups
- Partial exports (per-project) — single full-archive format in V0
- Compression or incremental/delta exports
- Any network behavior whatsoever

## 5. Acceptance Criteria

| #   | Criterion                                                                                                                                                                                                                                  |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1   | `export_workspace` writes exactly one file containing all projects, captures, decisions, settings, exclusions, and migration metadata; the manifest is plaintext and human-readable, all record content is sealed with the DPAPI-bound key |
| 2   | The export contains no screenshots, clipboard contents, microphone data, credentials, or network references                                                                                                                                |
| 3   | `import_workspace` decrypts and validates the envelope, applies all records through the typed repositories inside one transaction, and rolls back entirely on any conflict or validation failure                                           |
| 4   | Each export and import attempt is recorded in the append-only `export_metadata` table with a typed event record                                                                                                                            |
| 5   | The renderer cannot choose arbitrary paths (native dialog only) and cannot read decrypted record content — only manifests and status                                                                                                       |
| 6   | At least 8 new tests pass covering envelope roundtrip, manifest integrity, import rollback, version rejection, and renderer command payloads                                                                                               |
| 7   | Strict Clippy (`-D warnings`, both profiles), `cargo fmt`, renderer quality, and `pnpm audit` all pass; CI green on the PR                                                                                                                 |
| 8   | ADR-005 (proposed alongside) records the export/recovery decision and the key-binding trade-off                                                                                                                                            |

## 6. Branch and Delivery

- Branch `feat/exp-001-local-export-recovery` from `main` at the SEC-001 merge (`59851ea`)
- Commit message: `feat: add local export and recovery controls (EXP-001)`
- PR targeting `main`; merge after CI passes
- README "Next Build Increment" updated to the following item

## 7. Handoff Notes

The `KeyVault` (`src-tauri/src/security/key_vault.rs`) provides `KeyVault::seal(bytes)` / `KeyVault::open(bytes)` used by the export envelope; `LocalStore` (`src-tauri/src/db/mod.rs`) provides the typed repository readers/writers that the export service must use — no direct SQL in the service layer. Renderer changes extend the existing `SettingsForm`/`SettingsView` pattern (`App.tsx`), following the SET-001 conventions (typed command mocks in `App.test.tsx`, `preference-row`/`quiet-action` CSS classes).
