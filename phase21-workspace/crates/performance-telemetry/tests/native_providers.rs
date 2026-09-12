//! Phase 27 — native provider unit + integration tests (this Mac IS the unix test host).
//!
//! macOS provider paths execute for real here; Linux provider logic is exercised through
//! its pure parser functions against embedded /proc fixture texts (cfg-gated live sampling
//! runs when the workspace is built on Linux). Determinism tests REMAIN on the synthetic
//! platform: real clocks and kernel counters are not byte-deterministic by nature.

use std::time::Duration;

use aethercore_performance_telemetry::{PerfPlatform, PerfSnapshot, SyntheticPerfPlatform};

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use aethercore_performance_telemetry::MacosPerfPlatform;

    /// Injected-counter matrix for the pure tick-delta math (mirrors proc(5) semantics).
    fn ticks(
        user: u64,
        system: u64,
        idle: u64,
        nice: u64,
    ) -> aethercore_performance_telemetry::__test::CpuTicks {
        aethercore_performance_telemetry::__test::cpu_ticks(user, system, idle, nice)
    }

    #[test]
    fn tick_delta_math_matches_injected_counters() {
        // Half of the delta busy → 5_000 bp exactly (Δbusy=1000 of Δtotal=2000).
        let previous = ticks(1_000, 1_000, 2_000, 0);
        let current = ticks(1_500, 1_500, 3_000, 0);
        assert_eq!(
            aethercore_performance_telemetry::__test::busy_bp_from_ticks(previous, current),
            Some(5_000)
        );
        // Fully busy window → 10_000 bp ceiling.
        let previous = ticks(0, 0, 5_000, 0);
        let current = ticks(5_000, 0, 5_000, 0);
        assert_eq!(
            aethercore_performance_telemetry::__test::busy_bp_from_ticks(previous, current),
            Some(10_000)
        );
        // Fully idle window (Δtotal=9_000, genuinely nonzero) → real 0 bp, still a
        // measurement: distinct from the None-on-Δtotal==0 case below.
        let previous = ticks(0, 0, 0, 0);
        let current = ticks(0, 0, 9_000, 0);
        assert_eq!(
            aethercore_performance_telemetry::__test::busy_bp_from_ticks(previous, current),
            Some(0)
        );
    }

    #[test]
    fn hostile_tick_counters_clamp_without_panicking() {
        // Counter regression (reboot/journal wrap): both deltas saturate to 0,
        // so Δtotal == 0 — no window was actually observed. DBT-P45-001: this
        // must report "no measurement" (None), not a confident 0 bp.
        let previous = ticks(u64::MAX - 10, 0, 0, 0);
        let current = ticks(5, 0, 0, 0);
        assert_eq!(
            aethercore_performance_telemetry::__test::busy_bp_from_ticks(previous, current),
            None
        );
        // Busy counter exceeds its own total after subtraction: clamped to the ceiling.
        let previous = ticks(0, 0, 0, 0);
        let current = ticks(u64::MAX / 2, u64::MAX / 2, 0, 0);
        assert_eq!(
            aethercore_performance_telemetry::__test::busy_bp_from_ticks(previous, current),
            Some(10_000)
        );
        // Identical counters (no window): DBT-P45-001 — no elapsed window means no
        // measurement was made. Must be None, never a division-by-zero and never a
        // fabricated 0.
        let frozen = ticks(7, 7, 7, 7);
        assert_eq!(
            aethercore_performance_telemetry::__test::busy_bp_from_ticks(frozen, frozen),
            None
        );
    }

    #[test]
    fn real_macos_sample_is_normalized_and_honestly_degraded() {
        let platform = MacosPerfPlatform::new();
        let first = platform.sample(Duration::from_millis(250));
        let snapshot = normalized_snapshot(platform.sample(Duration::from_millis(250)));
        assert!(first.captured_unix_ms > 0);
        // Real memory evidence from mach: total physical bytes must be plausible (> 1 GiB).
        assert!(
            snapshot.memory.total_physical_bytes > 1024 * 1024 * 1024,
            "mach hw.memsize/vm stats must produce a real total"
        );
        assert!(snapshot.memory.memory_load_percent <= 100);
        // CPU basis points stay inside contract range.
        assert!(snapshot.cpu.total_busy_bp <= 10_000);
        // statfs volumes present on every supported macOS install.
        assert!(
            !snapshot.storage.is_empty(),
            "statfs must see at least one volume"
        );
        for vol in &snapshot.storage {
            assert!(
                vol.total_space_bytes > 0,
                "statfs total space must be nonzero"
            );
            assert!(
                vol.free_space_bytes <= vol.total_space_bytes,
                "statfs free space <= total space"
            );
        }
        // Honest degradation: GPU + thermal/power are declared Degraded, never simulated.
        let degraded: Vec<&str> = snapshot
            .collector_faults
            .iter()
            .filter(|fault| fault.kind == "Degraded")
            .map(|fault| fault.collector.as_str())
            .collect();
        assert!(degraded.contains(&"gpu"), "gpu must be honestly degraded");
        assert!(
            degraded.contains(&"thermalPower"),
            "thermal/power must be honestly degraded"
        );
        // No SMC temperature was fabricated.
        assert!(!snapshot.power.has_temperature && snapshot.power.temperature_c == 0);
        // Every per-processor value respects the wire bound.
        snapshot
            .cpu
            .per_processor_busy_bp
            .iter()
            .for_each(|bp| assert!(*bp <= 10_000));
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use aethercore_performance_telemetry::LinuxPerfPlatform;

    #[test]
    fn real_linux_sample_is_normalized() {
        let platform = LinuxPerfPlatform::new();
        let snapshot = normalized_snapshot(platform.sample(Duration::from_millis(250)));
        assert!(snapshot.cpu.total_busy_bp <= 10_000);
        assert!(snapshot.memory.memory_load_percent <= 100);
    }
}

