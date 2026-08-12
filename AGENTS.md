# Aura Repository Agent Instructions

Aura is a **Windows-first, local-first personal project-continuity desktop application**. This repository is implemented with **Tauri 2, React, TypeScript, Rust, and Vite**.

## Read First

Before making any change, read:

1. `docs/AURA_ANTIGRAVITY_BUILDER_INSTRUCTIONS.md`
2. `docs/AURA_V0_PRD_FOR_ANTIGRAVITY.md`
3. `docs/AURA_ANTIGRAVITY_ENGINEERING_WORK_PACKAGE.md`
4. Relevant ADRs in `docs/decisions/`
5. The files and tests related to the requested feature

The detailed builder manual is authoritative for product scope, architecture, privacy, task planning, test evidence, and completion reports.

## Non-Negotiable Constraints

- Aura V0 is **manual and intentional capture only**. Never add background screenshots, clipboard monitoring, microphone capture, passive activity collection, or silent inference without an explicit approved ADR and product decision.
- The renderer must not access raw native APIs, file paths, database keys, encryption material, or external-provider secrets. Use narrow typed Tauri commands.
- Do not add Tauri permissions, plugins, filesystem/shell/HTTP access, or dependencies “for future use.” Every new capability needs a specific feature, documented data flow, least-privilege rationale, tests, and review.
- Every durable record must preserve the applicable `project_id`, provenance, timestamp, and correction/supersession relationship. Never allow default cross-project retrieval.
- Never label sample data, placeholders, or future functionality as live, active, saved, encrypted, AI-powered, or automatically captured.
- Do not introduce cloud sync, authentication, AI providers, embeddings, or automated agents before the relevant local-first gates and security design spikes are approved.

## Required Working Method

Start every task with the **Task Plan** format in Section 8 of the builder manual. Keep changes narrow and vertically complete. Implement user-visible behavior, failure states, accessibility, tests, and documentation together. End every task with the **Completion Report** in Section 13.

When a proposed change affects permissions, retention, privacy, external transmission, secret handling, database schema, or architecture, create/update an ADR and request a decision before implementation if uncertainty remains.

## Quality Baseline

Before committing, run the relevant commands from `README.md` and the engineering work package: formatter, lint, TypeScript check, frontend tests, Rust format, Clippy, Rust tests, and production build as applicable. Fix all new warnings and test failures. Do not suppress a failure without a documented, reviewed reason.

## Git Discipline

Use one focused branch and one focused pull request per ticket. Do not mix refactors with feature work. Commit messages use conventional prefixes such as `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `build:`, or `chore:`. The PR description must link the ticket, acceptance criteria, test evidence, privacy impact, and known limitations.
