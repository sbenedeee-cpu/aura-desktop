---
applyTo: "src/**/*.{ts,tsx,css}"
---

# Aura React and TypeScript Renderer Rules

Read `AGENTS.md` and `docs/AURA_ANTIGRAVITY_BUILDER_INSTRUCTIONS.md` before making changes.

The React renderer is a presentation and interaction layer. It must call typed Tauri command adapters; it must not contain SQL, native Windows logic, encryption keys, raw filesystem paths, provider secrets, or policy decisions that are not also enforced by Rust.

Use strict TypeScript. Define feature DTOs and request types instead of passing implicit `any`, loosely shaped JSON, or unvalidated global state. Keep feature-specific components, hooks, and test files close together. Avoid an unstructured `components/` dumping ground.

Every stateful view must implement a truthful loading state, empty state, error state, and success/update state. A persistence operation may only display success after the native command confirms a committed result. A failed command must leave the user’s visible state accurate and actionable.

Accessibility is part of feature completion. Use semantic HTML, labelled controls, visible keyboard focus, keyboard-accessible dialogs, `aria-live` status for asynchronous outcome messages, and non-colour-only status cues. Respect `prefers-reduced-motion`.

Aura privacy/capture state must be visible near any capture action. Never enable a capture control by default when the native layer is unavailable, paused, or unapproved. Do not claim data is saved, encrypted, AI-derived, live, or automatically captured without real supporting behavior.

For each new interaction, write or update component/hook tests. Test user-observable outcomes and failure behavior rather than internal implementation details. Prefer stable accessible queries over CSS selectors.