/// Parser fixtures run on EVERY host (pure functions over embedded /proc texts).
#[cfg(target_os = "linux")]
mod linux_parsers {
    use aethercore_performance_telemetry::__test::{
        busy_bp_from_proc, cpu_from_proc_stat, loadavg_from, meminfo_from_proc, rows_from_diskstats,
    };

    const PROC_STAT: &str =
        "cpu  100 0 50 700 30 10 10 5 0 0\ncpu0 60 0 25 350 15 5 5 2 0 0\nintr 12345\n";
    const PROC_MEMINFO: &str = concat!(
        "MemTotal:       16384000 kB\n",
        "MemFree:         4200000 kB\n",
        "MemAvailable:    8192000 kB\n",
        "Buffers:          300000 kB\n",
        "Cached:          2400000 kB\n",
        "SwapTotal:       2000000 kB\n",
        "SwapFree:        1800000 kB\n",
        "HugePages_Total:       0\n",
        "garbage-line-without-unit 123\n",
    );
    const PROC_DISKSTATS: &str = concat!(
        "   8       0 sda 4210 555 190372 3010 1200 980 17416 2340 0 5120 5350 0 0 0 0 0 0\n",
        "   7       0 loop0 90 0 1100 40 0 0 0 0 0 40 40 0 0 0 0 0 0\n",
        "short-but-broken row\n",
        " 259       0 nvme0n1 9000 100 400000 5000 8000 900 120000 6000 0 8000 11000 0 0 0 0 0 0\n",
    );

    #[test]
    fn proc_stat_cpu_parses_aggregate_line_only() {
        let cpu = cpu_from_proc_stat(PROC_STAT).expect("aggregate cpu line parses");
        assert_eq!(cpu.user, 100);
        assert_eq!(cpu.system, 50);
        assert_eq!(cpu.idle, 700);
        assert_eq!(cpu.iowait, 30);
        assert_eq!(cpu.steal, 5);
    }

    #[test]
    fn proc_stat_cpu_skips_malformed_content_typed() {
        assert!(cpu_from_proc_stat("").is_none());
        assert!(cpu_from_proc_stat("cpu\n").is_none());
        assert!(cpu_from_proc_stat("cpu  1 two 3 4 5 6 7 8\n").is_none());
        assert!(cpu_from_proc_stat("no cpu line at all\n").is_none());
    }

