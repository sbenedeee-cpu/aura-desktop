# EXP-003 Working Plan — Retention Sweep / Context-Ageing (Derived)

## Source of scope
- README.md "Next Build Increment" says: EXP-002 delivered; candidate: EXP-003 — retention sweep / context-ageing.
- EXP-002 work package: "README Next Build Increment updated to following item (candidate: EXP-003 — retention sweep / context-ageing, or revisit per roadmap)".
- DB already stores per-capture `retention` (until_deleted | review_in_30_days) and a global `default_capture_retention`. Nothing currently acts on it — no sweep exists. V0 PRD: "Record lifecycle" (MEM-05 expiry/delete; lifecycle state) is in scope.

## Definition of EXP-003 (adopted)
**Retention sweep with context-ageing:** a scheduled, local-only background job (on app launch + a periodic timer while running) that:
1. Classifies each capture with `review_in_30_days` whose `created_at` is >= 30 days old as `aged`.
2. `aged` captures are surfaced in a "Review" view (not deleted automatically): renderer shows a review list with actions "Keep (reset clock)"/"Delete".
3. Delete is user-initiated and reversible in the same session via an undo-able audit event (recorded in audit/Event table).
4. ADR-003/004 boundary preserved: sweep runs only in Rust; renderer gets typed DTOs; no network; capability manifest unchanged.
5. Sensitive-classified captures are NEVER auto-aged (they stay until manually deleted) — privacy-by-default.

## Components
1. `domain/retention.rs` — new `RetentionPolicy`/`ReviewDueDecision` types; deterministic clock-injection for tests.
2. `application/retention_service.rs` — pure service: classify captures, compute reviewable, mark aged; tests with fake clock.
3. `db/repositories/retention.rs` — persistence: lifecycle column (active/aged/deleted), `mark_aged`, `keep_capture`, `soft_delete`, `list_reviewable`, `list_aged`.
4. DB migration: add `lifecycle_state TEXT NOT NULL CHECK (lifecycle_state IN ('active','aged','deleted')) DEFAULT 'active'`, `age_reviewed_at` optional.
5. `commands`: `list_reviewable_captures`, `keep_capture(id)`, `expire_capture(id)`, `retention_sweep()` (runs the sweep, returns counts), `retention_policy()` (current policy summary).
6. `lib.rs` tauri command registrations.
7. Renderer: Settings "Data lifecycle" section OR a Review badge on TodayView; review dialog with keep/expired actions; typed tests.

## Convention (match EXP-001/002)
- Rust application services own transitions, validation, audit. Renderer stays typed/thin.
- Tests: lib unit tests (8+ new), App.test.tsx typed mocks.
- ADR update: ADR-003 or ADR-005? — this is lifecycle, better an ADR update to ADR-003 (local storage) + README Next Build Increment update.
- README.md "Next Build Increment" updated after merge with following candidate.
- CONTRIBUTING.md / security review applies to retention changes.
- CI: pnpm quality green.


