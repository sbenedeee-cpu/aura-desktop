# ADR-002: Start with intentional capture and local-first state

**Status:** Accepted  
**Date:** 2026-08-11

## Context

Aura’s value proposition involves project continuity and contextual assistance, but passive desktop monitoring creates disproportionate privacy, safety, and trust risks before the product has proven value. The V0 must make its data handling obvious and reversible.

## Decision

Aura V0 will begin with **user-initiated context markers** and local workspace state. The user can pause or resume intentional capture in the application. The native command layer rejects capture requests while the mode is paused.

No screen capture, OCR, clipboard monitoring, microphone access, background observation, or external AI call is part of this initial capability set.

## Consequences

The first release has a narrower feature envelope, but establishes the user-control model that later perception features must inherit. This makes the required next work explicit: a local data model, visible activity state, per-project exclusions, retention controls, redaction tests, and discrete Windows adapter permissions.

## Rejected alternative

A default-on activity timeline was rejected for V0. It would increase the threat surface and implementation scope before Aura has demonstrated that its project workspace materially improves continuity.
