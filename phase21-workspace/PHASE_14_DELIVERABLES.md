# Phase 14 Deliverables — Autonomous Maintenance Intelligence & Idle Scheduler

## Status

**Source architecture complete and authoring-runtime qualified.** Native Windows qualification remains intentionally delegated to `scripts/verify-phase14.ps1` because the authoring environment does not contain the Windows Rust/MSVC/WiX/PowerShell runtime required to execute the real platform gates.

## Delivered architecture

### 1. `aethercore-idle-scheduler`

A new Operation-Kernel-side scheduler crate owns eligibility, cadence, jitter/backoff, preemption monitoring and passive-work admission. Its public workload domain is a closed enum containing exactly:

- `HardwareTelemetry`
- `DriverDiscovery`
- `CleanupInventory`
- `StartupInventory`
- `EventLogTriage`

There is no arbitrary task/command entry point and no representation for driver installation, cleanup deletion, repair mutation, startup mutation, consent consumption or application update.

### 2. Principal/session ownership

The Windows state probe obtains the active console session and its user token through WTS, then derives the existing Phase 9 `PrincipalContext` binding from that token. A scheduler cycle is bound to the resulting principal key and Windows session ID. User/session changes revoke the current publication fence and cancel the cycle.

No synthetic LocalSystem owner is used for user-visible passive snapshots.

### 3. Eligibility engine

Admission evaluates:

- active interactive owner and unlocked session;
- minimum idle duration;
- AC power and battery saver;
- presentation/full-screen/busy state;
- machine-wide mutation activity;
- Operation Kernel read-budget availability;
- network cost for driver discovery;
- workload-sensitive thermal pressure;
- Windows servicing/update contention.

Unknown presentation and servicing state fail closed. Unknown network cost blocks network-sensitive discovery. Unknown thermal evidence blocks the heavier thermally sensitive inventories without fabricating a normal temperature state.

### 4. Fast preemption and slow-signal isolation

The active preemption loop samples the fast session/activity/power path every **100 ms**. Network, servicing and thermal probes refresh separately every approximately **2 seconds** through a single-flight background worker, so a slow WMI/COM/SCM signal cannot hold up activity/session preemption.

Preemption order is deliberate:

1. revoke the `CommitFence`;
2. signal cooperative cancellation;
3. let bounded/watchdog platform isolation handle any opaque call that cannot stop immediately.

### 5. Linearized passive publication

`CommitFence` was added to `aethercore-collector-runtime` and threaded through passive Driver Hub, Cleanup, Startup and Diagnostic Engine refresh APIs. The fence has `Active`, `Committed` and `Revoked` states under one synchronization boundary.

A late worker after preemption may finish an opaque platform call, but it cannot publish a stale result. A result that committed before preemption remains a valid result from the earlier eligibility point and is not retroactively relabeled stale.

### 6. Passive domain APIs

The five autonomous workloads call dedicated passive entry points only:

- Diagnostic Engine hardware-only refresh;
- Diagnostic Engine Event Log/crash-only refresh;
- Driver Hub passive discovery;
- Cleaner passive inventory;
- Startup Manager passive inventory.

Hardware and Event Log passive refreshes merge with the current same-principal diagnostic snapshot and remain `Partial` until both evidence sides are available. Passive diagnostic refresh does not create high-frequency durable diagnostic-history records.

### 7. Cleanup privacy restriction

Autonomous cleanup inventory is narrower than interactive cleanup discovery. It excludes profile-specific roots and does not enumerate WER report archives/queues, minidumps or `MEMORY.DMP`. Interactive user-reviewed cleanup retains its existing broader allowlisted behavior.

### 8. Operation Kernel integration

Every passive workload:

- refuses admission while `MutationSupervisor` reports an active machine mutation;
- acquires the appropriate `ReadBudgetManager` lease;
- is single-flight at scheduler level;
- is additionally contained by a per-workload Phase 13 `IsolationGate`.

