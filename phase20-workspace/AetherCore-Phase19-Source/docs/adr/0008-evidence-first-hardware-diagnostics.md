# ADR 0008 — Evidence-first hardware and crash diagnostics

## Status

Accepted for Phase 6.

## Decision

AetherCore will not compute a synthetic hardware-health percentage and will not convert absence of telemetry/events into a positive hardware-health assertion.

Storage metrics are modeled as optional, source-attributed observations. Standard Windows storage reliability counters are combined with direct NVMe SMART/Health where available, but missing counters remain missing. Memory pressure is kept separate from WHEA hardware-error evidence. Kernel-Power Event 41 is treated only as unexpected-shutdown evidence. Lightweight minidump parsing is limited to metadata/header facts and does not claim driver/module root cause without symbol-assisted analysis.

Phase 6 is read-only and runs under the existing maintenance-service IPC boundary without creating a UAC/mutation plan. Completed diagnostic snapshots are locally persisted with bounded retention.

## Consequences

- Some devices will show many `Not reported` metrics. That is preferable to fabricated zeroes or a misleading score.
- A quiet WHEA log can support only “no logged hardware errors were found in the queried window,” not “RAM is healthy.”
- Crash cards may end with an unknown root cause. That is preferable to blaming the last visible driver or the dump filename.
- Correlation between nearby WHEA and crash timestamps is explicitly labeled correlation, not causation.
- Full symbol-backed dump analysis remains a later, optional diagnostic capability rather than being simulated by Phase 6.
