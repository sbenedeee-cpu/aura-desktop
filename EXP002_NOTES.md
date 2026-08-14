# EXP-002 Implementation Notes (updated)

## Repo state
- Branch: feat/exp-002-passphrase-portable-export (from e035f21 main). Work package docs/EXP-002_PASSPHRASE_PORTABLE_EXPORT_WORK_PACKAGE.md committed.
- Shell: must `source /home/ubuntu/.cargo/env` before any cargo command. `cargo add argon2@0.5` done in src-tauri.

## DONE so far
1. src/security/passphrase.rs — Passphrase (zero-on-drop, meets_strength_gate: >=12 OR >=8 w/ lower+upper+digit), PassphraseKey::derive(&passphrase, salt) argon2id (M=19MiB, T=2, P=1 → KEY_LENGTH 32), seal()/open() with v1 storage format (version byte + nonce + ct), generate_salt() 16 bytes. 8 unit tests (deterministic derivation, distinct keys, roundtrip, wrong passphrase, tampered, strength gate, zero on drop, no passphrase trace).
2. src/security/mod.rs registers passphrase module.
3. domain/export.rs: EXPORT_FORMAT_VERSION = 2; DPAPI_SEALING/PASSPHRASE_SEALING constants.
4. application/export_service.rs:
   - ExportEnvelope v2: added sealing: String (default "dpapi"), passphrase_salt_hex: Option<String>, passphrase_params: Option<PassphraseParams> (camelCase). PassphraseParams struct {memory_cost_kib, time_cost, parallelism} + owasp_defaults().
   - ExportService now holds passphrase: Option<Passphrase>, new() sets None, set_passphrase() setter.
   - assemble_export() unchanged DPAPI path, envelope includes sealing:"dpapi", None salts.
   - assemble_passphrase_export(passphrase: Passphrase) — strength gate check, collect records (same as DPAPI path, duplicated code), derive key from generate_salt(), seal with PassphraseKey::seal(), returns envelope sealing:"passphrase" with salt hex + params.
   - open_envelope(envelope) private helper: passphrase variant derives from self.passphrase + salt; else key_vault.open. Errors typed InvalidInput.
   - apply_import: parse envelope (v1-v2 accepted: format_version > 2 rejected, <1 rejected), payload_bytes = self.open_envelope, then utf8/checksum/counts/transaction as before. envelope_manifest same version tolerance.

## REMAINING
A. lib.rs: commands export_workspace/import_workspace/export_manifest currently no passphrase input. Need new signatures:
   - export_workspace(app, state, input: Option<ExportInput>?) — simplest: keep no input for DPAPI default and add `passphrase: Option<String>` via new input struct. BUT existing renderer calls `invoke("export_workspace", {})` — adding required input would break. Make input optional: `input: Option<ExportInput>` tauri can't take Option input struct directly; tauri commands accept zero-or-more args. Approach: change to `fn export_workspace(app, state) -> Result<ExportManifest, String>` for DPAPI and add passphrase arg only when... cannot overload. Simplest robust: make commands take no args; passphrase export routed through a NEW command `export_workspace_with_passphrase { passphrase: String }` returning ExportManifest. Import: change `import_workspace` to take `input: Option<ImportInput>` — tauri JS invoke with {} still works if args are Option<T>. Actually tauri 2: an `Option<T>` command arg binds null/undefined → None. So `fn import_workspace(app, state, input: Option<ImportInput>)` with ImportInput { passphrase: Option<String> } — renderer invoke("import_workspace", {}) still works (input=None). 
   - New export command signature: `fn export_workspace_with_passphrase(app, state, passphrase: String)`. Renderer invokes with passphrase string only when chosen.
   - export_manifest unchanged.
   - Wire new commands in invoke_handler list; import AppState pattern: `let mut store = state.store.lock()...` (Mutex).
   - Record events detail with sealing mode.
   - Import with passphrase: parse raw first to check sealing → if PASSPHRASE_SEALING and no passphrase given → error "passphrase required".
