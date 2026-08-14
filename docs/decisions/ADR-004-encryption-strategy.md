# ADR-004: Encryption strategy for persisted local data

- **Status:** Accepted
- **Supersedes:** the encryption gate clause of [ADR-003](ADR-003-local-storage-and-key-management.md)
- **Date:** 2026-08-14

## Context

ADR-003 committed Aura to local SQLite persistence and named Windows DPAPI key wrapping as a mandatory release gate before Aura ships persisted user data. SET-001 shipped the privacy-control surface, but the encryption gate itself was still open: the database file and the settings store remained unencrypted at rest.

Three candidate strategies were evaluated for V0:

1. **Full database encryption** (SQLCipher-style at-rest encryption of the whole file).
2. **Value-level envelope encryption** with a DPAPI-wrapped data-encryption key, applied through the typed command layer only.
3. **No encryption beyond OS file permissions** (rejected immediately: it fails ADR-003's explicit gate and offers no defense if the app-data folder is copied).

## Decision

Aura adopts strategy 2 as the V0 boundary, with the following concrete rules:

- **Key generation.** On first use, Aura generates a 32-byte random data-encryption key from the platform entropy source. The raw key is never written to disk, logs, crash reports, analytics, or renderer state.
- **Key wrapping.** The data-encryption key is protected with Windows DPAPI (`CryptProtectData` under the current user's context) and only the wrapped blob (`aura.keywrap`) is persisted. On non-Windows builds, the DPAPI implementation is excluded at compile time; a development-only stand-in exists solely so the portable logic can be exercised off Windows and is never selectable at runtime.
- **Value envelopes.** Secrets are encrypted with ChaCha20Poly1305 (AEAD). Each sealed value carries a version byte, a random 12-byte nonce, and the authenticated ciphertext, so envelopes are self-describing and future format versions can coexist during migration.
- **Surface.** Three diagnostic Tauri commands (`seal_secret`, `open_secret`, `key_vault_status`) exercise the boundary through the typed command layer. None of them returns raw key material or the wrapped blob; `key_vault_status` returns only a minimal summary.
- **Scope in V0.** Encryption is value-level, not database-level: the SQLite file itself is not re-encrypted. Records flow through the typed commands and settings layer; the envelope boundary is the V0 guarantee.

## Consequences

- **Positive.** The DPAPI gate from ADR-003 is satisfied: user-controlled key protection is enforced by Windows, the raw key never touches disk, and tampered ciphertext fails authentication. V0 ships without adding database-level encryption complexity.
- **Positive.** The versioned envelope format lets a future database-wide migration re-seal records or adopt an encrypted database crate without breaking stored values.
- **Trade-off.** Only data that flows through the sealed-value commands gains envelope protection; raw SQLite rows remain unprotected. This is a conscious V0 scope boundary, not a gap to paper over in documentation.
- **Open item.** The Windows DPAPI path (`CryptProtectData`/`CryptUnprotectData`) has been compiled against the windows-sys bindings and reviewed statically, but has not yet been exercised on a real Windows runtime. A Windows build verification (local or CI) is required before this is considered fully validated on the target platform.
