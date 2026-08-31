# Design elevation handoff

Completed on `codex/design-elevation`:

- Replaced the P36 compile-only icon artwork with a reviewed evidence-shield master and regenerated PNG, ICO (16–256px), and ICNS assets from that single source.
- Registered Windows/macOS icon containers in `apps/desktop/tauri.conf.json`.
- Replaced the navigation rail’s placeholder circle with the product mark.
- Added `apps/ui/DESIGN_LANGUAGE.md` covering evidence visibility, governed mutation, honest telemetry, five required states, RTL, themes, motion, and the review gate.
- Added an intentional light theme alongside the existing dark tokens.

Shell elevation is now implemented on `design/shell`: the rail uses grouped information architecture (Understand / Change safely / History), the shell exposes local-first context plus idle/loading/error/policy-guarded states, and the command palette keeps unavailable online surfaces visible with an honest status. Light and dark themes are explicit, persisted, keyboard reachable, and RTL-safe. Next ordered work: elevate `DeepScanPage`, then citation-first insights, followed by drivers/care/recovery. Do not touch Rust, IPC, installer, services, crates, or the security model.

Verification note: icon containers and dimensions were checked locally. After reinstalling the frontend dependencies, `pnpm check` reports 0 Svelte errors and the inherited 17 warnings; `pnpm build` succeeds. The existing browser-only Tauri error remains when previewing outside the desktop host and is unrelated to shell rendering.