B. Tests in export_service.rs: existing 34 tests use ExportEnvelope fields — need to update envelope construction helpers (add sealing/passphrase fields) and set_passphrase for passphrase tests. NOTE: existing tests construct ExportEnvelope directly in envelope_manifest/apply_import tests — will fail compile; add defaults to helpers.
   - New tests: passphrase roundtrip (derive fresh vault → import passphrase archive), wrong passphrase rejection, tampered passphrase archive, v1 backward compat (build v1 envelope manually w/o sealing field), envelope contains no passphrase trace, weakness gate rejects weak passphrase.
   - Test helper: fresh_workspace_with_shared_key already exists (from EXP-001 tests); import-on-different-vault test must use shared key — passphrase archive works on ANY vault so fresh vault fine.
C. Renderer (Phase 3): App.tsx SettingsForm export card: radio for seal mode ("Seal with this computer's key (recommended)" default vs "Seal with a passphrase (opens on any computer)"), passphrase modal (enter+confirm, strength message, toggle), Export button sends passphrase command when mode=passphrase; import asks passphrase when restoring a passphrase archive (invoke import_workspace {passphrase} or modal before invoke); keep typed DTOs; add tests (passphrase path, wrong passphrase error, strength gate, dpapi default unchanged).
D. Quality: pnpm quality (format/lint/typecheck/test/build renderer + rust:fmt + rust:clippy + rust:test + audit). Cargo fmt + clippy must pass.
E. Phase 4-6: commit (feat: add passphrase re-sealing...), push, gh pr create → main, gh api actions runs to watch (check-runs API blocked for token; use workflow_runs list), merge when green, README update (EXP-002 delivered, next = candidate EXP-003 retention sweep/context ageing), ADR-005 status note, report.

