# ADR-0005: Protect and journal immediately before WUA driver mutation

**Status:** Accepted

## Context

Phase 3 introduces the first real privileged system mutation. Driver installation must remain compatible with Windows Update applicability while providing recovery evidence across app exit, service restart, and reboot. Creating restore points too early increases the protected-change window; creating them after download but before `BeginInstall` keeps the transaction focused. Replaying an interrupted WUA installation is unsafe because the service cannot prove whether Windows committed part or all of the previous mutation.

## Decision

- WUA revalidates and downloads the exact service-planned Update ID/revision collection before the mutation barrier.
- Immediately before `BeginInstall`, the service must:
  1. create and independently verify a fresh System Restore point;
  2. export all applicable currently bound OEM driver packages;
  3. persist protection evidence;
  4. commit `mutation_started=true` and `Protected -> Executing`.
- Only after all four steps may the WUA layer call `BeginInstall`.
- `orcSucceededWithErrors` is not clean success in this workflow.
- A WUA `RebootRequired` state never triggers an AetherCore restart; after a detected reboot, only verification resumes.
- An interrupted `Executing`/`Verifying` operation is never replayed automatically. It becomes recovery evidence.

## Consequences

The workflow can block installation on machines where System Restore is disabled or cannot produce a fresh point. This is intentional. AetherCore prioritizes recoverability and explicit evidence over maximizing the number of machines on which one-click installation proceeds.

The service stores more journal data and performs extra PnP/WUA checks, but the UI can truthfully distinguish downloaded, protected, mutated, verified, reboot-pending, and recovery-required states.
