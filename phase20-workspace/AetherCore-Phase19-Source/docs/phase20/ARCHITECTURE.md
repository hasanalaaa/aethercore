# AetherCore Phase 20 — Performance Intelligence, Bottleneck Analysis & Safe Optimization

## Status

`PHASE_20_PERFORMANCE_INTELLIGENCE_SOURCE_COMPLETE`

This means the strongest source-level implementation and evidence produced in this delivery. It
does **not** mean Windows-native qualified or GA. Live PDH counter collection, EcoQoS behavior on
real silicon, and Game Mode registry semantics remain qualification debt (see
`QUALIFICATION_DEBT.md`).

## Baseline repair disclosure

The Phase 19 source archive was authored without a Rust toolchain and did not compile. Before any
Phase 20 work, the following baseline defects were repaired and are included in the binary-safe
patch:

| Area | Defects | Class |
|---|---:|---|
| `aethercore-persistence` | 10 | syntax (turbofish), missing `serde` dep usage, u64↔SQLite ToSql/FromSql, missing `mut` |
| `aethercore-operation-kernel` | 4+ | borrow-then-move in event closures, wrong enum variant name (`TelemetryProgress`) |
| `windows-repair-intelligence` | closure E0502 ×2 | mutable-closure borrow colliding with evidence scans |
| `driver-hub`, `system-repair`, `cleaner`, `startup-manager`, `driver-install` | ~10 | missing trait impl (`Display for DiscoveryFailure`), missing `OperationEngine::recoverable_plans`, telemetry accessor mismatches, incomplete struct literals, move-after-borrow |
| `pc-intelligence` | 5+ | tuple arity drift, ownership moves, stale `FaultKind` matching against string field |
| `support-bundle` tests | 6 | missing statement terminators |
| `security` | platform-dependent absolute-path guard rejecting valid Windows paths when audited off-Windows; extended-prefix normalization |
| desktop shell | `tauri.conf.json` unknown window field; missing icon referenced by `generate_context!` |

Baseline after repair: **298 tests passing, workspace `cargo check --all-targets` green.**

## Domain A — Zero-overhead performance telemetry engine

Crate: `crates/performance-telemetry`

* `PerfPlatform` trait with two implementations:
  * `WindowsPerfPlatform` (cfg(windows)): PDH English-counter queries (CPU total/DPC/ISR/context
    switches/queue length, memory standby/modified/page-fault dynamics, physical-disk active time,
    queue depth, transfer latency, GPU engine utilization) plus
    `CallNtPowerInformation(ProcessorInformation)` for power/thermal clamp evidence. All handles
    RAII-wrapped (`QueryHandle`, `CounterHandle`). Every fallible sub-collector degrades into a
    typed `CollectorFault`; one broken counter can never fail the snapshot.
  * `SyntheticPerfPlatform`: deterministic tick-derived samples for audits/tests/offline UI.
* `PerformanceRing`: owner-scoped bounded deque (`MAX_RING_SAMPLES = 300`), background sampler
  thread at clamped interval `[250 ms, 60 s]`, single-short-lock push per tick, deterministic
  `aggregate()` over ordered ring contents.
* `PerfSnapshot::normalized()` clamps every documented bound (256 CPUs, 32 devices, 16 engines,
  16 process entries, basis-point ranges, fault-detail truncation).

## Domain B — Bottleneck diagnosis & causal attribution engine

Crate: `crates/performance-bottleneck`

Eight deterministic rules over the evidence window, each citing measured fact keys with observed
value + threshold + timestamp:

| Rule | Role | Fires when |
|---|---|---|
| `CPU_SATURATION` | RootCause | avg busy ≥ 90 % sustained |
| `DPC_ISR_PRESSURE` | ContributingCondition | DPC+ISR share ≥ 25 % |
| `THERMAL_CLAMP` / `POWER_LIMIT_CLAMP` | RootCause | throttle flags present (reason-classified) |
| `STANDBY_STARVATION` | RootCause | hard faults ≥ 500/s sustained |
| `COMMIT_PRESSURE` | ContributingCondition | commit ≥ 85 % of limit |
| `IO_SATURATION` | RootCause | storage peak ≥ 92 % or avg ≥ 75 % (+latency evidence) |
| `GPU_BOUND_WORKLOAD` | RootCause | 3D engine ≥ 94 % (cites DPC pressure when jitter/lag) |
| `WORKING_SET_BLOAT` | ContributingCondition | modified-list growth ≥ 512 MiB over window |

