# ADR-005: Export and Recovery Strategy with DPAPI-Bound Archives

**Status:** Proposed
**Date:** 2026-08-14
**Supersedes:** (new; builds on ADR-003 and ADR-004)
**Affects:** EXP-001 local export and recovery controls

## Context

Aura V0 stores every record locally and encrypts sealed values with a random 32-byte data-encryption key wrapped by Windows DPAPI (ADR-003, ADR-004). A user who backs up, migrates machines, or recovers from a disk failure needs a way to move their entire workspace, but Aura must not introduce any network dependency, any cloud custody, or any passive-exposure surface to accomplish it.

## Decision

Aura exports the entire workspace as a single **sealed JSON envelope** written to a user-chosen path through a native file dialog, and restores it through an **all-or-nothing transactional import** validated by a SHA-256 checksum computed over the ciphertext.

1. **Envelope contents.** The export carries the complete record set (projects, captures, decision claims and sources, exclusion rules, settings) as versioned, camelCase JSON, plus a manifest recording format version, export timestamp, per-type record counts, the payload checksum, and the sealed payload length. The renderer only ever receives this typed manifest; it never reads the archive file itself.
2. **Encryption.** The payload is sealed with the same ChaCha20Poly1305 envelope construction as SEC-001, using the DPAPI-unwrapped workspace key. The archive therefore opens only on an Aura installation holding the same workspace key (the same Windows user boundary, or a vault file deliberately copied along with the archive).
3. **Portability model.** V0 deliberately does **not** bundle the unwrapped key, does not derive an additional key from user input, and does not perform passphrase re-sealing. A passphrase-based re-seal path is deferred to EXP-002 so that V0 ships the simplest sound model: the archive is portable to any machine where the vault file (`aura.keywrap` plus the envelope version) is present, and unusable elsewhere.
4. **Integrity.** Export computes SHA-256 over the raw ciphertext and persists a per-event audit row in the `export_metadata` table (append-only, sequence-ordered). Import verifies the checksum before opening the envelope and applies every record type inside a single SQLite transaction; any failure (tamper, damaged ciphertext, schema conflict, or foreign-key violation) rolls the import back completely, leaving the workspace untouched.
5. **Path control.** The renderer never constructs or chooses file paths. Save and pick paths come exclusively from native file dialogs (`tauri-plugin-dialog`), and the capability manifest is not expanded beyond the dialog permission.
6. **Rollback semantics.** Import is insert-or-update keyed on record identifiers: pre-existing records are merged deterministically rather than duplicated, while the whole batch remains atomic.

## Consequences

**Positive.** Users now own a verifiable, encrypted copy of their workspace that can survive disk loss or machine migration (with the vault file). Recovery is provably atomic and tamper-evident, and every export event is auditable. The renderer remains thin and typed.

**Negative.** An archive cannot be opened on a machine whose vault differs, and there is currently no password fallback if the user loses the vault file; losing `aura.keywrap` means losing the archive. The manifest does not leak plaintext, but record counts and timestamps are visible in it by design. Cross-platform portability is Windows-user-bound until EXP-002.

## Open Questions

- EXP-002: whether to add passphrase re-sealing (argon2id key derivation) as the standard portability path, and whether the DPAPI-only default remains the recommended mode.
- Whether archive contents should optionally be purged from memory and re-sealed on re-import (currently accepted as-is, since the archive is written once by the user).
