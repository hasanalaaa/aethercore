# P75 lane `care-consent` (Wave 2) — evidence (ledger `DBT-P75-038`…`045`)

Branch `lane/care-consent`, from `main` `174a9e4` (green, CI `36231082584`). No dependency or
lockfile change.

Scope used: `crates/care-orchestrator/**`, service `care.rs`, `router/care.rs`, the care dispatch in
`composition.rs`, the care-step writer in `persistence/src/lib.rs`, and one file outside the
`AMBITION.md` §3 list: `crates/operation-kernel/src/mutation.rs` (`MutationLease::delegate`,
needed to make care's lease machine-wide; no other Wave 2 lane touches it). One gate changed in its
own commit: `scripts/phase22-adversarial-audit.py` (below).

## Rows

| row | what was wrong | now | red-before (on `174a9e4`) |
|---|---|---|---|
| `DBT-P75-038` | `run_care_plan` executed every step, including `ReviewOnly` ones (driver install, system repair), and `CarePlan::build` sorted by `Reverse(level)`, so review work ran **first** — the comment said the opposite | only `Auto` steps reach a domain; review steps are reported `Skipped` and are not journaled as executing; auto work sorts first | `review_only_steps_are_never_executed`: `left: 2, right: 1`; `auto_work_is_ordered_before_review_work`: `left: "drv-plan-1", right: "clean-plan-1"` |
| `DBT-P75-039` | session consent was one boolean per principal, not bound to any plan: consent given for one plan ran a different one | a grant records the digest of the plan composed at that moment (the grant's response shows that plan); `run_care_plan` takes that digest and refuses with `ConsentRequired` / `DigestChanged` before taking the lease; the service then shows the current plan as `AwaitingConsent` | `consent_covers_only_the_plan_it_was_given_for`: a plan added after consent ran, `left: 2, right: 0` |
| `DBT-P75-040` | `CareCoordinator` built its own `MutationSupervisor::new()`, so its `OneClickCare` lease excluded only other care runs; any domain could start between or under care steps | care takes the lease on the kernel's machine-wide supervisor; each domain step runs under `MutationLease::delegate(workload, plan)`, which shares the care lease's hold, so the machine stays held until the care lease and every step lease drop | code (`care.rs`: `supervisor: MutationSupervisor::new()`); the old constructor had no way to reach the machine-wide supervisor, so `care_holds_the_machine_wide_lease` is written against the new one. Kernel: `a_delegate_holds_the_machine_until_every_holder_drops` |
| `DBT-P75-041` | `RealDomainDispatch` chose the owning domain by calling each `status()`, which returns `Ok(None)` until the plan has an execution record (`cleaner/src/lib.rs` `status`: `get_maintenance_execution(..)? else { return Ok(None) }`) — so every unstarted plan, i.e. every care step, failed with "no domain coordinator owns this plan" | routed by the kind care read from the plan; the dead SystemRepair arm is removed (review-only kinds never reach dispatch) | code, as quoted. `an_unstarted_cleanup_plan_reaches_the_cleaner` (new API) shows the cleaner's own answer, `operation engine: authorization required` |
| `DBT-P75-042` | `cancel()` revoked the coordinator's single fence forever: after one `CancelCareRun` every later run stopped before step 1 until a restart, and was reported `Completed` | each run gets its own fence, registered while it runs; a cancel reaches only the run in progress, and a second concurrent run is refused; a cancelled run reports `Cancelled` | `a_cancel_does_not_stop_the_next_run`: `state: "Completed"`, step `Skipped`, `left: 0, right: 1` |
| `DBT-P75-043` | the dispatch poll loops checked the deadline only when `status()` returned `Ok(Some)`: a status that erred or vanished held the care run (and the machine) forever | one `await_terminal` helper polls until terminal or the deadline, whatever each read returns | code; `a_status_that_never_answers_cannot_hold_the_run_forever` |
| `DBT-P75-044` | `replace_care_steps` ran `DELETE FROM care_steps` — every run's history — whenever a run journaled a new step | `insert_care_step` writes one `(run_id, step_index)` row | `journaling_one_run_keeps_every_other_runs_steps`: `left: 0, right: 1` |
| `DBT-P75-045` (**OPEN**, owner) | measured while tracing `DBT-P75-041`: every domain `start_with_lease` consumes that plan's own broker-approved consent intent (`consume_consent_and_transition`, 120 s expiry). Care's session consent does not create one, so a care step succeeds only if the owner approved that exact plan through the broker in the last 120 s | unchanged — this is the per-plan consent invariant and weakening it is not a lane's call | the dispatch test above: the cleaner refuses with `authorization required` |

`DBT-P75-045` needs the owner: either care keeps per-plan broker approval (and the UI must say that
One-Click Care asks once per plan), or care's session consent is defined to authorize `Auto` plans,
which changes the consent model.

## Gate change (own commit)

`p22-consent-required-guard` looked for the token `consent_granted` in `engine.rs`. The parameter
is now the approved digest, so the token vanished while the guard got stronger. The check now
matches the two refusals themselves (`None => return Err(CareError::ConsentRequired)` and
`digest != plan.plan_digest_sha256 => return Err(CareError::DigestChanged)`). Planted regression
(the `None` arm made `{}`): the audit reports the failure; restored: `"failures": []`.

## Local proof (macOS, at the lane head)

In the PR body with result lines: `cargo fmt --all -- --check`, workspace clippy `-D warnings`,
`cargo test --workspace --locked`, `static_validate.py`, `test_gate_readers.py`,
`ps_marker_scan.py`, `enterprise-adversarial-audit.py`, and the audits naming touched files
(`phase14`, `phase15`, `phase17_1`, `phase21`, `phase22`, `phase26`, `phase27`, `phase28`,
`zenith-recursive`). `phase27` and `phase28` fail identically on `origin/main` (`p27-wirefreeze:*`,
`p28-mutation-guard:offline.rs`, `p28-deps:allowlist-only`) — not introduced here.

Not run: Windows-target clippy (the service graph pulls `libsqlite3-sys`); the Windows CI job is the
compile verdict.
