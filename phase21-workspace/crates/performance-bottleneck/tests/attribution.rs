//! Phase 20 Domain B tests — deterministic attribution, evidence integrity, adversarial inputs.

use std::time::Duration;

use aethercore_performance_bottleneck::{Confidence, Role, analyze, thresholds};
use aethercore_performance_telemetry::{
    GpuEngineSample, PerfPlatform, PerfSnapshot, StorageQueueSample, SyntheticPerfPlatform,
    ThermalThrottleReason,
};

fn build_window(
    samples: Vec<PerfSnapshot>,
) -> (
    aethercore_performance_bottleneck::Report,
    aethercore_performance_telemetry::WindowAggregate,
) {
    let ring = aethercore_performance_telemetry::PerformanceRing::new();
    let owner = "oooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooo";
    for snap in samples {
        ring.push(owner, snap).unwrap();
    }
    let agg = ring.aggregate(owner);
    let window = ring.window(owner);
    (analyze(&agg, &window, 1_700_000_100_000), agg)
}

fn saturated_cpu_snapshot(at: i64) -> PerfSnapshot {
    let platform = SyntheticPerfPlatform::new();
    platform.with_cpu_busy_bp(9_700);
    let mut snap = platform.sample(Duration::from_millis(1_000));
    snap.captured_unix_ms = at;
    snap
}

fn idle_snapshot(at: i64) -> PerfSnapshot {
    let platform = SyntheticPerfPlatform::new();
    platform.with_cpu_busy_bp(300);
    let mut snap = platform.sample(Duration::from_millis(1_000));
    snap.captured_unix_ms = at;
    snap
}

#[test]
fn insufficient_samples_yield_an_honest_empty_report() {
    let samples: Vec<PerfSnapshot> = (0..3)
        .map(|index| saturated_cpu_snapshot(1_700_000_000_000 + index))
        .collect();
    let (report, _) = build_window(samples);
    assert_eq!(report.findings.len(), 0);
    assert_eq!(report.analyzed_sample_count, 3);
    assert!(!report.digest_sha256.is_empty());
}

#[test]
fn sustained_cpu_saturation_is_confirmed_root_cause() {
    let samples: Vec<PerfSnapshot> = (0..12)
        .map(|index| saturated_cpu_snapshot(1_700_000_000_000 + index * 1000))
        .collect();
    let (report, aggregate) = build_window(samples);
    assert!(aggregate.cpu_busy_bp_avg >= thresholds::CPU_SATURATION_AVG_BP);
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.code == "CPU_SATURATION")
        .expect("cpu saturation must be detected");
    assert_eq!(finding.role, Role::RootCause);
    assert_eq!(finding.confidence, Confidence::Confirmed);
    assert!(
        finding
            .evidence
            .iter()
            .all(|ev| ev.observed_value >= ev.threshold)
    );
    assert!(!finding.applicable_action_kinds.is_empty());
}

#[test]
fn healthy_window_produces_no_findings() {
    let samples: Vec<PerfSnapshot> = (0..15)
        .map(|index| idle_snapshot(1_700_000_000_000 + index * 1000))
        .collect();
    let (report, _) = build_window(samples);
    assert_eq!(report.findings.len(), 0);
}

#[test]
fn thermal_clamp_distinguishes_power_limit_from_thermal() {
    let mut samples: Vec<PerfSnapshot> = (0..10)
        .map(|index| idle_snapshot(1_700_000_000_000 + index * 1000))
        .collect();
    for snap in &mut samples {
        snap.power.throttle_active = true;
        snap.power.throttle_reason = ThermalThrottleReason::Power;
    }
    let (report, _) = build_window(samples);
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.code == "POWER_LIMIT_CLAMP")
        .expect("power clamp must be classified separately from thermal");
    assert_eq!(finding.role, Role::RootCause);
    assert_eq!(finding.confidence, Confidence::Confirmed);

    // Same evidence with thermal reason classifies as THERMAL_CLAMP instead.
    let mut thermal_samples: Vec<PerfSnapshot> = (0..10)
        .map(|index| idle_snapshot(1_700_000_000_000 + index * 1000))
        .collect();
    for snap in &mut thermal_samples {
        snap.power.throttle_active = true;
        snap.power.throttle_reason = ThermalThrottleReason::Thermal;
    }
    let (thermal_report, _) = build_window(thermal_samples);
    assert!(
        thermal_report
            .findings
            .iter()
            .any(|finding| finding.code == "THERMAL_CLAMP")
    );
    assert!(
        !thermal_report
            .findings
            .iter()
            .any(|finding| finding.code == "POWER_LIMIT_CLAMP")
    );
}

