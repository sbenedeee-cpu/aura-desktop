# ADR-003: Local Storage, Migrations, and Windows Key Management

**Status:** Accepted — approved by the product owner to begin ENG-002 on 2026-08-13.

**Date:** 2026-08-12

## Context

Aura V0 must retain project continuity, user-created context markers, decisions, and privacy settings without silently observing the desktop or introducing a cloud dependency. ENG-001 deliberately establishes the decision boundary before ENG-002 adds persistence. The current app state is in memory only and must remain that way until this decision is approved and implemented through a dedicated vertical slice.

## Decision

Aura V0 will use a **single local SQLite database** owned by the Windows desktop app. The database will contain product records only: projects, context markers, decisions, activity records, settings, and schema metadata. It will not contain screenshots, clipboard contents, microphone audio, access tokens, provider credentials, or unapproved desktop captures.

The implementation will use a Rust-side repository layer. The React renderer will access local data only through explicit Tauri commands with typed request and response contracts. The renderer must never open the database directly, construct raw SQL, or manage an encryption key.

Schema migrations will be append-only, numbered, and committed with the feature that changes storage. A migration must be reversible where practical; when reversal is unsafe, the migration and its recovery/backup procedure must be documented. The application must record the successful schema version and fail safely with a clear local error if an upgrade cannot complete.

Aura will use **Windows DPAPI** as the V0 key-wrapping boundary. A random data-encryption key will be generated locally on first use, protected with the current Windows user’s DPAPI context, and stored only in wrapped form. The raw data-encryption key must not be written to logs, analytics, source control, crash reports, or renderer state. The database encryption implementation will be selected in ENG-002 after a compatibility proof-of-concept; no encryption or database crate is approved by this ADR alone.

## Consequences

| Area | Consequence |
|---|---|
| Privacy | Aura remains local-first by default. Data belongs to the current Windows user and is not synchronized automatically. |
| Product capability | V0 can provide durable project continuity, settings, and activity history without passive desktop capture. |
| Architecture | Native Rust owns persistence and key handling; React receives only minimal typed data needed for the visible UI. |
| Recovery | A failed migration must preserve the prior database file and show a user-readable recovery path rather than silently resetting data. |
| Future sync | A later sync feature must use an explicit export/sync boundary and cannot assume direct access to local key material. |

## Non-goals

This decision does not add a database dependency, schema, migrations, local file capture, Windows accessibility access, OCR, cloud synchronization, AI provider credentials, or team/multi-user support.

## ENG-002 implementation gate

This ADR is approved for ENG-002. The milestone must demonstrate the selected database package on Windows, document the exact storage location and backup semantics, add migration tests, and prove that the renderer process cannot read raw key material. The DPAPI-wrapped data-encryption-key proof of concept remains a completion gate before shipping persisted user data; no raw key material may be exposed to the renderer.
