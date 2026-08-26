//! Phase 31 (W8) — perf ring aggregate + analyze baseline.

use aethercore_performance_telemetry::{PerfSnapshot, PerformanceRing};
use criterion::{Criterion, criterion_group, criterion_main};

fn idle_snapshot(base_ms: u64, index: usize) -> PerfSnapshot {
    let mut snap = PerfSnapshot::default();
    snap.captured_unix_ms = (base_ms + index as u64 * 1000) as i64;
    snap
}

fn bench_ring_aggregate(c: &mut Criterion) {
    c.bench_function("perf_ring_push_aggregate_60", |b| {
        b.iter(|| {
            let ring = PerformanceRing::new();
            let owner = "bench-owner-0123456789abcdef0123456789abcdef";
            for index in 0..60 {
                let _ = ring.push(owner, idle_snapshot(1_700_000_000_000, index));
            }
            ring.aggregate(owner)
        })
    });
}

criterion_group!(benches, bench_ring_aggregate);
criterion_main!(benches);
