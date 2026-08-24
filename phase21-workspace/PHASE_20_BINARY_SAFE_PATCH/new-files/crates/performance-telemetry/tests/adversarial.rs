//! Phase 20 adversarial + determinism test harness (Domain A).
//!
//! These tests attack the sampler's invariants: bounded memory, owner isolation, clamping of
//! hostile counter values, deterministic aggregation, and lifecycle races.

use std::sync::Arc;
use std::time::Duration;

use aethercore_performance_telemetry::{
    CollectorFault, CpuSample, GpuEngineSample, GpuSample, MemorySample, PerfPlatform, PerfSnapshot,
    PerformanceRing, StorageQueueSample, SyntheticPerfPlatform, ThermalThrottleReason,
    MAX_GPU_ENGINES, MAX_PROCESS_TOP, MAX_RING_SAMPLES, MAX_STORAGE_DEVICES,
};

const OWNER_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OWNER_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn snapshot_with_cpu(busy_bp: u32) -> PerfSnapshot {
    PerfSnapshot {
        captured_unix_ms: 1_700_000_000_000,
        interval_ms: 1_000,
        cpu: CpuSample {
            per_processor_busy_bp: vec![busy_bp],
            total_busy_bp: busy_bp,
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn hostile_counter_values_are_clamped_into_contract_ranges() {
    let mut hostile = PerfSnapshot {
        captured_unix_ms: 1,
        interval_ms: 999,
        cpu: CpuSample {
            per_processor_busy_bp: vec![u32::MAX; 512],
            total_busy_bp: u32::MAX,
            dpc_isr_busy_bp: u32::MAX,
            context_switches_per_sec: u64::MAX,
            processor_queue_length_x100: u64::MAX,
        },
        memory: MemorySample { memory_load_percent: u32::MAX, ..Default::default() },
        storage: vec![StorageQueueSample {
            device_id: "d".into(),
            friendly_name: "d".into(),
            active_time_bp: u32::MAX,
            ..Default::default()
        }; MAX_STORAGE_DEVICES + 8],
        gpu: GpuSample {
            engines: vec![GpuEngineSample { engine_name: "3D".into(), utilization_bp: u32::MAX }; MAX_GPU_ENGINES + 8],
            ..Default::default()
        },
        process_top: vec![Default::default(); MAX_PROCESS_TOP + 16],
        collector_faults: vec![CollectorFault {
            collector: "x".repeat(4096),
            kind: "y".into(),
            detail: "z".repeat(8192),
        }; 64],
        power: Default::default(),
    };
    // Pre-fix normalization is idempotent and applied on push; simulate that path.
    hostile = hostile.normalized();
    assert!(hostile.cpu.total_busy_bp <= 10_000);
    assert_eq!(hostile.cpu.per_processor_busy_bp.len(), 256);
    assert!(hostile.memory.memory_load_percent <= 100);
    assert_eq!(hostile.storage.len(), MAX_STORAGE_DEVICES);
    assert_eq!(hostile.gpu.engines.len(), MAX_GPU_ENGINES);
    assert_eq!(hostile.process_top.len(), MAX_PROCESS_TOP);
    assert!(hostile.collector_faults.len() <= 16);
    assert!(hostile.collector_faults[0].detail.chars().count() <= 256);
}

#[test]
fn ring_is_bounded_and_drops_oldest() {
    let ring = PerformanceRing::new();
    for index in 0..(MAX_RING_SAMPLES + 25) {
        let mut snap = snapshot_with_cpu(1_000 + index as u32);
        snap.captured_unix_ms = 1_700_000_000_000 + index as i64;
        ring.push(OWNER_A, snap).unwrap();
    }
    assert_eq!(ring.len(OWNER_A), MAX_RING_SAMPLES);
    let newest = ring.latest(OWNER_A).unwrap();
    assert_eq!(newest.captured_unix_ms, 1_700_000_000_000 + MAX_RING_SAMPLES as i64 + 24);
}

#[test]
fn foreign_owner_reads_nothing() {
    let ring = PerformanceRing::new();
    ring.push(OWNER_A, snapshot_with_cpu(5_000)).unwrap();
    assert!(ring.is_empty(OWNER_B));
    assert!(ring.latest(OWNER_B).is_none());
    assert!(ring.window(OWNER_B).is_empty());
    assert_eq!(ring.aggregate(OWNER_B).sample_count, 0);
    // Pushing under a different principal before install is rejected.
    assert!(ring.push(OWNER_B, snapshot_with_cpu(9_900)).is_err());
}

#[test]
fn aggregate_is_deterministic_for_identical_windows() {
    let ring_a = PerformanceRing::new();
    let ring_b = PerformanceRing::new();
    for index in 0..20 {
        let mut snap = snapshot_with_cpu(7_000 + index as u32);
        snap.memory.hard_faults_per_sec = 100 * index as u64;
        ring_a.push(OWNER_A, snap.clone()).unwrap();
        ring_b.push(OWNER_A, snap).unwrap();
    }
    assert_eq!(ring_a.aggregate(OWNER_A), ring_b.aggregate(OWNER_A));
}

#[test]
fn aggregate_over_empty_ring_reports_insufficient_evidence() {
    let ring = PerformanceRing::new();
    let agg = ring.aggregate(OWNER_A);
    assert_eq!(agg.sample_count, 0);
    assert_eq!(agg.cpu_busy_bp_avg, 0);
}

#[test]
fn synthetic_platform_is_deterministic_per_tick() {
    let platform = SyntheticPerfPlatform::new();
    platform.with_cpu_busy_bp(4_200);
    let a = platform.sample(Duration::from_millis(1_000));
    let b = platform.sample(Duration::from_millis(1_000));
    // Tick counter advances, so raw samples differ only in tick-derived fields.
    assert_eq!(a.cpu.total_busy_bp, b.cpu.total_busy_bp);
    assert_eq!(a.memory.hard_faults_per_sec, b.memory.hard_faults_per_sec);
    // dpc_isr_busy_bp = (tick % 11) * 10 — advance by exactly one tick modulo 110.
    let delta = (b.cpu.dpc_isr_busy_bp + 110 - a.cpu.dpc_isr_busy_bp) % 110;
    assert_eq!(delta, 10);
}

#[test]
fn start_stop_lifecycle_rejects_double_start_and_allows_restart() {
    let ring = PerformanceRing::new();
    let platform: Arc<dyn PerfPlatform> = Arc::new(SyntheticPerfPlatform::new());
    ring.start(platform.clone(), OWNER_A, 250).unwrap();
    assert!(ring.is_active());
    // Second start while active must fail.
    assert!(ring.start(platform.clone(), OWNER_A, 250).is_err());
    ring.stop();
    // Worker exits asynchronously; restart must be accepted immediately after stop flag clears.
    ring.start(platform.clone(), OWNER_A, 250).unwrap();
    ring.stop();
}

#[test]
fn interval_clamping_enforces_observer_effect_floor() {
    assert_eq!(PerfSnapshot::clamped_interval_ms(1), 250);
    assert_eq!(PerfSnapshot::clamped_interval_ms(500), 500);
    assert_eq!(PerfSnapshot::clamped_interval_ms(u32::MAX), 60_000);
}

#[test]
fn background_sampler_publishes_into_the_ring() {
    let ring = Arc::new(PerformanceRing::new());
    let platform: Arc<dyn PerfPlatform> = Arc::new({
        let p = SyntheticPerfPlatform::new();
        p.with_cpu_busy_bp(3_300);
        p
    });
    ring.start(platform, OWNER_A, 250).unwrap();
    // Wait until at least two ticks have landed (bounded wait ~2s).
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while ring.len(OWNER_A) < 2 && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(25));
    }
    ring.stop();
    assert!(ring.len(OWNER_A) >= 2, "background sampler did not publish");
    let agg = ring.aggregate(OWNER_A);
    assert!(agg.sample_count >= 2);
    assert_eq!(agg.cpu_busy_bp_avg, 3_300);
}