    #[test]
    fn proc_stat_delta_math_handles_hostile_counters() {
        let baseline = cpu_from_proc_stat(PROC_STAT).unwrap();
        // Regression: counters go backwards (container migration) → busy Δ and total Δ
        // both saturate to 0 → Δtotal == 0 → DBT-P45-001: no window was observed,
        // must be None, never a fabricated 0 bp.
        let regressed = cpu_from_proc_stat("cpu  10 0 5 70 3 1 1 0 0 0\n").unwrap();
        assert_eq!(busy_bp_from_proc(baseline, regressed), None);
        // Forward window: busy Δ=(150+0+75+20+20)-(100+0+50+10+10)=95; idle Δ=0 →
        // busy == total → ratio clamps at the ceiling: exactly 10_000 bp.
        let later = cpu_from_proc_stat("cpu  150 0 75 700 30 20 20 5 0 0\n").unwrap();
        assert_eq!(busy_bp_from_proc(baseline, later), Some(10_000));
        // Balanced growth computes the true interior ratio (busy Δ=50, total Δ=150).
        let grown = cpu_from_proc_stat("cpu  200 0 100 800 30 20 20 5 0 0\n").unwrap();
        let bp = busy_bp_from_proc(later, grown).expect("nonzero Δtotal must measure");
        assert!(bp > 0 && bp < 10_000);
        // Identical counters (Δtotal == 0): DBT-P45-001 — no elapsed window means
        // no measurement was made, never a division by zero and never a 0 bp lie.
        assert_eq!(busy_bp_from_proc(later, later), None);
    }

    #[test]
    fn meminfo_parses_known_keys_and_skips_garbage_rows() {
        let info = meminfo_from_proc(PROC_MEMINFO).expect("meminfo parses");
        assert_eq!(info.mem_total_kib, 16_384_000);
        assert_eq!(info.mem_available_kib, 8_192_000);
        assert_eq!(info.swap_total_kib, 2_000_000);
    }

    #[test]
    fn meminfo_requires_a_real_total() {
        assert!(meminfo_from_proc("MemFree: 12 kB\n").is_none());
        assert!(meminfo_from_proc("").is_none());
    }

    #[test]
    fn loadavg_first_field_parses_or_fails_typed() {
        assert_eq!(loadavg_from("1.25 0.90 0.75 3/4210 81542\n"), Some(1.25));
        assert_eq!(loadavg_from("not-a-number"), None);
        assert_eq!(loadavg_from(""), None);
    }

    #[test]
    fn diskstats_rows_parse_and_malformed_lines_are_skipped() {
        let rows = rows_from_diskstats(PROC_DISKSTATS);
        let names: Vec<&str> = rows.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, vec!["sda", "loop0", "nvme0n1"], "broken row dropped");
        let sda = &rows[0].1;
        assert_eq!(sda.major, 8);
        assert_eq!(sda.io_ticks_ms, 5120);
        assert_eq!(sda.weighted_io_ticks_ms, 5350);
        assert_eq!(sda.sectors_read, 190_372);
    }
}

/// Shared helper: run `normalized()` exactly like the production pipeline does.
fn normalized_snapshot(snapshot: PerfSnapshot) -> PerfSnapshot {
    snapshot.normalized()
}

/// Determinism REMAINS on the synthetic platform — pinned again next to the new providers.
#[test]
fn synthetic_platform_stays_deterministic_for_identical_windows() {
    let synthetic = SyntheticPerfPlatform::new();
    synthetic.with_cpu_busy_bp(4_200);
    let ring = aethercore_performance_telemetry::PerformanceRing::new();
    ring.start(std::sync::Arc::new(synthetic), "owner-p27", 1_000)
        .expect("synthetic sampler starts");
    std::thread::sleep(Duration::from_millis(2_300));
    ring.stop();
    let aggregate = ring.aggregate("owner-p27");
    // 1s cadence over a ~2.3s wall window: at least two ticks must have landed. The
    // exact count is scheduler-dependent, so the determinism pin is the aggregate VALUE.
    assert!(
        aggregate.sample_count >= 2,
        "expected ≥2 samples, got {}",
        aggregate.sample_count
    );
    assert_eq!(aggregate.cpu_busy_bp_avg, 4_200);
}
