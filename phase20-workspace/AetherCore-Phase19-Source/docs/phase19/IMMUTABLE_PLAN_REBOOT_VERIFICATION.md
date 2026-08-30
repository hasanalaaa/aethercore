# Immutable Plan, Reboot Resume & Verification

## Sealing

The operation plan freezes assessment ID, trusted repair action IDs, canonical machine-state fingerprint, RepairGraph JSON/digest, maximum safety tier and reboot-boundary count. The graph digest is deterministic.

Before mutation, the service obtains the current assessment and rejects execution when assessment ID, fingerprint or graph digest changed. The current graph is revalidated with recovery prerequisites before consent is consumed.

## Reboot

A graph containing a reboot barrier is not executed as an ordinary pre-reboot repair plan. Persistence stores a hashed reboot reassessment ticket whose policy is `FreshAssessmentRequiredAfterReboot`. A later fresh assessment may consume the ticket as reassessed, but the ticket never authorizes an old mutation.

## Verification authority

- component store: DISM API health recheck;
- protected system files: fresh SFC verify path;
- service repair: query service state plus WUA discovery;
- filesystem scan: rerun the bounded scan when selected.

The coordinator emits `SucceededVerified` only when all action-specific proof succeeds. A completed mutation with failed proof becomes `MutationSucceededVerificationFailed` and remains unresolved.
