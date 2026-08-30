# ADR 0016 — Autonomous Idle Scheduler with Zero Mutation Authority

**Status:** Accepted — Phase 14

## Decision

Add `aethercore-idle-scheduler` as a service-side Operation Kernel subsystem. The scheduler owns eligibility, jitter/backoff, preemption monitoring and read-budget admission, but it receives only a closed `AutonomousWorkload` enum containing passive observations.

The scheduler binds each cycle to the active console user's existing Phase 9 principal identity. It uses `ReadBudgetManager` for resource admission and `MutationSupervisor::is_active()` only as a non-disclosing machine-wide exclusion signal.

Publication is guarded by a three-state `CommitFence` shared between the preemption monitor and passive domain scan. This makes user-activity/session-change races linearizable: either revocation happens first and the result cannot publish, or the commit happens first and the already-valid snapshot remains committed.

## State signals

Eligibility combines WTS active-session idle time, session state, AC/battery-saver status, `SHQueryUserNotificationState`, Network List Manager cost, servicing-service contention and bounded ACPI thermal-zone WMI evidence. Unknown presentation/servicing states fail closed; network unknown blocks network-sensitive discovery; unavailable thermal evidence blocks thermally sensitive autonomous work instead of fabricating a normal temperature state.

## Scheduling

- idle threshold: 5 minutes by default;
- active preemption sample: 100 ms;
- slow-signal refresh request: every 2 seconds during a workload, performed single-flight on a separate background worker so the 100 ms activity/session/power path never waits for WMI/network/SCM;
- first and repeat runs include random jitter;
- failures use bounded equal-jitter exponential backoff;
- cadence/backoff is persisted per owner principal and workload in SQLite, so service restart does not reset due times;
- autonomous workloads are single-flight;
- every workload requires an Operation Kernel read lease;
- worker threads enter Windows background CPU/I/O priority mode.

## Durable state

The `autonomous_scheduler_runs` ledger contains scheduling metadata only. It cannot encode a mutation target or authorization. Failure to read or update this ledger disables autonomous scheduling rather than running without cadence memory; the interactive service remains available.

## Consequences

The system gains useful passive maintenance without creating a second privileged command authority. Opaque platform APIs may outlive a cancellation request; Phase 13 watchdog/quarantine semantics therefore remain part of the scheduler contract. Cancellation guarantees rapid scheduler yield and stale-publication prevention, not unsafe forced termination of arbitrary kernel/vendor calls.
