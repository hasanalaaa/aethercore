# Phase 5 Deliverables — Startup & Background Services Manager

## Delivered

### Native inventory

- Registry Run/RunOnce enumeration for machine and loaded user hives, with 32/64-bit views.
- Common/per-user Startup folder discovery with reparse rejection and exact file evidence.
- Boot/logon scheduled-task discovery through Task Scheduler COM.
- Conservative automatic third-party/unknown service discovery with protected-role policy.

### Passive-default domain model

- `RecommendationDecision::{Unreviewed, KeepEnabled, Disable}`.
- Every scan initializes items as Unreviewed in the UI.
- Unreviewed and KeepEnabled never become `PlanAction`s.
- Empty/inaction plan construction fails with `PassiveDefault` and performs no mutation.
- No Optimize-All path.

### Protection policy

- Windows/Microsoft/security components protected.
- Network/VPN, storage/RAID/NVMe, input/accessibility roles protected even when third-party.
- Launch-protected, shared/driver, dependency-sensitive and ambiguous services protected.
- Microsoft Windows scheduled-task namespaces protected.

### Reversible mutation layer

- Registry startup values: exact type/raw-byte backup, typed value deletion, exact restore.
- Startup-folder files: move to service-owned backup rather than delete, then exact restore.
- Scheduled tasks: Enabled flag only, definition drift detection, exact restore.
- Services: Automatic -> Demand/Manual for future starts; no service stop and no SERVICE_DISABLED; exact original start/delayed-auto restore.
- Service disable and restore require second confirmation in addition to UAC.

### Durable change/recovery journal

- SQLite migration `0004_startup_manager.sql`.
- Independent change IDs and origin chains.
- Exact original/applied state JSON recorded before mutation.
- Applied, AppliedRecovered, Restored, NoChange and RecoveryRequired reconciliation states.
- Restart never replays startup mutations automatically.

### IPC / Tauri / UI

- Protobuf protocol v4 with scan/snapshot/plan/start/status/history/restore endpoints.
- Maintenance-service handlers and startup recovery integration.
- Tauri commands with no arbitrary native target/path mutation endpoint.
- Startup dashboard with filters/evidence/protection state, per-item explicit decisions, immutable review, UAC, progress, history and Restore Original.

### Tests and verification

- Passive-default zero-mutation test.
- KeepEnabled zero-action test.
- Protected-target enforcement test.
- Service second-confirmation tests for disable and restore.
- Durable SQLite startup-change record test.
- Disable -> Restore coordinator chain test with independent auditable change records.
- Policy tests for essential/security/network/storage/input/accessibility protection.
- Task-definition hash normalization test.
- Read-only ignored Windows inventory probe.
- `scripts/verify-phase5.ps1` Windows gate.
- Static repository validator extended through Phase 5.

## Deliberately not claimed

- Phase 5 does not stop running services.
- Phase 5 does not set services to SERVICE_DISABLED.
- Phase 5 does not delete startup files, task definitions, registry keys, or services.
- Phase 5 does not infer a High/Medium/Low boot-impact score without direct attributable evidence.
- Phase 5 does not automate a real disable/restore from the verification script.
- Native Windows compilation/runtime execution is authoritative only when the repository passes the included Windows gate on the supported baseline.

## Final source gate

The dependency-light repository validator reports:

```text
ok: true
checks: 36
failed: []
```

Additional authoring-runtime checks independently parse TOML/JSON, verify Rust delimiter structure across 40 Rust files, parse/transpile the Svelte TypeScript script block, compile Python verification helpers, and search the Phase 5 privileged source for forbidden service-stop/delete/shell mutation markers.

Native Rust/Windows SDK compilation and real Windows startup/service mutations remain intentionally delegated to `scripts/verify-phase5.ps1`, Windows CI, and controlled Windows staging hardware/VMs.
