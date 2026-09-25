# P75 lane `plan-journal` — a terminal plan state never lands without its journal

Ledger row: `DBT-P63-012`. Proposed new rows are at the end (text only; the lead assigns ids).

## What was wrong (measured, not read)

No API moved a plan and wrote its execution journal in one transaction. `Database::transition_plan`
was one IMMEDIATE transaction (CAS `UPDATE plans` + `plan_events`), and `upsert_maintenance_execution`,
`upsert_execution` and `add_recovery_record` were separate autocommit writes. `cleaner`,
`system-repair` and `startup-manager` transitioned first and journaled after, so:

- in the window, `status()` returned `Failed` with the previous record (`recovery_required: false`);
- a kill or a DB error in the window made it permanent: the plan was already terminal,
  `recoverable_plans()` skips terminal plans, and the recovery flag and record were never written.

The ledger's cure ("move the upsert above the transition") does not close the second point: two
writes are still two writes. It also misread `driver-install`: `:723/:730 → :741` transitions before
it upserts, and every recovery record there (`:478→:484`, `:707→:713`, `:1192→:1198`) followed the
transition.

Also wider than the row: `status()` in `cleaner`, `system-repair`, `startup-manager` and
`driver-install` read the record and then re-read the plan, returning the second plan state — so
"old record + Failed" was reachable even with correct writers. The four stream watchers in
`maintenance-service/src/streaming.rs` read a status, then decided "terminal" from a fresh engine
read and stopped — so the last status streamed could be the pre-terminal one. And a runtime
failure after mutation in `cleaner`/`system-repair` never wrote a recovery record, so the Recovery
panel stayed empty.

## What changed

- `persistence`: `PlanJournal` (`Maintenance(record, Option<recovery>)` / `Driver(record, Option<recovery>)`)
  and `Database::transition_plan_with_journal`. One IMMEDIATE transaction: journal upsert, optional
  recovery insert, then the existing CAS; it commits only if the CAS moved exactly one row, otherwise
  it rolls back and nothing is written. The statement bodies are factored into `*_in(&Connection)`
  functions the standalone writers reuse, so there is one copy of each SQL statement.
- `operation-engine`: `OperationEngine::transition_with_journal`, sharing `transition`'s checks.
- `cleaner`, `system-repair`, `startup-manager`: every terminal transition (runtime failure,
  completion, restart recovery) goes through it. The already-terminal branch of the fail paths keeps
  the plain upsert (cleaner and system-repair call their fail function twice on one failure; the
  second call must not add a second recovery record).
- Runtime failure after mutation now writes a recovery record in the same transaction, reusing kinds
  and texts the UI already localizes in EN and AR (`CleanupInterrupted`, `SystemRepairInterrupted`;
  `tech.recovery.*`, `tech.cleanup.interruptedAfter`, `tech.repair.stopped`) — no new UI string.
- `driver-install`: the out-of-order paths above plus `finish_verification`'s two terminal paths.
  The two WUA-failure paths keep ignoring a missed write, as before: the plan then stays non-terminal
  and the worker's `fail_safely` records the same error and fails it.
- `status()` in all four crates reads the plan first, then the record.
- Watchers decide terminality from the status they publish (`snapshot_released(&v.plan_state, …)`),
  so the final status streamed is the terminal one.
- Test comments at `cleaner/tests/coordinator.rs` and `system-repair/tests/coordinator.rs` that
  explained the old race are rewritten, and `wait_terminal` now keys on `plan_state` alone — which is
  itself a regression guard: the record must be final the moment the plan is.

## Proof

Committed tests, run against the unchanged product code first (tests added, product untouched):

```
test cleanup_failure_after_deletion_barrier_requires_recovery_review ... FAILED   (no recovery record)
test completed_plan_is_never_visible_before_its_completed_journal ... FAILED
  left: [("Completed", "Verifying", false)]   right: [("Completed", "Completed", true)]
test tests::completed_plan_is_never_visible_before_its_completed_journal ... FAILED   (startup-manager)
  left: [("Completed", "Executing", false)]   right: [("Completed", "Completed", true)]
test failure_after_barrier_requires_recovery_review ... FAILED   (system-repair, no recovery record)
```

After the change: `cargo test -p aethercore-persistence -p aethercore-operation-engine
-p aethercore-cleaner -p aethercore-system-repair -p aethercore-startup-manager
-p aethercore-driver-install --locked` — all pass (persistence 23, cleaner 5, startup-manager 12,
system-repair 4 + 5, driver-install 4 + 6, operation-engine 5).

