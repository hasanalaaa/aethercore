# AetherCore Enterprise Convergence Deliverables

## Refactored source

- `crates/windows-foundation` — shared RAII ownership for common Win32 resource pairs and explicit ownership transfer into `std::fs::File`.
- `crates/update-engine` — lock-independent observer publication; per-upload staging locks/reservations; owner-scoped content-addressed service paths; recovery-safe stale cleanup.
- `crates/support-bundle` — linearized preparation reservations and fail-closed cleanup outside quota locks.
- `crates/operation-kernel` — bounded EventBus health telemetry with strict owner-scoped export support.
- `crates/ipc` — count+byte bounded client/server writer pumps, typed outbound backpressure, finite bootstrap enqueue, atomic non-blocking close, at-most-once disconnect, and negotiated client inflight admission.
- `crates/security`, `idle-scheduler`, `hardware-telemetry`, `startup-manager`, `restore-point`, `windows-update` — adoption of the new Windows foundation where lifecycle semantics are common and lexical.
- `apps/ui` — discriminated IPC event union, exhaustive reducer, removal of interactive `any`, cached live motion/accessibility preferences.

## Verification

- `scripts/enterprise-adversarial-audit.py`
- `scripts/enterprise-adversarial-audit.ps1`
- `scripts/enterprise-stress-matrix.ps1`
- `scripts/enterprise-soak-analyze.py` — full-series slope + quartile resource-growth analysis.
- `scripts/verify-enterprise.ps1`
- `release/enterprise-stress-matrix.json`
- inherited Phase 0–16 static/security/reliability/scheduler/GA gates
- Windows Cargo fmt/clippy/test + UI check/build
- 17 targeted regressions covering bundle reservations, update observer/staging races, EventBus lifecycle/telemetry, IPC backpressure bytes, disconnect and inflight admission
- Phase 16 resilience/soak and signed release lifecycle integration
- GA seal requires Enterprise release stress + sustained resource-trend evidence

## Architecture records

- `docs/ENTERPRISE_ADVERSARIAL_AUDIT.md`
- `docs/adr/0019-enterprise-convergence-refactor.md`
- `ENTERPRISE_TRANSFORMATION_MATRIX.md`

## Qualification semantics

`verify-enterprise.ps1` is the master post-Phase-16 qualification gate. It does not create a GA seal. `verify-production.ps1` remains the sole GA-seal authority and requires the Enterprise source audit before accepting Phase 16 evidence and signing the production seal.

Long-running Windows execution remains required for native FFI, signed installer, WebView2/accessibility and leak-trend claims.
