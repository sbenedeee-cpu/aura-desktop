# Aura UI Redesign — The Continuity Desk

**Status:** Active visual specification for UX-001  
**Supersedes:** `AURA_UI_DIRECTION_AND_STITCH_PROMPT.md` for implementation decisions  
**Visual reference:** `aura-desk-ui-reference.png`  
**Product constraint:** The interface may be polished, but it may not claim capture, AI analysis, encryption, cloud sync, or decision history that Aura has not actually implemented.

## 1. Product Expression

Aura is designed as a **continuity desk**, not a dark “personal operating system” dashboard. The application should feel like opening a carefully prepared project folio: clear hierarchy, calm material surfaces, and a single place to understand what matters next.

The redesign deliberately removes the previous black-panel aesthetic, pseudo-futuristic terminology, saturated status treatments, and crowded card grid. Its new character is **editorial, warm, and operationally honest**. Large serif headings establish a sense of perspective, while compact sans-serif labels preserve speed and clarity. The application feels native to a thoughtful creative professional’s desktop rather than a generic startup analytics tool.

## 2. Non-Negotiable Truthfulness Rules

| The interface may say                                                   | The interface must not say                                                                   |
| ----------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `Local record`, `Saved locally`, `Manual only`, `No background capture` | `Encrypted` before the DPAPI proof and encrypted storage gate pass                           |
| `No activity recorded yet`                                              | `Captured note`, `Decision recorded`, or other fabricated sample data                        |
| `No blocker recorded`                                                   | A guessed blocker, inferred project state, or AI-generated summary                           |
| `Manual context is paused`                                              | That context is still collected, queued, or monitored                                        |
| `Future surface` for inactive routes                                    | That Capture, Memory, Activity, or Settings are already usable if the feature does not exist |

## 3. Core Layout

Aura uses a three-column desktop composition at widths above 1180px:

1. **Navigation rail (236px):** wordmark, calm route list, and an explicit privacy status card anchored at the bottom.
2. **Continuity canvas (fluid, 640–840px):** route title, project identity, deterministic continuity brief, and the next deliberate action.
3. **Local record rail (300–360px):** selected-project activity only. When no project is selected or no records exist, the panel explains that nothing has been recorded rather than filling space with sample events.

At narrower widths, the activity rail collapses below the continuity canvas. The navigation retains textual labels; icons never become the only way to understand the product.

## 4. Design Tokens

| Category      | Token             | Value / behaviour                                        |
| ------------- | ----------------- | -------------------------------------------------------- |
| Canvas        | `--canvas`        | `#F7F4ED`, a warm paper-white application field          |
| Surface       | `--surface`       | `#FFFCF6`, cards and editable panels                     |
| Surface muted | `--surface-muted` | `#F0ECE3`, selected navigation and quiet grouping        |
| Ink           | `--ink`           | `#20201D`, primary text and line work                    |
| Ink muted     | `--ink-muted`     | `#706E67`, metadata and explanatory text                 |
| Line          | `--line`          | `#D9D4C9`, 1px structural dividers                       |
| Accent        | `--accent`        | `#2856D9`, primary action, selection, focus ring         |
| Accent soft   | `--accent-soft`   | `#E6ECFF`, selected context surface                      |
| Success       | `--success`       | `#2A7A55`, paired with saved-status copy                 |
| Warning       | `--warning`       | `#906A18`, paired with explicit sensitive/retention copy |
| Danger        | `--danger`        | `#B53B37`, archive/delete only                           |
| Display type  | `--font-display`  | `"Iowan Old Style", "Palatino Linotype", Georgia, serif` |
| UI type       | `--font-ui`       | `Inter, ui-sans-serif, system-ui, sans-serif`            |
| Radius        | `--radius-card`   | `12px`; controls use `8px`                               |
| Spacing       | `--space-*`       | 4px base scale: 4, 8, 12, 16, 20, 24, 32, 40, 48, 64     |
| Motion        | `--motion-fast`   | 160ms ease-out; disabled with reduced-motion preference  |

The design uses no gradients, glow, glassmorphism, dashboard charts, ambient particle effects, or decorative “AI” graphics. Every colour pairing must pass accessible contrast for text and controls. Focus rings use a 3px visible accent outline with a surface offset.

## 5. Screen Architecture

### Today

Today is Aura’s primary resumption surface. The page title is a time-neutral prompt such as `Continue with clarity.` It must not use a false personalized greeting or infer time/context. The page begins with the selected project name and an explicit local privacy pill.

The **Continuity Brief** contains only persisted fields from the selected project: Goal, Current task, Next step, and Blocker. If any value is empty, use exact absence copy: `No goal recorded`, `No current task recorded`, `No next step recorded`, or `No blocker recorded`. The primary action is `Open project`. `Add context` is secondary and remains visibly disabled with a reason when privacy is paused. It should not be presented as a headline product capability until the full manual capture workflow exists.

The Local record rail shows only existing project-scoped activity records. Empty state: `Nothing has been recorded for this project yet.` The user can select the project in Today with a simple project switcher once selected-project persistence exists.

### Projects

Projects is a focused project catalogue rather than a noisy dashboard. The list includes the project name, a truthfully selected or active status, the recorded next step if present, and relative/absolute local update time. The first blank state uses one primary path: `Create project`.

The detail panel opens for the selected project and has labelled inputs for **Project name**, **Goal**, **Current task**, **Next step**, and **Blocker**. All fields use semantic labels. The saving lifecycle is legible: `Save changes` → `Saving locally…` → `Saved locally` or a recoverable error. Archive is visually isolated at the bottom with a confirmation dialog. A project cannot be silently archived by a list interaction.

### Future routes

Capture, Memory, Activity, and Settings should not masquerade as implemented. During UX-001, show them as non-interactive `Planned` destinations or omit them from the main interaction path. The navigation is designed to accommodate them without making a future capability claim.

## 6. Accessibility and Interaction Contract

The navigation uses semantic buttons/links with clear `aria-current` state. Every dialog traps focus, Escape cancels if no destructive action is in progress, and focus returns to the invoking element. Project list keyboard navigation is explicit. Inputs preserve entered content on recoverable save failures. The interface supports 200% browser zoom and keeps the main action visible at a 1024px desktop width.

Visual loading uses semantic text plus skeleton blocks, never a motion-only cue. The layout respects `prefers-reduced-motion`. Status uses an icon/label plus colour. Archive actions require a named-project confirmation and convey that the project is hidden from default views but retained locally.

## 7. Mapping Reference Art to Current V0

The reference uses a large heading, serif hierarchy, a quiet navigation rail, a detailed project card, and a local-record timeline. UX-001 implements the structural direction but uses only the current database fields and local activity rows. Any fictional decisions, note bodies, future task lists, or `Captured note` timeline entries shown in a reference must be replaced with empty-state copy until their corresponding V0 work packages are shipped.

## 8. Google Stitch Status

The connected Google Stitch page authenticated but did not expose interactive controls or a usable viewport to automation. No prompt was submitted and no Stitch artefact was generated. The generated local visual reference is therefore the active design input. If Stitch becomes interactive later, use it to explore this exact Continuity Desk direction; treat all results as visual references only, never as implementation or product-state evidence.

## 9. UX-001 Implementation Boundary

The redesign is implemented in the focused `feat/ux-001-projects-today` branch. It may restructure the renderer into route-focused components, add selected-project persistence and project-scoped read-model data through the existing Rust-owned SQLite boundary, and introduce migration/test coverage only where required. It must not add external dependencies, capabilities, capture payloads, AI, cloud calls, or data collection.
