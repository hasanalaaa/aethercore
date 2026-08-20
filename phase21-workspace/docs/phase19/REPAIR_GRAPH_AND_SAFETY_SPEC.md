# RepairGraph & Safety Specification

## Safety tiers

- Level 0 — Diagnostic: read-only.
- Level 1 — Safe Auto: low-risk, narrow, evidence-linked operation.
- Level 2 — Sensitive Repair: explicit consent.
- Level 3 — Reboot or Offline: explicit barrier/resume semantics.
- Level 4 — Recovery Escalation: guided official recovery.
- Level 5 — Destructive Recovery: reset/reinstall class; never silent or SAFE_AUTO.

## Graph invariants

`RepairGraph::new` sorts node identity before hashing and performs deterministic topological ordering. It rejects duplicate IDs, missing dependencies and cycles. It also rejects destructive SAFE_AUTO, contradictory reset/clean-reinstall recovery and automatic continuation across a reboot barrier.

`validate_for_execution(recovery)` adds the runtime recovery prerequisite: an automatically executable node that requires mandatory recovery protection cannot proceed when restore/WinRE/journal protection is unavailable.

## Typical dependency path

`RepairComponentStore → VerifyComponentStore → RepairSystemFiles → VerifySystemFiles → RetryWindowsUpdate`

A required reboot is inserted as a hard barrier. Pre-reboot graph assumptions cannot authorize a post-reboot mutation.
