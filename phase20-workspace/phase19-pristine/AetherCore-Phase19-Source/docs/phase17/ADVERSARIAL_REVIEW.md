# Phase 17 Adversarial Review

## Pass A — false positives

- Healthy storage/memory/startup fixture produces no finding.
- The healthy-check counter now accepts only explicit healthy fact variants; unrelated inventory/history facts do not inflate the “Healthy” result.
- Diagnostic unavailability and provider faults are limitations, not machine-health failures.
- Cleanup and memory thresholds have below/at-boundary tests.
- Stale and future-dated crash evidence is rejected.
- An invalid draft correlation between AetherCore self-update failure and Windows component-store corruption was removed because those are different update domains.
- Non-selectable/non-vendor driver offers are informational and produce no executable `InstallDriver` candidate; vendor-managed offers remain manual.
- Sensitive/hardware remediation is not counted as an optional optimization, and result groups are mutually exclusive.

## Pass B — false negatives

Synthetic cases explicitly cover missing-driver, storage-reliability, Windows-integrity, startup, driver-regression and crash/hardware rules. Findings carry rule IDs and evidence references.

The driver-regression rule is wired to production evidence rather than fixtures alone: recent verified driver-install journal entries are fetched through a principal-scoped persistence query and normalized into `DriverChange` facts before correlation.

## Pass C — resource failure

Collector threads return an explicit unavailable result if worker creation fails. Existing read budgets cap expensive read concurrency. Native collector polling is cancellation-aware and bounded by a ten-minute deadline. IPC watcher state is coalesced. Renderer reconnect hydrates from authoritative service state.

Finding/history persistence errors are not silently swallowed: an otherwise completed scan becomes `Partial`, retains current findings, and exposes a generic continuity limitation. Native WMI/RPC/WUA/Event Log stall injection and renderer-disconnect stress remain Windows qualification debt P17-QD-007/008.

## Pass D — evidence corruption

Core payloads are closed Rust enums rather than arbitrary JSON. Evidence technical values are bounded. Crash-time rules reject stale/future evidence. Correlations require temporal ordering and bounded windows. Driver-install history must be verified and owned by the requesting principal. `RemediationPlan::seal` rejects duplicate selected action IDs and unknown/stale IDs before hashing the immutable action set. Severity is not promoted to confidence. No opaque AI/health score is present.

Malformed native provider payload behavior still requires the final Windows qualification scenarios.
