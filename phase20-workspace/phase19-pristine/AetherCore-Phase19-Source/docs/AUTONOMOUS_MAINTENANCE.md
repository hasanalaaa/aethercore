# AetherCore Autonomous Maintenance Contract

Phase 14 adds a LocalSystem-owned idle scheduler to the Operation Kernel, but it does **not** add autonomous authority to mutate Windows. The scheduler may observe only five closed workloads: hardware telemetry, Windows driver discovery, cleanup inventory, startup inventory, and Event Log/crash triage. There is no generic command/task entry point and no mutation workload in `AutonomousWorkload`.

## User and session ownership

Every eligibility sample starts from the active console session. The service obtains the interactive user's WTS token, derives the existing Phase 9 principal binding from that token, and publishes snapshots/events only to that principal's event stream. A console-session or binding change revokes the current publication fence and cancels the work.

## Eligibility

A workload is admitted only after all applicable checks succeed:

- active console-session owner is present and unlocked;
- idle duration is at least the configured threshold;
- AC power is available and battery saver is off;
- user notification state is not busy, full-screen, presentation, locked/absent, or unknown;
- Windows servicing is not active or unknown;
- no machine mutation lease is active;
- the Operation Kernel `ReadBudgetManager` admits the workload;
- driver discovery requires an explicitly unmetered network;
- thermally sensitive workloads require ACPI thermal-zone evidence below elevated pressure.

`Unknown` is never silently converted into a healthy/normal state. Where an unknown signal affects a workload's safety policy, the workload is blocked.

## Idle and presentation detection

The LocalSystem service does not use session-0 `GetLastInputInfo`. Idle duration comes from the active WTS session's `WTSINFOEX` timestamps. Presentation/full-screen state is queried while impersonating the active user, followed immediately by `RevertToSelf`. A failed revert is a security-fatal scheduler-probe condition: the scheduler thread exits rather than continuing service work under an interactive token.

## Thermal, power, network and servicing signals

Power comes from `GetSystemPowerStatus`. Network cost is read through Network List Manager and driver discovery is disabled on metered or unknown cost. Servicing contention is conservatively inferred from Windows servicing/update service state. ACPI thermal data uses `MSAcpi_ThermalZoneTemperature` only when both current temperature and a critical trip point are available; finite WMI slices, a wall deadline and a zone-count cap are enforced. No arbitrary absolute temperature is invented when the platform does not expose trip-point evidence.

## Preemption and publication

Active work is sampled on a 100 ms preemption cadence. Slow thermal/network/servicing signals are refreshed separately. Preemption first revokes a shared `CommitFence`, then signals cooperative cancellation.

The fence is a three-state linearization boundary:

- `Active`: publication may still occur;
- `Committed`: a result already linearized and cannot later be relabeled stale;
- `Revoked`: a late worker may finish a platform call but cannot publish.

This is important because some WMI, Windows Update, Event Log, or vendor-driver calls cannot be forcibly interrupted safely. Phase 13 `IsolationGate` containment remains in force: a timed-out worker stays quarantined and a replacement is not spawned until it exits.

## Durable cadence and restart discipline

Scheduler cadence is persisted in SQLite per `(owner_principal_key, workload)`. The ledger stores only scheduling metadata: failure count, next eligible time, last outcome and last completed time. It stores no paths, command lines, consent tokens, authorization grants or mutation intents. A service restart therefore does not forget minimum intervals or failure backoff and cannot create a background-provider stampede. If the cadence ledger cannot be read or written, autonomous scheduling fails closed while interactive maintenance remains available.

## Resource discipline

The scheduler is single-flight even though normal read-only requests can overlap within `ReadBudgetManager`. Each autonomous worker enters Windows `THREAD_MODE_BACKGROUND_BEGIN`, which lowers CPU and I/O scheduling priority for that worker, and uses the scheduler's cooperative CPU/I/O governor between provider phases. First-run and repeat execution include randomized jitter. Failures use equal-jitter exponential backoff with a bounded maximum.

This is deliberately not described as a hard byte-per-second cap on opaque kernel/vendor APIs: AetherCore cannot safely preempt an arbitrary synchronous IOCTL midway through execution. The enforceable guarantees are admission budgets, single-flight execution, low scheduling priority, cooperative checkpoints, watchdog isolation, and no autonomous mutation authority.

## Zero background mutations

The scheduler has no code path to:

- install a driver;
- run DISM/SFC/repair mutations;
- delete files;
- disable/restore startup items;
- approve/consume UAC consent;
- execute application updates.

Any active machine mutation blocks scheduler admission and preempts an autonomous workload that is still uncommitted.