#[test]
fn standby_starvation_requires_sustained_hard_faults() {
    let mut samples: Vec<PerfSnapshot> = (0..12)
        .map(|index| idle_snapshot(1_700_000_000_000 + index * 1000))
        .collect();
    for snap in &mut samples {
        snap.memory.hard_faults_per_sec = thresholds::HARD_FAULT_STARVATION_PER_SEC + 250;
    }
    let (report, _) = build_window(samples);
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.code == "STANDBY_STARVATION")
        .expect("starvation must be detected");
    // Starvation is the observed root cause of memory-driven sluggishness; no action is auto-
    // proposed because the correct fix is workload-side, never a destructive purge.
    assert!(finding.applicable_action_kinds.is_empty());
}

#[test]
fn io_saturation_with_slow_transfers_reaches_confirmed() {
    let mut samples: Vec<PerfSnapshot> = (0..10)
        .map(|index| idle_snapshot(1_700_000_000_000 + index * 1000))
        .collect();
    for snap in &mut samples {
        snap.storage.push(StorageQueueSample {
            device_id: "disk0".into(),
            friendly_name: "System".into(),
            active_time_bp: 9_800,
            avg_transfer_latency_us: 40_000,
            ..Default::default()
        });
    }
    let (report, _) = build_window(samples);
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.code == "IO_SATURATION")
        .expect("io saturation must be detected");
    assert_eq!(finding.confidence, Confidence::Confirmed);
    assert_eq!(
        finding.evidence.len(),
        2,
        "latency evidence must be cited alongside active time"
    );
}

#[test]
fn io_peak_evidence_cites_the_snapshot_that_measured_it() {
    let start = 1_700_000_000_000;
    let mut samples: Vec<PerfSnapshot> = (0..5)
        .map(|index| idle_snapshot(start + index * 1000))
        .collect();
    for (index, snap) in samples.iter_mut().enumerate() {
        snap.storage.push(StorageQueueSample {
            device_id: "disk0".into(),
            active_time_bp: if index == 0 { 9_800 } else { 1_000 },
            ..Default::default()
        });
    }
    let (report, _) = build_window(samples);
    let finding = report
        .findings
        .iter()
        .find(|f| f.code == "IO_SATURATION")
        .unwrap();
    let peak = finding
        .evidence
        .iter()
        .find(|e| e.fact_key == "storage.activeBp.peak")
        .unwrap();
    assert_eq!(peak.observed_value, 9_800.0);
    assert_eq!(peak.observed_unix_ms, start);
}

#[test]
fn gpu_bound_links_to_dpc_pressure_when_jitter_is_high() {
    let mut samples: Vec<PerfSnapshot> = (0..8)
        .map(|index| idle_snapshot(1_700_000_000_000 + index * 1000))
        .collect();
    for snap in &mut samples {
        snap.gpu.engines.clear();
        snap.gpu.engines.push(GpuEngineSample {
            engine_name: "3D".into(),
            utilization_bp: 9_900,
        });
        snap.gpu.frametime_jitter_us = 9_000;
        snap.cpu.dpc_isr_busy_bp = 4_000; // above DPC threshold => dependency exists
    }
    let (report, _) = build_window(samples);
    let gpu = report
        .findings
        .iter()
        .find(|finding| finding.code == "GPU_BOUND_WORKLOAD")
        .unwrap();
    let dpc = report
        .findings
        .iter()
        .find(|finding| finding.code == "DPC_ISR_PRESSURE")
        .unwrap_or_else(|| {
            panic!(
                "DPC finding absent; codes={:?}",
                report
                    .findings
                    .iter()
                    .map(|f| f.code.clone())
                    .collect::<Vec<_>>()
            )
        });
    assert!(
        gpu.caused_by_finding_ids.contains(&dpc.id),
        "causality edge must be materialized"
    );
}

