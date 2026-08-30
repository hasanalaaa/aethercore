# Phase 17 PC Intelligence Architecture

## Scope

Phase 17 introduces `aethercore-pc-intelligence` as a read-only interpretation layer over existing authoritative AetherCore collectors. It does not replace Driver Hub, Diagnostic Engine, System Repair, Startup Manager, Cleaner, Update Engine, Operation Kernel, or their Windows-native providers.

The implemented data flow is:

`collect -> normalize -> correlate -> findings -> remediation candidates -> optional sealed plan`

No Deep Scan path executes a remediation action.

## Boundaries

- **Collectors remain authoritative.** The new backend invokes existing principal-bound read-only domain scans under `ReadBudgetManager` leases.
- **Normalization is typed.** Raw subsystem snapshots become `SystemFact` values with explicit source, privacy-preserving resource identity, freshness, confidence, typed payload variants, and bounded evidence.
- **Hardware inventory is represented as facts.** PnP devices are normalized as `HardwareDevice` facts with class/manufacturer/description/installed-driver/GPU-vendor fields where available; private device identifiers are hashed before entering the intelligence model.
- **Driver-change correlation has a production source.** Recent `verified=1` driver-install journal rows are read from SQLite through an owner-principal-scoped query and normalized into `DriverChange` facts. This prevents a synthetic-only correlation path and blocks cross-principal history leakage.
- **Provider faults are limitations.** Diagnostic provider faults normalize to `DiagnosticLimitation` facts with states such as permission denied or timed out; these never become machine-health findings by themselves.
- **Rules are deterministic.** `rules.rs` contains a reviewable versioned inventory and no ML/opaque health score.
- **Healthy checks are explicit.** The dashboard's healthy count is derived only from fact variants that affirm a healthy condition; unrelated unreferenced facts do not inflate it.
- **Severity and confidence are independent.** A critical condition can be medium-confidence; confidence never derives from severity.
- **Remediation is proposal-only.** `RemediationCandidate` describes authority, privilege, safety, reversibility, reboot expectation and verification intent. Non-selectable driver offers do not become executable install actions; vendor-managed offers remain manual. `RemediationPlan::seal` rejects duplicate, unknown, or stale selected actions, canonically sorts the accepted actions, and hashes the immutable consent snapshot.
- **Persistence is continuity-oriented.** Scan summaries/snapshots, finding lifecycle, overrides, and sealed plans are stored in SQLite WAL schema v10. Raw native payload dumps are not persisted by the intelligence layer. Persistence failure changes an otherwise completed scan to `Partial` and exposes a generic continuity limitation while preserving current in-memory findings.
- **IPC is streamed.** Deep Scan state is transported through persistent IPC v7 events. Service-side snapshot watching coalesces intermediate state, and hydration republishes current authoritative scan state after reconnect.

## Scan metrics

`ScanMetrics` records scan duration, summed collector duration, peak active tasks, emitted Deep Scan event count, successful persistence writes, and an approximate normalized-payload byte count. These are operational measurements, not health scores. The persisted final snapshot contains the event count known at completion; reconnect/hydration events occurring after completion update the live authoritative snapshot but do not rewrite historical rows solely to increment telemetry.

## Resource governance

The first expensive batch runs Driver Hub, System Repair assessment, and Diagnostics concurrently. Existing global read-budget capacity is four, so the coordinator deliberately consumes at most three expensive read families at once and leaves capacity for foreground work. Startup and Cleanup form a second bounded batch. Update and recovery-readiness reads are bounded follow-on tasks.

Collector wait loops have a ten-minute ceiling and cancellation checks every 100 ms. This is a source-level bound; native provider behavior still requires the Windows qualification scenarios listed in `QUALIFICATION_DEBT.json`.

## Privacy

PnP instance IDs and storage device IDs are converted to SHA-256-derived internal stable IDs before they enter the intelligence model. Driver-install history queries are principal-scoped. Usernames, emails, absolute user paths, account identifiers and raw hardware serials are not required by the new fact/finding contracts.

## UI

`DeepScanPage.svelte` is integrated into production navigation and the Overview CTA. It uses existing `Pressable`, `ProgressBar`, `MaterialSurface`, `TechnicalText` and design motion primitives rather than introducing a second interaction system. Progress is derived from actual stage weights and task terminal states; there is no time-driven 0-100 animation.

Findings can appear progressively while the scan is active because the coordinator evaluates the current fact set on each authoritative interim publication. The renderer displays a restrained progressive subset and announces only finding-count changes to assistive technology rather than every progress event.

The simple surface shows categorical system status, current stage, task counts, mutually exclusive result groups, and diagnostic limitations. `CompletedWithWarnings` is rendered as a limitation state rather than as a failure. Evidence and rule/version details are expandable. EN/AR keys are paired; raw internal warnings are not used as primary localized limitation copy.
