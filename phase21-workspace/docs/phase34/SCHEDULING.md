# Phase 34 — Scheduling

## Semantics

`FleetSchedule` (schema `aethercore.fleet.schedule.v1`) selects hosts by id
or `group:<tag>`, pins a compliance profile (`cis-l1`/`cis-l2`), and carries
a cadence:

- `EveryHours(n)` — minimum enforced period is `MIN_CADENCE_SECS = 3600`
  (structural no-tight-loop bound; `EveryHours(0)` is a typed error).
- `DailyAtUtcHour(0..=23)` — fixed daily window.

## Determinism

All decisions are pure functions of schedule state + an injected
`Clock`. `FixedClock` enables deterministic proofs; `SystemClock` is used in
production paths. `is_due(schedule, now)` and `begin_run(schedule, now)` are
the only decision points.

## Missed runs

After downtime, `begin_run` anchors the next run at `now + period` exactly
once — catch-up happens, catch-up storms do not.

## Overlap

`OverlapLock::try_acquire` grants at most one in-flight run per schedule
(single-run policy). A second acquire while running is a typed
`OverlapLocked` refusal. Other schedules are unaffected.

## Failure behavior

Every finished run appends to `fleet_run_history` (append-only). Failures
update the `last_result` summary metadata; they never erase prior history
rows.

## Cancellation

A run honors the shared `CancelToken`; in-flight hosts surface typed
`Cancelled` results, and per-host isolation is inherited from the
orchestrator.

## Execution — the scheduler tick (corrective closure)

Semantics alone are not execution. `crates/fleet/src/scheduler_runner.rs`
implements the deterministic executable pipeline:

```
FleetSchedule → due evaluation (is_due) → scope resolution
   → selected FleetHosts → bounded Fleet orchestrator (run_batch)
   → REMOTE read-only compliance operation → per-host typed results
   → append run history → update last_result → advance next_run
```

The store is injected (`SchedulerStore` trait): the CLI persists to the
fleet state directory (`schedules.json` + append-only `run_history.json`),
and hermetic proofs drive the COMPLETE pipeline against an in-memory store
and a fake transport — no network.

- CLI equivalent: `aetherctl fleet schedule run-due` (`FleetJob::ScheduleRunDue`).
- Only due + enabled schedules run; overlap lock enforced; missed runs
  catch up exactly once; bounded fleet concurrency (default 4, ceiling 16);
  cancellation propagated; host and schedule failures isolated;
  deterministic schedule/host ordering; read-only compliance only.
- Proofs: `due_schedule_invokes_fleet_compliance_transport_end_to_end`,
  `not_due_schedule_is_not_run`, `disabled_schedule_is_never_run`,
  `failure_is_isolated_and_history_preserved`,
  `cancellation_propagates_as_typed_outcomes`,
  `missed_run_catches_up_exactly_once`,
  `results_are_deterministic_and_bounded`,
  `scope_resolution_is_tag_and_id_aware_deduped`.

## Platform honesty

On macOS this phase proves the FULL pipeline — semantics AND execution —
deterministically via the fixed-clock runner proofs and the
`fleet schedule run-due` tick path. A continuously running background
scheduler daemon on Windows is **Windows-native qualification debt**
(QD-034-006) and is explicitly not claimed here.
