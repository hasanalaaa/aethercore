# Windows Repair Intelligence Architecture

Phase 19 extends the existing PC Intelligence and system-repair architecture; it does not create a second global intelligence stack.

## Pipeline

`Observe → Normalize → Diagnose → Root/Contributing/Symptom → RepairGraph → Recovery/Safety → Consent → Execute → Verify → Continue/Escalate`

### Observe

`system-repair` owns read-only Windows probes. Each collector fails independently to an `Unknown` fact so a broken provider cannot turn Windows globally unhealthy.

### Normalize and diagnose

`windows-repair-intelligence` owns typed `RepairFact`, `Diagnosis`, confidence, scope, uncertainty, rule version and recovery readiness. Root cause is separate from contributing condition and symptom.

### Plan

`RepairGraph` is a deterministic DAG. Nodes declare action, dependencies, target resource, safety tier, reversibility, recovery requirement, reboot boundary and verification procedure. The graph rejects cycles, missing dependencies, unsafe post-reboot automatic continuation, destructive SAFE_AUTO actions and contradictory destructive recovery.

### Execute

`system-repair` converts only the supported executable graph subset into a sealed `SystemRepairAction`. The renderer receives no executable, shell, PowerShell, registry, service or arbitrary filesystem authority. Execution rechecks assessment ID, machine-state fingerprint and graph digest before consuming one-shot consent.

### Verify and recover

Mutation success is not resolution authority. Each executable repair has post-repair evidence. Verification failure becomes `MutationSucceededVerificationFailed`; interruption distinguishes pre- and post-mutation failure; automatic replay after service restart is disabled.

### Persistence

Schema v13 stores precise execution outcome, fingerprint, graph digest, verification state, reboot requirements, repair timeline events and reboot reassessment tickets. A reboot ticket is only a reminder to perform a fresh assessment; it is never mutation authority.