`persistence` unit tests (it owns `rusqlite`, so the SQL probe lives there and runs on every OS):
`terminal_state_is_committed_with_the_journal_that_explains_it` — an `AFTER UPDATE OF state ON plans`
trigger copies the joined `maintenance_executions` row at the instant of the flip and sees
`("Failed", 1, Some(5))`; `failed_journal_write_leaves_the_plan_recoverable` — a `BEFORE UPDATE`
`RAISE(ABORT)` on the journal leaves the plan `Executing`, no event, no recovery row;
`missed_state_cas_writes_no_journal`.

### The crate-level SQL probes the brief asked for — run, not committed

The brief's design adds `rusqlite.workspace = true` to `[dev-dependencies]` of the crates under test.
Measured: that adds `"rusqlite"` to the crate's entry in `Cargo.lock` (`+1` line), which the lane
contract forbids, so the probes were run with the dev-dependency in place locally and then removed.
They install the same triggers from a second connection on the test database and drive the real
coordinators with the fake platforms:

| probe | unchanged code | this change |
|---|---|---|
| cleaner Failed: journal at the flip | `[("Executing", 0, false)]` — FAILED | ok |
| cleaner Completed: journal at the flip | `[("Verifying", 0, false)]` — FAILED | ok |
| cleaner journal write aborted | "plan Failed is terminal although its journal was never written" — FAILED | ok (plan stays recoverable; `recover_incomplete` then records `recovery_required`) |
| system-repair Failed | `[("Executing", 0, false)]` — FAILED | ok |
| system-repair Completed | `[("Executing", 0, false)]` — FAILED | ok |
| system-repair journal write aborted | same message — FAILED | ok |

The first row is the ledger's symptom exactly: the plan became `Failed` while its record still said
`Executing` with `recovery_required = 0`. If the dependency lane adds the dev-dependency, these six
tests can be committed as they are.

### Gates (macOS, from `phase21-workspace/`)

- `cargo fmt --all -- --check` — clean.
- `cargo clippy --all-targets --locked -- -D warnings` — clean for `persistence`, `operation-engine`,
  `system-repair`, `startup-manager`. For `cleaner`, `driver-install` and `maintenance-service` it
  stops on findings that are all in code this lane did not touch and that fail on `main` too
  (`cleaner` macOS dead code at `:26`, `:1057-1113`; `driver-backup`, `hardware-telemetry`,
  `security`, `performance-telemetry/macos_impl.rs`); without `-D warnings` those three crates report
  no finding in any file or line this lane changed.
- Windows target: every crate here pulls `libsqlite3-sys`, so local `x86_64-pc-windows-msvc` clippy
  cannot build; the Windows CI job on the PR is the compile and test verdict.
- `static_validate.py` 347 checks, 0 failed; `test_gate_readers.py` all 14 fail closed;
  `ps_marker_scan.py` 234 assertions, 0 failed, 3 unmeasured (`freeze-dependencies.ps1` baselines,
  `verify-reproducible.ps1` — unrelated to this lane).
- Audits naming touched files: `phase14`, `enterprise`, `phase15`, `phase17_1`, `phase21`, `phase22`,
  `zenith` pass. `phase17` (`FaultKind::PermissionDenied`, debt fields), `phase19` (`P19-PLAN-002`
  looks for a one-line token rustfmt split in `start_inner`, identical on `main`) and `phase27`
  (wire-tag ceilings) fail identically without this change and check nothing it touches.

## Proposed status cells

- `DBT-P63-012` → **CLOSED** — terminal transitions in `cleaner`, `system-repair`, `startup-manager`
  and `driver-install` commit the plan state, its journal and its recovery record in one transaction
  (`transition_plan_with_journal`); `status()` reads plan before record; SQL probe proof in
  `persistence` tests plus the crate-level probe table above; Windows verdict = the PR's CI run.

## Proposed new rows (text only)

1. `startup-manager` had the same transition-then-journal ordering (`fail_execution`, the Completed
   path) — not in the ledger. Fixed here; proof: the startup-manager test above. Proposed: CLOSED.
2. The stream watchers decided terminality from a second engine read and could stop after streaming
   a pre-terminal status. Fixed here (terminality from the published snapshot). No automated test —
   the watchers are threads over a live `ServiceContext`; proof is by construction. Proposed: CLOSED,
   verification = code.
3. Runtime failure after mutation in `cleaner` / `system-repair` wrote no recovery record. Fixed
   here; proof: the two recovery-record assertions above. Proposed: CLOSED.
4. Crate-level SQL probes need a `rusqlite` dev-dependency in `cleaner` and `system-repair`
   (`Cargo.lock` +1 line each). Proposed: OPEN, owner = dependency lane.