This preserves normal interactive read concurrency without allowing autonomous work to contend with a machine mutation.

### 9. Resource discipline

Autonomous workers enter Windows `THREAD_MODE_BACKGROUND_BEGIN` for lower CPU/I/O scheduling priority. `ResourceGovernor` adds cancellable cooperative CPU/I/O admission windows and the service reserves a bounded amount before each provider phase.

This is not represented as an absolute byte-per-second or CPU-percentage guarantee over opaque synchronous Windows/vendor APIs. The enforceable policy is background scheduling priority, cooperative checkpoints, single-flight admission, mutation exclusion, watchdog isolation and immediate stale-publication revocation.

### 10. Jitter, backoff and durable cadence

First and repeat runs include randomized jitter. Failures use bounded equal-jitter exponential backoff.

Migration `0008_phase14_scheduler.sql` adds `autonomous_scheduler_runs`, keyed by `(owner_principal_key, workload)`. It stores scheduling metadata only: failure count, next eligible time, last outcome, last-completed time and update time. It stores no command/path/consent/authorization/mutation data.

If the cadence ledger cannot be read or written, autonomous scheduling disables itself; interactive maintenance remains available. Service restart therefore cannot silently reset due times/backoff for the same principal binding.

### 11. Persistent event-stream telemetry

Phase 14 adds typed `SchedulerEvent` Protobuf contracts and `EVENT_KIND_SCHEDULER`. Scheduler run state plus successful passive domain snapshots are published through the existing principal-scoped IPC v7 event stream.

The Activity surface displays the latest autonomous-maintenance state through the Phase 12 EN/AR typed catalogs. Raw internal scheduler/provider details are not displayed as ordinary user prose.

### 12. Verification gates

Delivered scripts:

- `scripts/phase14-scheduler-audit.py`
- `scripts/phase14-scheduler-audit.ps1`
- `scripts/phase14-scheduler-tests.ps1`
- `scripts/phase14-scheduler-fault-injection.ps1`
- `scripts/verify-phase14.ps1`

`verify-phase14.ps1` inherits Phase 0–13 first, then runs Phase 14 architecture checks, locked Windows workspace compilation, deterministic package tests, scheduler/preemption fault-injection and the aggregate static gate. Reproducibility/release packaging is deliberately deferred until all Phase 14 gates pass.

An opt-in `-LiveReadOnlySchedulerProbe` executes only the ignored Windows state-observation test; it starts no maintenance workload and exposes no mutation path.

## Source qualification at seal point

- Phase 0–14 aggregate static gate: **272/272 PASS**
- Phase 14 scheduler architecture audit: **63/63 PASS**
- inherited Phase 13 reliability audit: **61/61 PASS**
- Phase 12 localization parity: **952 EN / 952 AR**, **10 plural families**, runtime plural regression PASS
- TypeScript strict authoring audit: **32/32 PASS**
- Svelte structural/embedded-TS audit: **24/24 PASS**
- CSS PostCSS parse: **9/9 PASS**
- Rust lexical/delimiter sweep: **70/70 PASS**
- TOML/JSON/Python/YAML/WiX parsing: **32 / 7 / 5 / 4 / 2 PASS**
- SQLite fresh migration chain: **PASS**
- Phase 13→14 SQLite upgrade: **PASS**
- product `TODO/FIXME/HACK`: **0**
- renderer polling timers: **0**
- scheduler mutation entry points: **0**
- scheduler arbitrary process/command launch paths: **0**

## Qualification boundary

The authoring environment has no Cargo/Rust Windows target, PowerShell, Windows WTS/SCM/WMI/EventLog runtime, WiX or Authenticode. Therefore this delivery does not claim that Windows-native compile/live/installer/signing gates ran here.

The authoritative native gate is:

```powershell
.\scripts\verify-phase14.ps1
```

and public promotion still requires the documented physical/runtime/installer/security matrices.
