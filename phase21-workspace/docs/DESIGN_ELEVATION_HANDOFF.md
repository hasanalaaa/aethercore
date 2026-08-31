# Design elevation handoff

Completed on `codex/design-elevation`:

- Replaced the P36 compile-only icon artwork with a reviewed evidence-shield master and regenerated PNG, ICO (16–256px), and ICNS assets from that single source.
- Registered Windows/macOS icon containers in `apps/desktop/tauri.conf.json`.
- Replaced the navigation rail’s placeholder circle with the product mark.
- Added `apps/ui/DESIGN_LANGUAGE.md` covering evidence visibility, governed mutation, honest telemetry, five required states, RTL, themes, motion, and the review gate.
- Added an intentional light theme alongside the existing dark tokens.

Next ordered work: elevate `AppShell`, `NavigationRail`, and `CommandPalette`, then `DeepScanPage`, then citation-first insights, followed by drivers/care/recovery. Do not touch Rust, IPC, installer, services, crates, or the security model.

Verification note: icon containers and dimensions were checked locally. `pnpm check`/`vite build` could not run because the checked-in pnpm tree has a broken `@jridgewell/remapping` symlink under Svelte; reinstall dependencies before the frontend gate.