## Implementation state (working notes)
- Branch: feat/exp-003-retention-sweep (from main after PR #14 merge).
- DONE:
  - Migration 7 in src-tauri/src/db/migrations.rs: adds lifecycle_state (active/aged/deleted) + lifecycle_updated_at to captures, index.
  - src-tauri/src/domain/retention.rs: LifecycleState (unused? kept for future), RetentionSweepResult, ReviewableCapture. (Removed dead RetentionSweepClock struct.)
  - src-tauri/src/application/retention_service.rs: RetentionService with classify_pass; tests (7 unit tests). REVIEW_AFTER_DAYS=30; sensitive + until_deleted protected.
  - src-tauri/src/application/mod.rs: added retention_service module.
  - src-tauri/src/db/repositories/captures.rs: LifecycleCapture, captures_for_retention_sweep(), age_capture(), keep_capture(), delete_capture(), lifecycle_capture_by_id(); create() inserts lifecycle columns.
  - src-tauri/src/lib.rs: commands run_retention_sweep, list_reviewable_captures, keep_capture, expire_capture + reviewable_from_lifecycle helper. Uses chrono Utc.
- TODO:
  1. Register new commands in tauri::generate_handler! (lib.rs ~line 745).
  2. cargo check/tests (48+ new tests).
  3. ExportService collect_captures should filter lifecycle_state='deleted'? (currently exports all) — decide: exclude 'deleted' from export counts & records; keep 'aged' and 'active'. Update export manifest record counts counts accordingly. Activity_records unaffected.
  4. Renderer: reviewable state + handlers in App.tsx; review dialog component; keep/expire buttons; wire to Settings or a new entry. Add CSS classes in App.css (quiet-action etc exist).
  5. App.test.tsx typed mocks for the 4 commands.
  6. README Next Build Increment update -> EXP-004 candidate (e.g., review queue + expiry polish OR search/filter). Update CONTRIBUTING? no.
  7. ADR-005 open questions: add note that EXP-003 closes lifecycle question? Actually lifecycle is ADR-003 domain; add brief ADR note or skip (CONTRIBUTING requires ADR for retention changes -> write docs/decisions/ADR-006-retention-sweep-local.md? check decisions list max).
  8. pnpm quality clean; commit; push; PR.
- Command invoke shapes (renderer):
  - run_retention_sweep: invoke<Result<RetentionSweepResult>>("run_retention_sweep", {})
  - list_reviewable_captures: {}
  - keep_capture: { captureId: string }  (tauri camelCase snake_case -> use snake args: input {capture_id}? Actually tauri passes args as object with camelCase param names; our fn param is `capture_id` so JS arg key is `captureId` (tauri converts).)
  - expire_capture: same.
- UI conventions: SettingsForm in App.tsx (2225+), SettingsView (2092+), Dialog pattern PassphraseExportDialog (1607), classes: dialog-backdrop, dialog-card, section-kicker, dialog-intro, field, field-error, primary-action, secondary-action, quiet-action, settings-note, settings-card, settings-actions, setting-choice, text-input, export-manifest, toggle-label, preference-row.
- App.tsx invoke pattern example: await invoke<null>("import_workspace", { input: { passphrase: ... } });
- Quality scripts: pnpm quality:renderer (format:check, lint, typecheck, test, build); pnpm quality:native (rust:fmt, rust:clippy, rust:test); pnpm quality; rust tests: pnpm rust:test (cargo test --manifest-path src-tauri/Cargo.toml).
- CI uses pnpm rust:clippy with -D warnings; rust 1.97.1 installed in this sandbox ($HOME/.cargo/env auto-sourced).


## Progress checkpoint 2 (post backend, mid renderer)
- Backend COMPLETE and green: 59 lib tests pass. Migration 6 adds lifecycle_state/lifecycle_updated_at. collect_captures in export_service filters deleted. New commands registered in generate_handler.
- Renderer in progress:
  - DONE: DTOs added (RetentionSweepResult, ReviewableCapture, emptySweepResult) after CaptureRetention; root state [reviewableCaptures, isLoadingReview, sweepNotice]; loadReviewQueue/useEffect for today view; runRetentionSweep/keepCapture/expireCapture handlers; TodayView invocation in App with new props.
  - TODO NEXT:
    1. Patch TodayView props: add to destructuring (after continuity): isLoadingReview, onExpireCapture, onKeepCapture, onRunSweep, reviewableCaptures; add to type block.
    2. Inside TodayView render (after ActivityRail or before): render ReviewQueue section: heading "Review"; intro: "Some local captures passed their 30-day review window. Decide to keep or remove each one." Buttons per capture: Keep (secondary), Remove (quiet-action destructive) w/ confirm; aria labels "keep_capture_X"/"expire_capture_X". If empty & !isLoadingReview: show quiet info "No captures need review. Run a sweep anytime from Settings → Data lifecycle." Also show "Run sweep now" button in settings card instead? Keep simple: ReviewRail section.
    3. SettingsForm "Data lifecycle" section: retention note + sweep button? Add to existing Export & recovery area? Decide: add to SettingsForm after exclusions section or inside export card. Keep minimal: add new settings card "Data lifecycle" w/ sweepNotice display? Simpler: no settings change, review surface only on TodayView + settings note inside Export card.
    4. App.test.tsx mocks + tests: list_reviewable_captures, run_retention_sweep, keep_capture, expire_capture (follow export test style: beforeEach invoke mocks line ~64-86/201-223/466-482; review tests beside export section 201-464).
    5. ADR: CONTRIBUTING says retention changes need ADR — create docs/decisions/ADR-006-retention-sweep-context-ageing.md (check existing decision numbers: ADR-005 exists; latest?).
    6. README Next Build Increment (lines 101-107): update "EXP-002 delivered" -> "EXP-003 retention sweep delivered"; candidate EXP-004 (e.g., "review queue polish + expiry window configurability" or revisit roadmap).
    7. pnpm quality (renderer: format lint typecheck vitest build; native: rustfmt clippy test), clippy needs -D warnings clean, commit "feat: add retention sweep and context-ageing review (EXP-003)", push feat/exp-003-retention-sweep, gh pr create --base main.
- UI conventions: classes dialog-backdrop dialog-card dialog-actions settings-card settings-note settings-actions quiet-action secondary-action primary-action field-error text-link section-kicker sr-only. SettingsForm starts ~2300 (after props), SettingsView 2170. PassphraseExportDialog pattern: 1690s.
- tauri invoke camelCase: JS arg {captureId} -> Rust capture_id param (tauri converts automatically).


## Progress checkpoint 3 (renderer mostly done, test fixes)
- Renderer UI complete: ReviewRail on TodayView (queue + Keep/Remove buttons + confirm dialog + review-notice), Data lifecycle card in SettingsForm ("Retention review" h3, "Run retention sweep" button, sweepNotice). All props wired; tsc clean; build green.
- Tests: App.test.tsx has EXP-003 describe block at end (4 tests); beforeEach mocks added to BOTH describe blocks. 3 fixed (heading->getByText), remaining failures: keep/expire notice text now in review-notice (added notice prop) — need rerun; sweep test failing because `<body><div /></body>` => render error: 'Settings' button not found. Body empty = component threw during render in test. Likely a missing className (review-notice) NOT the cause. Investigate: render throws — maybe `window.confirm` spy issue in the earlier test leaking? No, fresh test. More likely: in the new test the mock returns agedCapture but the render fails in ReviewRail... check: review rail uses notice prop (fine). Possibly the issue is test file top: `import` of waitFor? Check imports at top. OR the vitest -t output earlier showed body empty meaning render crashed; maybe a missing import in App.test.tsx (`within` used earlier in export tests? check top imports).
- TODO: debug sweep test render crash; rerun all 22 tests; clippy; format; ADR-006; README Next Build Increment update (EXP-003 delivered, next candidate); commit/push/PR.
- Commands: run_retention_sweep {}, list_reviewable_captures {}, keep_capture {captureId}, expire_capture {captureId}.
- Note: SettingsForm lifecycle card inserted between exclusion-card and export card (lines ~2664-2693).


## Progress checkpoint 5 (debugging remaining TS error in App.test.tsx)
The TS compiler reports `TS1005: '=>' expected` at `src/App.test.tsx:703:79`, i.e. the end of `it("runs a retention sweep from the data lifecycle settings card", async () {`. Whole-file brace balance is 0 with no negatives, and every visible line looks syntactically correct (the "deliberately removes" test opens with `async () => {` at line 681 and closes with `});` at 698). Root cause hypothesis: the probe-test insertion/removal edits may have left a duplicated or mangled line somewhere near 698-703 (e.g. a stray `});` pair), or an earlier edit removed the arrow from `async () => {` on line 681 or 703. Verify with targeted edits: re-check raw lines 681, 698-703 character-by-character; if line 681 shows `async () => {` and 703 shows `async () {` (missing =>), rewrite line 703. Note tsc error points at COLUMN 79 which is past the `{` at position ~78 — classic sign that the PREVIOUS statement (line 698 `  });`) is not seen as terminated, possibly because line 698 is actually `});` twice or missing. The failing suite otherwise: root cause of settings not rendering in the EXP-003 describe was a MISSING `get_privacy_preferences` mock in that describe's beforeEach — FIXED (added). After TS passes: run full vitest (expect 22 pass), then ADR-006 (docs/decisions/ADR-006-retention-sweep-context-ageing.md), README Next Build Increment update (EXP-003 delivered; candidate EXP-004 could be "configurable ageing window + sensitive-capture review bypass polish"), delete EXP003_PLAN.md or keep, commit `feat: add retention sweep and context-ageing review (EXP-003)`, push feat/exp-003-retention-sweep, gh pr create --base main.
Quality targets: pnpm quality = format + lint + typecheck + vitest + build + rustfmt + clippy (cargo 1.97.1 installed in sandbox, alias source "$HOME/.cargo/env"), 59 lib tests passing, 22 renderer tests. PR #14 merged; branch feat/exp-002 merged earlier.
