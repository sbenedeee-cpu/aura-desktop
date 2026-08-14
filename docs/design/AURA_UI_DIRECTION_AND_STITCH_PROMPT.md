# Aura UI Direction and Google Stitch Prompt

> **Superseded for implementation.** Use [`AURA_UI_REDESIGN.md`](./AURA_UI_REDESIGN.md) as the active visual specification for UX-001 and all renderer decisions. This earlier document remains only as historical exploration material.

**Status:** Historical exploration reference  
**Scope:** Aura Desktop V0 local-first project continuity  
**Authority:** This document translates the approved PRD visual and interaction contract into an implementable visual direction. It does not add product capability or relax any privacy rule.

## 1. Design Intent

Aura is a **calm, high-signal desktop workspace** for resuming meaningful work without re-explaining the project. It is not a generic productivity dashboard, surveillance console, or science-fiction control room. The interface should make three things legible in seconds: the selected project, the next deliberate action, and Aura’s current privacy state.

The visual character is **quietly premium and grounded**. It combines a warm charcoal canvas, restrained depth, crisp type, and a single accessible blue-violet accent reserved for user actions and explicitly understood status. Information density is deliberate: Today prioritizes scanning and resumption, Projects supports editing and selection, and future Activity/Memory screens become denser without becoming visually noisy.

## 2. Semantic Token System

| Token              |     Value | Role                                              |
| ------------------ | --------: | ------------------------------------------------- |
| `--canvas`         | `#121311` | Application background                            |
| `--surface`        | `#1A1B18` | Persistent navigation and standard cards          |
| `--surface-raised` | `#22231F` | Elevated panels and dialogs                       |
| `--surface-input`  | `#171815` | Editable fields and selected list rows            |
| `--border-subtle`  | `#34362F` | Card and table division                           |
| `--border-strong`  | `#505349` | Focus-adjacent structural emphasis                |
| `--text-primary`   | `#F3F4EE` | Primary reading text                              |
| `--text-secondary` | `#B7B9B0` | Supporting labels and explanatory text            |
| `--text-muted`     | `#82857B` | Metadata only, never the sole state indicator     |
| `--accent`         | `#8B8CF7` | Primary user action and selected state            |
| `--accent-hover`   | `#A5A6FF` | Action hover and keyboard focus ring              |
| `--accent-soft`    | `#2A2B53` | Low-emphasis selected context                     |
| `--success`        | `#78C99B` | Confirmed local save, paired with explicit text   |
| `--warning`        | `#E5B75E` | Sensitive-data and retention warnings             |
| `--danger`         | `#E58383` | Archive, delete, and irreversible-error treatment |

Use a 4px base spacing grid with a practical desktop scale of `4, 8, 12, 16, 20, 24, 32, 40, 48, 64`. Use `8px` radii for inputs and small controls, `12px` radii for panels, and no oversized glass effects. Motion is short and purposeful (`160–220ms`), with an equivalent reduced-motion presentation.

Typography uses **Inter** or the installed system UI sans stack. Use a 28px route title, 20px project title, 16px body, 14px supporting text, and 12px metadata. Status is always expressed with an icon or label in addition to colour.

## 3. Desktop Information Architecture

The persistent left sidebar contains these destinations, in this order: **Today, Projects, Capture, Memory, Activity, Settings**. Only Today and Projects are interactive in UX-001; future destinations must appear as clearly unavailable only if they are shown at all. Do not expose `Cortex` as a primary route.

The route header always shows the current surface, selected project scope, the visible privacy state, and one primary action. The selected project remains visible before an edit, capture, search, or archive action. At typical Windows desktop widths, secondary detail panels collapse before primary actions disappear. Critical controls remain keyboard reachable with visible focus rings.

## 4. Screen Specification

### 4.1 Today — Resume Work

The Today screen begins with a compact **Local-only** and **Paused / Manual only** status rail. The main continuity panel uses a short headline such as `Resume Aura Desktop` only when a project has been selected. It displays the user-recorded goal, current task, next step, unresolved blocker, latest decisions, and recent activity in clearly labelled sections. Absence is explicit: `No blocker recorded`, `No decisions recorded`, and `No recent activity yet`.

The primary action is `Open project`; the secondary action is `Add deliberate context`, which remains disabled with a clear explanation when privacy is paused. Today must never invent a summary or describe future AI activity as live.

### 4.2 Projects — Browse and Act

The Projects screen uses a left project list and a right detail panel. The list rows show the project name, status, next step or first-use copy, and last local activity. The selected row uses the accent-soft surface and an explicit `Selected` cue. A search/filter affordance is present only if it is fully implemented.

The right panel displays editable project fields: name, one-line goal, current task, blocker, and next step. Save behavior is explicit: idle → validating → saving → saved or recoverable error. A slim activity section shows safe metadata only. The archive action is visually separated, destructive-coloured, and invokes a confirmation dialog that states the project will disappear from default views while its audit history remains.

A first-use empty state says: `Start with a project you want to resume without re-explaining.` It offers `Create project` as the only primary action.

### 4.3 Create Project — Focused Modal

