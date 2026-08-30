# Phase 17 Deep Scan Orchestration

## Stages

1. System identity
2. Hardware inventory
3. Driver inventory
4. Windows integrity
5. Storage health
6. Memory pressure
7. Hardware-error history
8. Crash diagnostics
9. Startup footprint
10. Cleanup opportunities
11. Update state
12. Recovery readiness
13. Correlation
14. Recommendation synthesis

Each stage has an explicit weight. `PcScanProgress.percent` is computed from completed stage weight divided by total stage weight; there is no time-based interpolation.

## Execution batches

- Batch A: drivers/hardware, Windows integrity, diagnostics; maximum three expensive read families.
- Batch B: startup and cleanup; maximum two.
- Follow-on: update state and recovery readiness.
- CPU-only: deterministic correlation and recommendation synthesis.

Each worker returns a terminal collector state. Spawn failure is surfaced as an unavailable collector rather than silently disappearing. A failed or unavailable collector can yield a partial scan while successful domains remain useful.

## Cancellation

Cancellation is principal-bound and propagates from Tauri command through IPC to `DeepScanCoordinator` and child tokens. The coordinator stops starting later batches after cancellation. Existing running providers are observed through bounded polling and timeouts. Partial facts already collected are evaluated and persisted honestly.

## Streaming and hydration

The service publishes `DeepScanSnapshot` through EventKind `DeepScan`. A coalescing watcher emits when state/progress/fact/finding/collector signatures change. The regular service hydration path includes current Deep Scan state, allowing renderer reconnect without client polling loops.
