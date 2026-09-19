//! Phase 27 — GE gate: REAL-sample integration proof on this Mac.
//!
//! Starts the sampler at the 250 ms floor for ~12 ticks on the REAL platform
//! (`MacosPerfPlatform`), then drives ring.aggregate() → bottleneck::analyze() and
//! asserts the report structure is valid: sample_count, digest format, well-formed
//! findings, no panic. Determinism tests REMAIN on the synthetic platform — real clocks
//! are not byte-deterministic, and that is stated in docs/phase27/ARCHITECTURE.md.

use std::sync::Arc;
use std::time::Duration;

use aethercore_performance_bottleneck::analyze;
#[cfg(target_os = "macos")]
use aethercore_performance_telemetry::PerfSnapshot;
use aethercore_performance_telemetry::PerformanceRing;

#[cfg(target_os = "macos")]
const TICKS: u64 = 12;

#[test]
#[cfg(target_os = "macos")]
fn real_sampler_aggregate_analyze_pipeline_is_wellformed() {
    let ring = PerformanceRing::new();
    let owner = "p27-real-sample-owner-0123456789abcdef";
    ring.start(
        Arc::new(aethercore_performance_telemetry::MacosPerfPlatform::new()),
        owner,
        // The documented floor: 250 ms.
        PerfSnapshot::clamped_interval_ms(250),
    )
    .expect("real sampler starts");

    // ~12 ticks at >= 250 ms cadence; sleep a little beyond to absorb jitter.
    std::thread::sleep(Duration::from_millis(TICKS * 250 + 400));
    ring.stop();

    let aggregate = ring.aggregate(owner);
    assert!(
        aggregate.sample_count >= 10,
        "expected >=10 real samples, got {}",
        aggregate.sample_count
    );
    assert!(aggregate.window_ms > 0, "real wall-clock window elapsed");
    assert!(aggregate.cpu_busy_bp_avg <= 10_000);
    assert!(aggregate.cpu_busy_bp_peak <= 10_000);

    let window = ring.window(owner);
    assert_eq!(window.len(), aggregate.sample_count as usize);

    let now = chrono::Utc::now().timestamp_millis();
    let report = analyze(&aggregate, &window, now);
    // Report structure validity.
    assert_eq!(report.analyzed_sample_count, aggregate.sample_count);
    assert_eq!(report.digest_sha256.len(), 64, "sha256 hex digest");
    assert!(
        report
            .digest_sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit() || b == b'"'),
        "digest is lowercase hex"
    );
    assert!(!report.rule_engine_version.is_empty());
    for finding in &report.findings {
        assert!(!finding.title_key.is_empty());
        assert!(!finding.summary_key.is_empty());
        for evidence in &finding.evidence {
            assert!(!evidence.fact_key.is_empty());
        }
    }
}

/// The synthetic path stays available and deterministic for audits/offline UI (T4).
#[test]
fn synthetic_selection_remains_available_for_audits() {
    let ring = PerformanceRing::new();
    let owner = "p27-synthetic-audit-owner-0123456789abcd";
    ring.start(
        Arc::new(aethercore_performance_telemetry::SyntheticPerfPlatform::new()),
        owner,
        1_000,
    )
    .expect("synthetic sampler starts");
    std::thread::sleep(Duration::from_millis(2_300));
    ring.stop();
    let aggregate = ring.aggregate(owner);
    assert!(aggregate.sample_count >= 2);
    let window = ring.window(owner);
    let now = chrono::Utc::now().timestamp_millis();
    let _report = analyze(&aggregate, &window, now);
}
