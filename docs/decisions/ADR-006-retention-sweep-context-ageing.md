# ADR-006: Retention sweep and context-ageing review

- Status: accepted
- Date: 2026-08-14
- Deciders: Aura core team (Manus, Antigravity, Jules)
- Supersedes: —

## Context

Aura's manual-capture model means every capture is deliberate, but deliberate context still accumulates. Older captures that are never re-read or decided on clutter the workspace and quietly contradict the promise of an intentional system. Captures are private by default, so any automatic handling of them must be conservative: the sweep must never remove data, must never re-read anything marked sensitive without consent, and must never override a user's explicit retention choice.

The capture table already carries a retention policy (`until_deleted`, `review_in_30_days`) and a classification (`standard`, `sensitive`) established in SET-001. These fields are the natural hooks for an ageing policy that operates only on rows the user has already consented to age.

## Decision

Aura ships a **retention sweep (EXP-003)** with these properties.

1. **Review-first ageing.** Standard captures on the `review_in_30_days` policy that pass thirty days without a user decision move to an `aged` lifecycle state and surface in a review queue on the Today view. Ageing is never deletion: the row is preserved, an audit event is written, and the only automatic effect is a change of state plus a review entry.
2. **Protected categories are never aged automatically.** Sensitive captures and captures with `until_deleted` retention stay `active` forever until the user decides otherwise. The sweep counts them as protected in its result but never touches them.
3. **Explicit user decisions only.** A capture leaves the review queue only through a deliberate Keep (`active` again, clock resets) or Remove (`deleted`, irreversible, double-confirmed) action, each written as an audit event. `deleted` rows are excluded from exports so removed context never reappears in portable archives.
4. **Clock-isolated, pure policy.** The ageing policy lives in an application service that takes an injected clock, so tests exercise the rules deterministically. The repository layer owns the SQL transitions and the sweep counts.
5. **No polling.** The review queue loads when the Today view opens; there is no background timer. This keeps the app honest about being a tool the user opens rather than a process that runs behind them.
6. **Export consistency.** The export service's capture collection now excludes `deleted` rows, matching the semantics established in EXP-001/EXP-002: an export reflects the current local workspace, and removed context stays removed.

## Consequences

The captures table gains three lifecycle columns via migration 6 (`lifecycle_state`, `lifecycle_updated_at`), so the schema now runs to six migrations. Four new typed commands are exposed (`list_reviewable_captures`, `run_retention_sweep`, `keep_capture`, `expire_capture`), and the Settings view gains a Data lifecycle card alongside the existing Export & recovery card. The 30-day window is hardcoded for EXP-003; EXP-004 will expose it as a bounded user preference.

The main risk — accidentally ageing context the user wanted to keep — is mitigated by the protected-category rule, the review-first gate, and the fact that a Keep action always restores an aged capture. The secondary risk — silent data loss on Remove — is mitigated by the irreversible label, the double confirmation, and the audit event.

## Open questions

Whether audit events for lifecycle transitions should be individually visible in the Memory view is deferred to EXP-004, along with the configurable ageing window.