## Renderer conventions (from EXP-001)
- SettingsForm card structure: <div className="settings-section"> kicker DATA OWNERSHIP, h3, rows; buttons use quiet-action / settings-actions; manifest dl aria-label="Export contents" role=list? (testing-library can't map dl to list reliably, use findByLabelText); CSS vars --line, --surface-muted, --radius-control; field-error class for errors.
- Tests: App.test.tsx `within` imported; mocks via msw-like invoke mocks at top of file (mockInvoke pattern); sampleExportManifest const.
- Handler names: exportWorkspace, importWorkspace, showExportManifest; state: exportBusy/exportError/exportNotice/exportManifest.
- export_workspace returns { exportedPath }; import_workspace returns {manifest fields}? (check lib.rs current returns ExportManifest; renderer invoke<null> ignored).

## Command payload shape decision (final)
- export_workspace(app, state) -> ExportManifest (DPAPI, unchanged; renderer invoke {})
- export_workspace_with_passphrase(app, state, passphrase: String) -> ExportManifest
- import_workspace(app, state, input: Option<ImportInput>) -> ExportManifest where ImportInput { passphrase: Option<String> } camelCase
- export_manifest(source_path: String) -> ExportManifest (renderer passes {} currently — works? lib.rs expects String; renderer passes {}→""? check earlier: renderer invoked {} and passed — maybe command signature actually (source_path: String) gets empty string... it reads file "" → error. Actually in EXP-001 tests maybe manifest preview was tested differently. Verify current renderer: showExportManifest invoke("export_manifest", {}) — if it worked in tests it's mocked. Check lib.rs: `fn export_manifest(source_path: String)` — if renderer passes {} source_path is "" and read fails. Possibly the renderer never actually used it in real run, or source_path gets serde default... No default. Tests mock invoke so it didn't surface. For EXP-002, make export_manifest take no args and read nothing? Better: remove the command and make renderer ask command to preview? Keep simple: make export_manifest take `source_path: Option<String>` and read the last exported path? Complex. Simplest: keep as-is (not part of EXP-002 scope), but fix: renderer should not pass {}; change command to not require path by having renderer invoke with source_path from... We can't get path. DECISION: change export_workspace to return manifest incl path AND add manifest preview only for the just-exported archive: remove export_manifest command, renderer shows manifest returned by export_workspace directly (no dialog). Simpler + cleaner: export_workspace already returns ExportManifest. Import preview: import_workspace returns ExportManifest before/after apply — renderer can show counts. So DROP export_manifest command and its renderer usage? That's scope drift. SAFEST: leave export_manifest as is (string arg; renderer passes {sourcePath} when available — but renderer never has path...). It currently passes {} and tests mock it, so it compiles/works in CI. Keep untouched to minimize scope.

## Clippy fixes remaining (all found)
1. lib.rs:576 — remove `mut` from service in export_workspace_with_passphrase (assemble takes &self; set_passphrase not used there).
2. passphrase.rs:291 — remove `mut` in zero-on-drop test.
3. export.rs:18 — DPAPI_SEALING dead code → allow(dead_code) with comment (public API for commands).
4. passphrase.rs:41 — TooWeak variant never constructed → replace error path: assemble_passphrase_export returns Err with AuraError::InvalidInput (not PassphraseError::TooWeak). Either allow or remove TooWeak; simplest: allow(dead_code) on variant with note it exists for future strength-gating hooks. Actually cleanest: use TooWeak in assemble_passphrase_export mapping — map error type. Do: return Err(...InvalidInput with gate message) already; add `#[allow(dead_code)]` on TooWeak since assemble_passphrase_export raises InvalidInput directly.
5. passphrase.rs:92 — remove unused len/is_empty methods.
Also export_workspace_with_passphrase needs mut removal.
After fixes: cargo clippy all-targets clean, cargo fmt, then Phase 3 renderer.

## Phase 3 renderer DONE so far
- ExportManifest type now has sealing: "dpapi" | "passphrase".
- New state in main App: exportSealing (default "dpapi"), passphraseModalOpen, passphraseDraft, passphraseConfirmDraft, passphraseDraftError, passphraseStrength, importPassphrase, importPassphraseError.
- Handlers: exportWorkspace (routes dpapi→exportWorkspaceDpapi or opens modal), confirmPassphraseExport (invokes "export_workspace_with_passphrase" {passphrase}), importWorkspace (invokes import_workspace {input:{passphrase}}), updatePassphraseDraft (strength gate same rule as native), updateImportPassphrase.
- PassphraseExportDialog component added before ProjectDialog (lines ~1616-1709): kicker PORTABLE ARCHIVE, h2 id="passphrase-export-heading", strength-meter span, fields draft/confirm, error field-error, buttons Cancel + "Seal and export…" (disabled busy||!draft||!confirmDraft). Uses dialog-backdrop/dialog-card/dialog-actions/dialog-intro/field classes.
- SettingsView + SettingsForm wired with new props (exportSealing, importPassphrase, importPassphraseError, passphraseDraft/ConfirmDraft/DraftError/ModalOpen/Strength, onSealingChange, onImportPassphraseChange, onPassphraseDraftChange, onPassphraseConfirmDraftChange, onPassphraseModalClose, onConfirmPassphraseExport, passphraseDraft).
- Export card UI: radios (setting-choice class) for seal mode; passphrase input (aria-label "Passphrase for restoring a sealed archive"); manifest dl gets "Sealed with" row.
- Main component SettingsView invocation updated; passphrase modal rendered when passphraseModalOpen.

## Remaining Phase 3
1. Add CSS: .strength-meter + .is-strong (reuse .field-error? no — new: near export-manifest or dialog CSS; use --line token) — add to App.css.
2. App.test.tsx: update sample manifest to include sealing; add tests:
   - default export invokes export_workspace (dpapi default unchanged)
   - passphrase radio → click Export opens dialog, confirms invoke export_workspace_with_passphrase {passphrase}
   - weak passphrase blocked client-side (strength gate)
   - mismatch blocked
   - import invokes import_workspace with input.passphrase
   - manifest shows Sealed with row
3. pnpm quality from /home/ubuntu/aura-desktop (runs prettier/lint/tsc/vitest build + rust fmt/clippy/test/audit). Then Phase 4 commit/push, PR, CI, merge, README (next increment EXP-003 context ageing/retention sweep), ADR-005 note, final report.
