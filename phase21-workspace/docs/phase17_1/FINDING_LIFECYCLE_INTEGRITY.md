# Finding Lifecycle Integrity — Phase 17.1

## Truth model
`FindingLifecycle` remains the continuity dimension (`New`, `Active`, `Improved`, `Resolved`, `Recurred`, `Ignored`). Phase 17.1 adds the orthogonal `FindingVerificationStatus`: `ConfirmedCurrent`, `NotRechecked`, `VerificationUnavailable`, `ResolutionConfirmed`.

Absence from one scan is never sufficient to resolve a persisted Finding. Each finding code carries a deterministic `resolution_authority` scope. A missing current Finding transitions to `Resolved` only when every required scope is `Completed`; `CompletedWithWarnings`, `Unavailable`, `PermissionDenied`, `TimedOut`, `Failed`, `Cancelled`, `Pending`, or missing scope state does not authorize automatic resolution.

For conditions whose absence alone is not positive proof (Windows integrity, storage, memory pressure, startup footprint, application update), the owner scope must also supply a same-resource explicitly healthy fact. Driver/device and event-window findings use authoritative absence after the owning collector completes successfully.

## Unverified continuity
If authority is incomplete, the last persisted Finding is carried into the authoritative terminal snapshot with its original `lastObservedUnixMs`, verification status `NotRechecked` or `VerificationUnavailable`, and no remediation candidate. It is not marked resolved and its historical evidence is not rewritten as newly observed.

## Resolution evidence
Confirmed resolution records `resolvedAtUnixMs`, `resolutionScanId`, a localized reason key, and structured `ResolutionEvidence` containing scope, successful collector state, observation time, and any positive healthy-fact references. Resolution is therefore explainable and suitable for future timeline use.

## Cancellation
A cancelled scan is incomplete by default. A scope already completed before cancellation can independently authorize resolution for findings it owns; unrelated scopes that did not complete cannot resolve their findings.