#[test]
fn working_set_bloat_measures_growth_not_absolute_size() {
    let mut first = idle_snapshot(1_700_000_000_000);
    first.memory.modified_page_list_bytes = 128 * 1024 * 1024;
    let mut last = idle_snapshot(1_700_000_011_000);
    last.memory.modified_page_list_bytes = 128 * 1024 * 1024 + 600 * 1024 * 1024;
    let mut filler = Vec::new();
    for index in 1..7 {
        let mut snap = idle_snapshot(1_700_000_000_000 + index * 1000);
        snap.memory.modified_page_list_bytes = first.memory.modified_page_list_bytes;
        filler.push(snap);
    }
    let (report, _) = build_window([vec![first], filler, vec![last]].concat());
    let bloat = report
        .findings
        .iter()
        .find(|finding| finding.code == "WORKING_SET_BLOAT")
        .expect("bloat must be detected from growth");
    assert_eq!(bloat.role, Role::ContributingCondition);
}

#[test]
fn identical_windows_have_identical_digests_regardless_of_insertion_timing() {
    let make = || {
        let samples: Vec<PerfSnapshot> = (0..12)
            .map(|index| saturated_cpu_snapshot(1_700_000_000_000 + index * 1000))
            .collect();
        build_window(samples).0
    };
    let a = make();
    std::thread::sleep(std::time::Duration::from_millis(5));
    let b = make();
    assert_eq!(a.digest_sha256, b.digest_sha256);
    assert_eq!(a.report_id, b.report_id);
    assert_eq!(a.findings.len(), b.findings.len());
}

#[test]
fn hostile_extreme_window_stays_bounded_and_never_panics() {
    let mut hostile = saturated_cpu_snapshot(1_700_000_000_000);
    hostile.cpu.per_processor_busy_bp = vec![u32::MAX; 4096];
    hostile.memory.hard_faults_per_sec = u64::MAX;
    hostile.memory.commit_bytes = u64::MAX;
    hostile.memory.commit_limit_bytes = 1;
    let samples: Vec<PerfSnapshot> = (0..thresholds::MIN_SAMPLES_FOR_FINDINGS as usize)
        .map(|_| hostile.clone())
        .collect();
    let (report, _) = build_window(samples);
    // Commit ratio clamps to 10000 bp rather than overflowing.
    for finding in &report.findings {
        for ev in &finding.evidence {
            assert!(ev.observed_value.is_finite());
        }
    }
    assert!(report.findings.len() <= 8);
}

/// `analyze` takes the aggregate and window from two separate ring reads. If
/// the window contains no storage measurement, it cannot date or corroborate
/// the aggregate's peak. A root-cause finding must wait for matching evidence.
#[test]
fn io_saturation_drops_when_no_device_reported() {
    let aggregate = aethercore_performance_telemetry::WindowAggregate {
        sample_count: 12,
        window_ms: 12_000,
        storage_active_bp_peak: thresholds::STORAGE_SATURATION_PEAK_BP,
        storage_active_bp_avg: thresholds::STORAGE_SATURATION_AVG_BP,
        ..Default::default()
    };
    let window: Vec<PerfSnapshot> = (0..12)
        .map(|index| PerfSnapshot {
            captured_unix_ms: 1_700_000_000_000 + index * 1_000,
            storage: Vec::new(),
            ..Default::default()
        })
        .collect();
    let report = analyze(&aggregate, &window, 1_700_000_100_000);
    assert!(report.findings.iter().all(|f| f.code != "IO_SATURATION"));
}

/// The other half of the same distinction: when devices DO report, the latency
/// evidence is still cited exactly as before.
#[test]
fn io_saturation_still_cites_a_latency_devices_did_report() {
    let aggregate = aethercore_performance_telemetry::WindowAggregate {
        sample_count: 12,
        window_ms: 12_000,
        storage_active_bp_peak: thresholds::STORAGE_SATURATION_PEAK_BP,
        storage_active_bp_avg: thresholds::STORAGE_SATURATION_AVG_BP,
        ..Default::default()
    };
    let window: Vec<PerfSnapshot> = (0..12)
        .map(|index| PerfSnapshot {
            captured_unix_ms: 1_700_000_000_000 + index * 1_000,
            storage: vec![StorageQueueSample {
                avg_transfer_latency_us: thresholds::TRANSFER_LATENCY_US,
                ..Default::default()
            }],
            ..Default::default()
        })
        .collect();
    let report = analyze(&aggregate, &window, 1_700_000_100_000);
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.code == "IO_SATURATION")
        .expect("storage saturation finding");
    assert!(
        finding
            .evidence
            .iter()
            .any(|ev| ev.fact_key == "storage.transferLatencyUs"),
        "a measured latency must still be cited"
    );
    assert_eq!(finding.confidence, Confidence::Confirmed);
}
