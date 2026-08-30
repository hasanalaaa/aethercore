# Phase 17 Performance & Resource Evidence

## Source-level controls verified

- Expensive collector batch concurrency is bounded to three read families; Startup/Cleanup is bounded to two.
- The existing `ReadBudgetManager` remains the shared admission-control authority.
- Collector observation loops use a ten-minute ceiling and 100 ms cancellation checks.
- Service-side Deep Scan publication is coalesced at a 120 ms watcher interval and emits only when authoritative scan signatures change.
- Progress is work-derived from weighted stages and terminal task counts; there is no elapsed-time fake progress loop.
- `ScanMetrics` records duration, summed collector duration, peak active tasks, stream-event count, successful DB writes, and an approximate normalized-payload size.
- History retention is bounded and raw native payload dumps are not persisted by the intelligence layer.

## Qualification boundary

These controls are source-supported and included in the executed Phase 17 static audit. Real CPU/disk/WMI/IOCTL pressure, event throughput, cancellation latency, memory footprint and wall-clock scan duration on representative Windows hardware are `NOT_EXECUTED` here and remain in `QUALIFICATION_DEBT.json`. No production performance number is fabricated.
