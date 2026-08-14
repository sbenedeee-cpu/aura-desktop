# SEC-001: Windows DPAPI Key-Wrapping and Encrypted-Storage Proof of Concept

**Status:** In progress — approved by the product owner to begin on 2026-08-13.
**Date:** 2026-08-13
**Gate it closes:** The mandatory release-security gate from ADR-003 ("ENG-002 implementation gate") that stands before Aura ships persisted user data.

## 1. Purpose

ADR-003 accepted local SQLite persistence and committed Aura to **Windows DPAPI as the V0 key-wrapping boundary**, while explicitly deferring the encryption implementation choice to a later compatibility proof-of-concept. ENG-002 delivered persistence without encryption: the settings, captures, decisions, and projects stored in `aura.sqlite3` today are unencrypted at rest. SEC-001 is that proof-of-concept, executed as a narrow, vertically complete milestone that proves the key-wrapping design is real, testable, and safe — without yet re-encrypting the existing database or changing the renderer.

## 2. Locked Context

| Source | What it locks |
|---|---|
| ADR-003 | Random data-encryption key generated on first use, protected with the current Windows user's DPAPI context, stored **only** in wrapped form; raw key never written to logs, analytics, crash reports, or renderer state; renderer must not read raw key material |
| AGENTS.md | Renderer receives only narrow typed Tauri commands; no dependencies "for future use"; security-sensitive changes need documented rationale and tests |
| SET-001 (merged) | Existing typed privacy settings, exclusions, and atomic transitions must keep working unchanged |

## 3. Design

The proof of concept implements a `KeyVault` Rust module with three cooperating pieces.

**Key generation.** On first use, Aura generates 32 random bytes of key material (via `getrandom`). The plaintext key exists only in process memory for the duration of the unwrap operation; it is never written to disk, logs, or the renderer.

**DPAPI wrapping.** The key is wrapped with `CryptProtectData` under the current Windows user's context (`windows-sys` bindings, `CRYPTPROTECT_LOCAL_MACHINE` flag not set so the blob is bound to the user profile). Only the wrapped blob is persisted, to a file in the Tauri application-data directory. No database crate or schema change is required — the wrapped blob is a plain file.

**Enveloped encryption.** To keep the PoC database-agnostic (ADR-003 deferred crate selection), the `KeyVault` exposes **value-level envelope encryption**: `seal(plaintext) -> (nonce, ciphertext, tag)` and `open(ciphertext) -> plaintext` using ChaCha20Poly1305 (`chacha20poly1305` crate, AEAD). A sealed value is self-describing (version byte + nonce + ciphertext), so a future migration can re-seal values or move to file/database-level encryption without breaking old records.

**Linux compatibility.** `CryptProtectData` exists only on Windows. The `KeyVault` exposes a sealed `PlatformKeyWrapper` trait with a Windows implementation (DPAPI) and a sandbox-safe implementation for development/CI only, behind a compile-time `cfg` — never selectable at runtime, and gated by an explicit `dev` feature. All cross-platform tests run on the portable parts (key derivation shape, seal/open roundtrip, sealed-value versioning).

## 4. Scope

**In scope:**

1. New `src-tauri/src/security/key_vault.rs` with `KeyVault` (generate, wrap, store, unwrap, seal, open) and the platform trait
2. New `docs/decisions/ADR-004-encryption-strategy.md` recording the PoC outcome and the path to database-level encryption
3. Two new typed Tauri commands for diagnostic/verification only: `seal_secret` and `open_secret`, plus a `key_vault_status` command returning a summary (wrapped exists? sealed bytes? version?) — returning **no** raw key material
4. Unit tests covering: first-use key generation, wrap/unwrap roundtrip semantics on the portable layer, seal/open roundtrip, sealed-value version format, and renderer-invisible key material
5. README and work-package updates

**Out of scope (explicit non-goals):**

- Re-encrypting the existing `aura.sqlite3` database or its records (a later migration ticket, after the PoC is validated on real Windows)
- SQLCipher or any database crate upgrade
- Any renderer change beyond the diagnostic status summary command
- Cloud key material, export of key material, or multi-user key sharing

## 5. Acceptance Criteria

| # | Criterion |
|---|---|
| 1 | On Windows, `KeyVault::new` generates a 32-byte key, wraps it with DPAPI, and persists only the wrapped blob |
| 2 | `seal` / `open` roundtrip is lossless for arbitrary UTF-8 payloads and rejects tampered ciphertext (AEAD authentication failure) |
| 3 | No raw key bytes are ever written to disk, logged, or exposed through any Tauri command |
| 4 | The `key_vault_status` command returns only a summary; tests assert that no command output contains raw key material |
| 5 | At least 6 new unit tests pass on the CI matrix; strict Clippy and rustfmt remain clean |
| 6 | The full `pnpm quality` gate passes before merge |
| 7 | ADR-004 is accepted, recording the PoC result and the decision on full database encryption |

## 6. Branch and Delivery

Branch: `feat/sec-001-dpapi-encryption-poc`. Conventional commits. PR description must state ADR-003 gate closure, the PoC outcome, test evidence, and the non-goals. Merge only after the GitHub Actions quality gate passes.
