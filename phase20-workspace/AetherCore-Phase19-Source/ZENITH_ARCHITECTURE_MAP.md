# AetherCore Zenith Architecture Map

## Repository topology

The current workspace contains **34 Rust workspace members**, **82 Rust source files**, and a Svelte/Tauri renderer with **25 Svelte components, 33 TypeScript modules, and 9 CSS files** at the time of the Zenith pass.

### Process / authority boundary

- `apps/desktop` — non-elevated Tauri/WebView2 desktop host.
- `services/maintenance-service` — privileged machine service and typed mutation authority.
- `apps/consent-broker` — constrained UAC/consent handoff.
- `apps/update-broker` — restricted elevated update execution path.
- `apps/install-hardener` — installation hardening boundary.

The renderer never becomes a machine-mutation authority. Service-derived identity, typed intent, immutable plans, one-shot consent, and `MutationSupervisor` remain the governing mutation model.

### Core domain / infrastructure crates

- Contracts and transport: `contracts`, `ipc`.
- Windows ownership/security: `windows-foundation`, `security`.
- Execution/recovery: `operation-engine`, `operation-kernel`, `persistence`.
- Driver stack: `windows-pnp`, `windows-update`, `driver-hub`, `driver-backup`, `driver-install`, `gpu-policy`.
- Repair/cleanup/startup: `restore-point`, `system-repair`, `cleaner`, `startup-manager`.
- Diagnostics: `collector-runtime`, `diagnostics`, `hardware-telemetry`, `crash-diagnostics`, `diagnostic-engine`.
- Autonomous read-only governance: `idle-scheduler`.
- Secure update/support: `update-engine`, `update-download`, `support-bundle`.
- Release verification tooling: `update-manifest`, `support-bundle-verify`, `ga-probe`.

### Renderer architecture

- `apps/ui/src/app` owns shell/page orchestration.
- `apps/ui/src/platform` owns IPC/session/stream integration.
- `apps/ui/src/design/motion` owns springs, projection/rubber-band math, preference state, press and drag actions.
- `apps/ui/src/design/primitives` owns reusable semantic interaction/material primitives.
- `apps/ui/src/features/*` owns feature presentation/controllers without privileged business authority.
- `apps/ui/src/lib/i18n` owns typed English/Arabic localization and bidi-safe technical segmentation.

### Release / qualification graph

`Phase 0–16 gates` → `verify-enterprise.ps1` → `zenith-adversarial-audit.ps1` → optional Phase-16 packaging/signing boundary → evidence-backed `verify-production.ps1` → GA seal.

CI retains the existing Enterprise gate and adds Zenith source regression. Signed release executes the Zenith source gate before the existing Enterprise signing/packaging step. This is intentionally additive rather than a historical gate rewrite.
