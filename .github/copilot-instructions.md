# Aura Coding Agent Instructions

Read `AGENTS.md` first, then `docs/AURA_ANTIGRAVITY_BUILDER_INSTRUCTIONS.md`. They are the repository-wide source of truth for development behavior.

Aura is a Windows-first desktop application built with Tauri 2, React, TypeScript, Rust, and Vite. Implement a focused local-first V0: durable project continuity, manual context capture, memory/decision provenance, and explicit privacy controls.

Do not add passive capture, screenshots, clipboard monitoring, microphone capture, broad native permissions, cloud sync, authentication, AI providers, external tools, or secrets without the required approved ADR, ticket scope, security review, and test evidence.

Before editing, produce the Task Plan in Section 8 of the builder manual. After editing, produce the Completion Report in Section 13. Read `docs/AURA_V0_PRD_FOR_ANTIGRAVITY.md`, `docs/AURA_ANTIGRAVITY_ENGINEERING_WORK_PACKAGE.md`, relevant ADRs, source code, and tests before changing behavior.

Run the appropriate formatting, linting, type-checking, Rust checking, tests, and production build commands before committing. Keep every PR small, independently reviewable, and linked to specific acceptance criteria.