Guarantees enforced by tests: findings below 5 samples are refused; CONFIRMED requires ≥ 10;
causal edges are materialized only between produced findings and are acyclic by tier ordering;
identical windows produce byte-identical reports including the SHA-256 digest.

## Domain C — Reversible safe optimization & governance engine

Crate: `crates/performance-optimization`

Anti-snake-oil invariants (code-enforced, adversarially tested):

1. No destructive registry cleaning or file deletion anywhere in the action surface.
2. No `EmptyWorkingSet` brute force — memory relief is a cooperative trim request via official
   memory-management APIs, requires explicit consent, and is verified by measured delta.
3. Every candidate carries a fixed-by-kind reversibility (`SessionOnly`, `AutomaticRestore`,
   `ManualReview`); applied changes are journaled as restorable change records with pre-state.
4. Execution is a machine mutation gated through the shared `MutationSupervisor`
   (`MutationWorkload::Optimization` added to the kernel enum) with `CommitFence` publication.
5. The audit-only `NoopPlatform` proves the executor's operation vocabulary contains no
   destructive verbs.

Action kinds: `EcoQos`, `BackgroundPriority`, `CooperativeTrimRequest`, `GameModeProfile`.

## Wire contract (protocol v7, additive)

New `performance.proto`: requests `StartPerfSampling/StopPerfSampling/GetPerformanceSnapshot/
GetBottleneckReport/CreateOptimizationPlan/StartOptimization/GetOptimizationStatus`; events
`EVENT_KIND_PERFORMANCE_SNAPSHOT (22)`, `EVENT_KIND_BOTTLENECK_REPORT (23)`,
`EVENT_KIND_OPTIMIZATION_EXECUTION (24)` with envelope payloads 31–33; response payload tag 44.

Service integration: `PerformanceEngine` composes ring + governor inside `ServiceContext`
(constructed with the kernel's mutation supervisor so optimization execution participates in
machine-wide single-flight). Router handlers follow the established principal-scoped pattern;
sampling starts/stops per owner; analysis is computed on demand from the bounded ring.

Renderer integration: typed contracts, stream-state slices, Tauri commands
(`start_perf_sampling`, `stop_perf_sampling`, `get_performance_snapshot`,
`get_bottleneck_report`, `create_optimization_plan`), and the Performance page.

## Domain D — Apple-grade performance UI

`apps/ui/src/features/performance/PerformancePage.svelte` + controller:

* Metric cards with live sparklines (bounded 60-sample local history, always-LTR technical charts).
* Press feedback on pointerdown via shared `fluidPress`; no fixed-duration CSS animations on any
  interactive surface; all motion flows from the existing spring primitives.
* Translucent material surfaces with light-catching edges per the shared design system.
* Reduced-motion / reduced-transparency / increased-contrast honored through the global
  preference store (no bespoke media queries).
* Full EN/AR catalog parity (38 new message keys each side, verified programmatically);
  Arabic plural rules added for finding counts; technical values bidi-isolated.
* Honest empty states: insufficient evidence and all-clear are distinct, never fabricated.

## Verification

* Workspace `cargo check --all-targets`: green.
* Workspace `cargo test`: **327 passed / 0 failed** (298 baseline + 29 new).
  * New suites: `performance-telemetry/tests/adversarial.rs` (9),
    `performance-bottleneck/tests/attribution.rs` (11),
    `performance-optimization/tests/governance.rs` (9).
* Renderer `svelte-check`: 0 errors (17 pre-existing warnings unchanged).
* Note: running the whole workspace test suite with default job parallelism can surface a
  spurious SQLite `DatabaseBusy` in one driver-install test caused by concurrent test binaries
  sharing the CPU-bound scheduler; `--jobs 2` (or running that crate alone) is clean. This is
  test-harness scheduling, not product behavior, and is tracked in the issue ledger.
