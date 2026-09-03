# P46 Lane B — the Svelte port: target list and progress

Own progress table for `P46-LANE-B-SVELTE.md`. Resume by reading the brief, then
the first row here not marked DONE. Trust the code over the row; if they
disagree, fix the row and say so.

Branch `design/shell-v2` in the design worktree. Port target is the real app at
`phase21-workspace/apps/ui` — Svelte 5 + Vite + Tauri, 92 tracked files, builds
clean at baseline (`vite build`, 196 modules, 737 ms).

## What the shell is, and what it is not

`AetherCore.html` is a fixed-1280 single-page specification with zero `@media`
queries and a demo state machine. The app is a real Tauri client with a
transport, controllers and a bilingual catalog of 1,666 keys. The port moves the
**design system** into the app. It does not move the shell's markup, its demo
data, or its navigation shape.

Where the shell and the app disagree on structure, the app wins; where they
disagree on colour, type, geometry or the four signature elements, the shell
wins.

## Screen map — shell §4 → app page

| Shell screen | App surface | Notes |
|---|---|---|
| 4.1 Overview | `features/overview/OverviewPage.svelte` | first, and in its empty state first of all |
| 4.2 Deep Scan Results | `features/intelligence/DeepScanPage.svelte` + `FindingCard.svelte` | |
| 4.3 Hardware Health | `features/diagnostics/HardwarePage.svelte` | |
| 4.4 Performance Telemetry | `features/performance/PerformancePage.svelte` | |
| 4.5 AI Insights | `features/insights/InsightsPanel.svelte` | panel, not a page, in this app |
| 4.6 Drivers Inventory | `features/drivers/DriversPage.svelte` | carries the policy-hold row |
| 4.7 Deep Clean | `features/cleanup/CleanupPage.svelte` | |
| 4.8 System Repair | `features/repair/RepairPage.svelte` | |
| 4.9 Recovery & Rollback | `features/RecoveryPanel.svelte` + `features/timeline/TimelinePage.svelte` | |
| 4.10 Crash History | `features/diagnostics/CrashPage.svelte` | |
| 4.11 Fleet Management | `features/fleet/FleetPage.svelte` | largest page, 541 lines |
| 4.12 Settings | — | the app has no settings page; theme/locale live in the rail. Not invented here. |
| 4.13 Command Palette | `components/CommandPalette.svelte` | |
| 4.14 Plan Review Consent | `app/PlanDialogs.svelte` | |
| (app-only) | `features/startup/StartupPage.svelte` | no shell screen; styled from the same vocabulary |
| (app-only) | `features/activity/ActivityPage.svelte`, `care/CarePanel.svelte`, `system-care/SystemCarePanel.svelte` | same |

## Work items

Commit and push after every one. Never in batches.

### 1 — target established
- [x] 1.1 shell source extracted, `DESIGN.md` read in full, this list committed

### 2 — tokens, before any component
- [x] 2.1 six colour roles + surfaces + type/space/radius scales into `design-tokens.css`, both themes
- [x] 2.2 IBM Plex Sans Arabic bundled locally and subset; no CDN, no system fallback for Arabic

### 3 — the four signature elements, before the screens
- [x] 3.1 denied-by-policy — violet `#AD4EBC`, 1.5px dashed, 8px radius, `block` icon, names its rule in monospace, never error styling
- [x] 3.2 evidence chip — expands to the raw observation; typed so an uncitable insight cannot be rendered
- [x] 3.3 honest empty state — "Not collected yet" / "لم تُجمع بعد", `—` at rest, never a fake zero
- [x] 3.4 persistent policy band — always visible, lists the rules currently refusing

### 4 — the screens, one commit each
- [ ] 4.1 Overview, empty state first
- [ ] 4.2 Overview, populated
- [ ] 4.3 Deep Scan
- [ ] 4.4 Hardware Health
- [ ] 4.5 Performance Telemetry
- [ ] 4.6 AI Insights
- [ ] 4.7 Drivers Inventory
- [ ] 4.8 Deep Clean
- [ ] 4.9 System Repair
- [ ] 4.10 Recovery & Rollback
- [ ] 4.11 Crash History
- [ ] 4.12 Fleet Management
- [ ] 4.13 Command Palette
- [ ] 4.14 Plan Review Consent
- [ ] 4.15 Startup / Activity / Care (app-only surfaces)
- [ ] 4.16 Navigation rail, header island, screen header

### 5 — responsive
- [ ] 5.1 real `@media` at 1280 / 1024 / 960, both languages, populated

### 6 — verify, with numbers
- [ ] 6.1 screenshots at 1280/1024/960 × en/ar, populated via the fixture harness
- [ ] 6.2 Arabic from the rendered DOM: root RTL, layout genuinely RTL, glyphs from the embedded face
- [ ] 6.3 grep counts for `denied`, `evidence`, and untraceable numbers
- [ ] 6.4 `vite build` and `svelte-check` clean

## Baseline measurements (before any change)

- `vite build`: 196 modules, 623.58 kB JS / 94.56 kB CSS, 737 ms — PASS
- `grep -ric denied src/` = 15
- `grep -ric evidence src/` = 277

## Owner register — do not attempt

- The application icon artwork is unsettled. Not chosen here, not blocked on.
- Shell screen 4.12 Settings has no counterpart page in the app. Not invented.
