# ADR-001: Use Tauri 2, React, TypeScript, and Rust

**Status:** Accepted  
**Date:** 2026-08-11

## Context

Aura needs a Windows-first desktop application that will eventually handle consented desktop context, local memory, privacy controls, and AI-provider integration. The product therefore needs a clear native boundary, strong permission discipline, and an interface layer that can iterate quickly.

## Decision

Use **Tauri 2** as the desktop shell, **React + TypeScript** for the user interface, and **Rust** for the native application core.

## Consequences

This preserves a clean separation between the interface and privileged native code. It makes Windows API adapters and local processing feasible without putting system access in renderer code. It also supports a future macOS/Linux path without making cross-platform support a V0 commitment.

Electron remains a viable alternative for a future team that prioritises mature Node-native ecosystem coverage over footprint and Rust-native boundaries. React Native Windows is not selected because Aura is a desktop-system product rather than a shared mobile-first experience and would still require a substantial native integration layer for its perception roadmap.

## Guardrails

The stack choice does not authorise passive capture, cloud transmission, arbitrary plugin installation, or autonomous computer use. Those are separate product and security decisions that require individual capability manifests and user-facing consent.