The create-project modal has a concise title, a plain-language privacy note (`Saved only on this device in this local release.`), labelled inputs for project name, optional goal, and optional first next step. It validates name input inline and offers `Cancel` and `Create project`. Closing, Escape, and Cancel create no record. The post-save state names the created project and moves focus predictably to it.

### 4.4 Archive Confirmation — Honest and Reversible

The archive dialog identifies the exact project by name and explains: `Archiving hides this project from default views. It does not erase its local history.` It offers a neutral `Cancel` and a destructive `Archive project` action. Keyboard focus begins inside the dialog and Escape cancels safely.

## 5. Required Interaction States

| State               | Design behavior                                                                                                        |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| Loading             | Preserve route layout, use text-labeled skeletons, and avoid implying data is current before the native read succeeds. |
| First use           | Explain the next safe action and do not render fabricated sample projects.                                             |
| Validation error    | Show the message beside the field, preserve user input, and move focus to the first actionable error after submit.     |
| Saving              | Disable duplicate submission, retain context, and show the affected record.                                            |
| Saved               | Use concise confirmation such as `Project saved locally`; do not use celebration animation.                            |
| Recoverable error   | Explain that the data was not confirmed as saved, preserve form contents, and offer retry or safe cancel.              |
| Paused privacy      | Show `Paused — manual context cannot be saved` next to the relevant action, not only in Settings.                      |
| No selected project | Explain the scope requirement and offer a clear project-selection action.                                              |

## 6. Google Stitch Prompt

Paste the following prompt into Google Stitch once the design workspace is usable. Generate **desktop app screens**, not a marketing page and not an imagined AI dashboard.

```text
Design a polished Windows desktop application named Aura: a privacy-first, local-first personal project-continuity workspace. Create a high-fidelity dark-mode desktop UI for a calm, credible productivity product. This is NOT a surveillance console and NOT an AI chatbot. It helps a solo user resume a selected project from deliberate, locally stored records.

Visual direction: refined warm-charcoal desktop interface, tactile but restrained depth, high legibility, no neon, no glassmorphism excess, no sci-fi effects, no decorative gradients. Think Linear’s composure, Notion’s clarity, and a premium native desktop feel. Use Inter or a modern system sans. Use a 4px spacing grid, 12px panel radius, precise 1px borders, quiet shadow depth, and visible focus states.

Use this palette: canvas #121311, standard surface #1A1B18, raised panel #22231F, input surface #171815, subtle border #34362F, primary text #F3F4EE, secondary text #B7B9B0, muted text #82857B, action accent #8B8CF7, accent surface #2A2B53, success #78C99B, warning #E5B75E, danger #E58383. Accent colour is reserved for primary actions and selected state. Do not communicate state with colour alone.

Generate four cohesive desktop screens at 1440px wide with realistic content, each with a fixed left sidebar. Sidebar items: Today, Projects, Capture, Memory, Activity, Settings. Only Today and Projects are active; the rest should look deliberately unavailable or secondary, never falsely active. Add a slim status area that says “Local only” and “Manual only” with a visible pause control.

Screen 1 — Today / Resume Work: Header “Today”; selected project chip “Aura Desktop”; privacy badge “Manual only”. Large continuity panel headed “Resume Aura Desktop” with user-recorded sections: Goal “Ship a trustworthy local-first Windows app”, Current task “Complete the Projects and Today vertical slice”, Next step “Review the selected project brief”, Blocker “No blocker recorded”, Latest decisions “Tauri is selected for Aura V0”, and a compact recent-activity timeline. Primary button “Open project”; secondary button “Add deliberate context”. Use exact truthful empty-state language such as “No decisions recorded” where needed.

Screen 2 — Projects: Header “Projects”; create button “New project”. Two-column layout. Left list with “Aura Desktop”, “Great Seeds Website”, “Eternal Studios”, each showing a next step and last local activity. Highlight Aura Desktop with an explicit Selected label. Right detail panel with labelled editable fields: Project name, Goal, Current task, Blocker, Next step. Include visible save state, “Saved locally” confirmation, a local activity section, and an isolated destructive “Archive project” action.

Screen 3 — First-use / no projects: Header “Projects”; strong quiet empty state: “Start with a project you want to resume without re-explaining.” Supporting copy: “Aura stores this local release on this device. Nothing is captured in the background.” Primary button “Create project”. Include an approachable create-project modal with labelled Project name, optional Goal, optional First next step, Cancel, and Create project.

Screen 4 — Archive confirmation: retain the Projects screen behind a focused accessible modal. Modal title “Archive Aura Desktop?” Copy: “Archiving hides this project from default views. It does not erase its local history.” Secondary action “Cancel”; destructive action “Archive project”. Show keyboard focus clearly.

Important UX constraints: show meaningful loading, empty, validation, saved, paused, and recoverable-error states. Never claim automatic capture, AI analysis, encryption, sync, or cloud activity. Every important action must make the selected project and local privacy state visible.
```

## 7. Implementation Guardrails

The Stitch output is a visual reference only. The React implementation must preserve Aura’s typed Tauri boundary, use semantic CSS tokens rather than image-like visual styling, and meet the keyboard, focus, reduced-motion, responsive-desktop, and truthful-state requirements in the PRD. No external asset, provider, permission, or API is required by this visual direction.
