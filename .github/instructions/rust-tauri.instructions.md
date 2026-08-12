---
applyTo: "src-tauri/**/*.rs"
---

# Aura Rust and Tauri Native Layer Rules

Read `AGENTS.md`, `docs/AURA_ANTIGRAVITY_BUILDER_INSTRUCTIONS.md`, the relevant ADRs, and applicable Rust tests before editing native code.

The Rust layer owns domain validation, policy enforcement, timestamps, IDs, persistence, migration execution, transactions, audit/event creation, and conversion of internal failures into safe typed DTOs. The React renderer is not a trusted security boundary.

Keep `#[tauri::command]` functions thin. Commands should deserialize a narrow request DTO, call an application service, and return a narrow result/error DTO. Business rules, repository access, and privacy gates belong beneath command handlers in explicit modules.

All future durable writes must be transactional when more than one record is affected. Project-scoped reads and writes must validate `project_id`; do not implement convenience queries that return records across all projects by default. Event history is append-only; corrections create superseding records instead of destructive in-place mutation.

Never return raw database errors, filesystem paths, secrets, internal stack traces, or captured sensitive content to the renderer. Use stable error codes, safe messages, retryability, and user-action guidance.

Do not add a Tauri plugin, capability, native API, external network request, or background task without an approved ticket, ADR, least-privilege analysis, and test plan. Native capture must remain unavailable unless the policy layer, consent state, and approved feature flag all allow it.

Run `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, and `cargo check` for relevant native changes. Unit-test policy decisions and repository errors; integration-test transactions, migration behavior, project isolation, and failure rollback.
