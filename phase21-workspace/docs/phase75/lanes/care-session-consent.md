# P75 lane `care-session-consent` — DBT-P75-045, owner decision "hardened session consent"

The owner chose neither ledger option. One owner approval of the care plan authorizes exactly
its Auto steps, for exactly one run, bound to the owner principal, single-use; each step runs
only while its domain plan's content digest equals the digest the approved care plan committed
to; ReviewOnly never runs; no approval outlives its run; the broker's per-plan 120 s path is
unchanged for everything outside care.

| requirement | commit | how |
|---|---|---|
| care digest commits to content | `c654b3a` | `CareStep.domain_plan_digest` hashed into the care digest |
| one approval, one run, one owner | `b677f3b` | `SessionConsentRegistry` (digest, granted_at) per principal; `start_run` takes it; void after 120 s |
| step runs only at the approved content | `f07f2a2` | `authorize_care_step` inserts an approved consent intent only while plan owner, state and digest match; the domain's unchanged `consume_consent_and_transition` consumes it; `DigestChanged` refuses that step, others still run; unconsumed authorizations are withdrawn |
| ReviewOnly never runs | `f07f2a2` | refused before any authorization; reported Skipped |
| clients start only the plan shown; copy says what one click approves | `902a479`, `fda2d3d`, `c07b1e0` | aetherctl and the UI compare the granted digest with the shown one; dialog lists the automatic steps |

Red-before on `7ee0673` (`cargo test -p aethercore-maintenance-service --bins -- p75_care`):
- `one_approval_runs_every_auto_step`: `DomainRejected { Cleanup, "operation engine: authorization required" }`
- `a_consumed_approval_cannot_run_twice`: `left: "Completed", right: "AwaitingConsent"`
- `the_care_digest_commits_to_domain_plan_content`: equal digests for different content
- `review_only_work_never_executes_under_care_consent`: authorization required (the review skip itself held since DBT-P75-038)
- `another_owners_approval_is_rejected`: passed before (the map was already keyed by principal) — kept as a guard; the persistence test also covers another owner at the barrier.
- step refusal and barrier binding: `a_step_whose_plan_changed_after_approval_is_refused`, `a_care_step_authorization_is_bound_to_owner_content_and_state` (new API).

Also in this lane (trial run): `DBT-P75-061` the approval dialog opened with no plan loaded and the run from Overview showed nothing (`c07b1e0`); `DBT-P75-062` an empty plan asked for approval (`be0c75b`).
