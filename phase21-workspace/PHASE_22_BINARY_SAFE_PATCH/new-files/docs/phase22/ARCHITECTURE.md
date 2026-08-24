# Phase 22 — One-Click Care Orchestration

Status: source-complete on the development host; Windows-native qualification remains
open (see QUALIFICATION_DEBT.json). Contract: PCD-ONE-CLICK-ORCHESTRATION —
"Future orchestration may run only SAFE_AUTO plus explicitly approved dependent
actions under shared mutation/recovery policy."

## Part A — blocking defect repair (pre-scope)

`crates/driver-install/tests/coordinator.rs::temp_root()` derived its scratch path
from pid + wall-clock nanos only; parallel test threads sharing a coarse clock could
collide on `state.db`. Fix: a function-local `AtomicU32` sequence counter now anchors
uniqueness (`{pid}-{seq}-{nanos}`, std only, no new dependencies), applied through the
shared helper so every call site benefits. Evidence: 10 consecutive isolated green
runs; two identical full-suite runs at 343/0. Logged as closed issue P21-ISS-004.

## 1. Domain crate — crates/care-orchestrator

### Plan composition (src/model.rs)

`CarePlan::build` consumes candidate steps referencing EXISTING domain plan ids. It
never creates mutation semantics: a step is `(domain_plan_id, domain_kind, safety,
title_key)` — no action content. Composition collapses duplicates, rejects empty ids,
caps at `MAX_CARE_STEPS = 16`, and sorts by a total order (safety level descending,
then kind, then plan id) before computing the deterministic `plan_digest_sha256`.
Identical input yields an identical digest regardless of caller ordering.

### Safety firewall

`CareSafety::Auto` (level 0) covers domains whose own contract classifies their plans
SAFE_AUTO — cleanup (exact allowlisted evidence) and startup disables (reversible).
Everything else, concretely DriverInstall and SystemRepair today, is
`CareSafety::ReviewOnly` (level ≥ 2) and flows to review guidance only: the
orchestrator refuses to execute it. Level 1 stays reserved for future explicitly
approved dependent actions.

### Execution engine (src/engine.rs)

`run_care_plan` enforces, in order:

1. **Consent first.** Without explicit `consent_granted` (the service layer's
   per-session registry — absence of refusal is not consent, product invariant 6)
   the run returns `ConsentRequired` before touching anything.
2. **Single-flight.** The machine-wide lease is acquired under
   `MutationWorkload::OneClickCare` (new kernel variant). Any other active mutation —
   including a second care run — yields `LeaseBusy`.
3. **Journaled resume.** Every durable transition (run started → consent recorded →
   step state → step result → run finished) goes through the `CareJournal` trait,
   bound to the `care_runs`/`care_steps` tables. A crashed run leaves its exact
   position on disk.
4. **CommitFence cancellation.** A revoked fence stops the run at the next step
   boundary; unexecuted steps are cited as `Skipped`, never hidden.
5. **Truth-first report.** Each finished step cites the DOMAIN's own verification
   outcome (`VerifiedByDomain` / `CompletedUnverified` / `Failed` / `Skipped`) plus
   the domain's verification string verbatim. Hostile domain behavior (failure,
   timeout, poisoned journal) surfaces as typed errors with journaled evidence —
   the run never continues past an opaque failure and never fabricates aggregate
   "cleaned/optimized" claims.

## 2. Wire contract — additive only

New `care.proto`; tags consumed:

| Surface | Tag | Name |
|---|---|---|
| EventKind | 26 | `EVENT_KIND_CARE_RUN` |
| EventEnvelope payload | 35 | `care_status` |
| Request payloads | 80–83 | start / grant-consent / get-status / cancel |
| Response payload | 47 | `care_status` |
| MutationWorkloadKind | 6 | `MUTATION_WORKLOAD_KIND_ONE_CLICK_CARE` |

Frozen ranges through Phase 21 are asserted unchanged by the extended wire-freeze gate.

## 3. Service integration

`services/maintenance-service/src/care.rs` binds the orchestrator to persistence
(`PersistenceJournal`) and composes plans from the owner's own `plans_in_states`
rows classified by domain kind. Session consent lives in an in-memory registry by
design — consent that outlives the session would not be one-time. Router handlers
(GetCareStatus / GrantCareSessionConsent / StartCareRun / CancelCareRun) are
principal-scoped exactly like every other domain; status republishes over
`EVENT_KIND_CARE_RUN` and normalizes in the desktop host like any stream event.

Honest scope note: the Phase 22 `ServiceExecutor` returns a typed `DomainRejected`
for live dispatch rather than a fabricated success — wiring each domain's
headless `start_with_lease` entry point into the orchestrator thread is logged as
QD-022-002. The full run path (single-flight, journal, fence, report) is exercised
by tests against the trait boundary.

## 4. Renderer

Typed contracts (`CareRunStatus`, `CareStepReport`), a `careStatus` stream slice, and
a controller (`features/care/controller.ts`). `CarePanel` mounts on the Dashboard
(Overview) route: Start opens the explicit consent dialog; authorize-and-start happens
in one gesture; a running run exposes Cancel (fence-based); the final view lists every
step with its safety class and cited domain outcome, plus a summary key that claims
only what the evidence supports. All interactions follow the Apple contract:
fluidPress pointerdown feedback, spring-driven motion via shared primitives,
reduced-motion/transparency honored through preference tokens, TechnicalText bidi
isolation for digests/kinds.

## 5. i18n

28 `care.*` keys EN+AR; parity enforced programmatically (Gate P22-7); Arabic plural
family `unit.careStep` with all six forms.

## 6. Verification summary

Entry gates reproduced unchanged (668/43/159). Part A fix proven (10× isolated green;
2× full suite 343/0). Workspace total after Phase 22: **363 passed / 0 failed**
(+10 care-orchestrator adversarial). Extended audit: **242 checks PASS**. svelte-check:
0 errors / 17 warnings (unchanged). Binary-safe patch round-trip ×2 byte-identical;
master archive deterministic ×2 (`PHASE22_FINAL_SHA256.txt`).
