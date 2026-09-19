//! P44 1.A/1.B — the regression tests for DBT-P42-005 (macOS/Linux still build
//! `PerfSnapshot` literally rather than through `Reading<T>`/`CollectedSubsystems`).
//!
//! §44 1.A found the same defect class §20.1 found on Windows, in different
//! shapes:
//!
//! - macOS/Linux `cpu.dpcIsrBusyBp` and `cpu.contextSwitchesPerSec` are
//!   permanently `0` — no line of either provider ever writes them — with no
//!   fault ever raised to say so, forever, regardless of machine state.
//! - macOS/Linux storage's `avgTransferLatencyUs`/`readBytesPerSec`/
//!   `writeBytesPerSec` are permanently `0` for the same reason.
//! - Linux `processTop` is `Vec::new()` unconditionally
//!   (`linux_impl.rs:512-514`) with **zero fault ever pushed** — the exact
//!   §20.1.1 site 7 shape that was deleted from Windows and replaced with a
//!   `NotCollected` reading.
//! - Linux `sample_power` scans `/sys/class/thermal_zone*/temp`
//!   (`linux_impl.rs:472`), uses the result **only** to decide whether to push
//!   a fault, and never writes a single reading into `PowerSample` even when
//!   real temperature data was found — real evidence collected and discarded,
//!   with no fault in the case where data existed.
//!
//! These are written and committed FAILING before the Part 1.C fix, matching
//! the practice §42.1 established for Windows.

#![cfg(any(target_os = "macos", target_os = "linux"))]

use std::time::Duration;

use aethercore_performance_telemetry::{CollectorFault, PerfPlatform, PerfSnapshot};

fn faults_for<'a>(snapshot: &'a PerfSnapshot, collector: &str) -> Vec<&'a CollectorFault> {
    snapshot
        .collector_faults
        .iter()
        .filter(|fault| fault.collector == collector || fault.collector.starts_with(collector))
        .collect()
}

/// Serializes every `under_load` call in this file. Two concurrent full-core
/// spin harnesses in the same process (libtest's default parallelism) starve
/// each other's threads inside the 120ms in-tick delta window, which produced
/// a spurious `totalBusyBp: 0` reading during development of this file — a
/// test-isolation artifact, not a product defect (confirmed by re-running
/// with `--test-threads=1`, where the same assertion passed). Serializing
/// here removes the confound rather than papering over it with a retry.
///
/// This guard is process-local: it does not serialize against
/// `native_providers.rs`'s or `phase27_real_sample.rs`'s own real-sampling
/// tests, which run as separate `cargo test` binaries.
///
/// **`totalBusyBp: 0` measured at ~1-in-10 under this exact harness on this
/// real Apple Silicon host — DBT-P44-003, CLOSED by SESSION_CONTEXT.md §45.**
/// §45.0 confirmed the mechanism by instrumentation against pristine P44
/// source (not assumed): every flake showed `busy_bp_from_ticks`'s two
/// tick observations byte-for-byte identical (`total == 0`) — a window that
/// was genuinely too short, not a different defect. §45.2 pushed the
/// contract to this exact field boundary — `busy_bp_from_ticks` now returns
/// `Option<u32>`, `None` on the tie — and the caller retries within the
/// sampling budget before giving up. §45.3 re-measured this same harness at
/// 1/50 (2%), down from 7/40 (17.5%); the one remaining occurrence carries
/// an honest `cpu: Unavailable` fault naming the retry exhaustion, not a
/// silent zero. Full elimination was not attempted (would need either an
/// unbounded retry or a different tick source) — recorded as DBT-P45-004,
/// open, not worked around. Does not undermine the real-provider proof in
/// §44.4 — four manual `aetherctl telemetry-once` invocations under real
/// load, run directly rather than through this harness, never returned
/// zero.
static LOAD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Loads every logical processor hard for the duration of `body`, mirroring
/// `dbt_p41_002.rs`'s `under_load` exactly so the lower-bound assertion cannot
/// be satisfied by an idle box.
fn under_load<T>(body: impl FnOnce() -> T) -> T {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    let _guard = LOAD_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let stop = Arc::new(AtomicBool::new(false));
    let threads: Vec<_> = (0..std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4))
        .map(|_| {
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                let mut acc = 0u64;
                while !stop.load(Ordering::Relaxed) {
                    for i in 0..50_000u64 {
                        acc = acc.wrapping_add(i.wrapping_mul(2_654_435_761));
                    }
                }
                acc
            })
        })
        .collect();
    std::thread::sleep(Duration::from_millis(300));
    let out = body();
    stop.store(true, Ordering::Relaxed);
    for thread in threads {
        let _ = thread.join();
    }
    out
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use aethercore_performance_telemetry::MacosPerfPlatform;

    fn real_snapshot_under_load() -> PerfSnapshot {
        under_load(|| MacosPerfPlatform::new().sample(Duration::from_millis(1_000)))
    }

    #[test]
    fn real_macos_provider_reports_non_zero_cpu_under_load() {
        let snapshot = real_snapshot_under_load();
        let cpu = &snapshot.cpu;
        // DBT-P45-001: `totalBusyBp: 0` is legitimate now in exactly one case —
        // every retry inside the sampling budget still tied (§45.2's bounded
        // retry exhausted), and the whole `cpu` subsystem reports
        // `Reading::unavailable` with a fault naming it. That is an honest
        // "could not measure", not the silent lie this test originally caught
        // (a confident zero with no `cpu` fault at all). Distinguish the two
        // rather than asserting non-zero unconditionally, which would make this
        // test flake forever on the rare, now-honestly-reported case.
        let cpu_unavailable = faults_for(&snapshot, "cpu")
            .iter()
            .any(|fault| fault.collector == "cpu" && fault.detail.contains("tick delta"));
        assert!(
            cpu.total_busy_bp > 0 || cpu_unavailable,
            "every logical processor was spinning; totalBusyBp must not be a silent 0 — either \
             a real measurement or an honest `cpu` Unavailable fault naming the tick-delta \
             retry exhaustion. cpu={cpu:?} faults={:?}",
            snapshot.collector_faults
        );
    }

    /// §44 1.A: `dpc_isr_busy_bp` and `context_switches_per_sec` are never
    /// written by `macos_impl.rs` — always `0`, with no fault. cpu is otherwise
    /// reported as fully measured (no `cpu` fault), so a consumer cannot tell
    /// "genuinely zero" from "this platform never measures it".
    #[test]
    fn no_collector_returns_an_empty_payload_without_a_fault() {
        let snapshot = real_snapshot_under_load();
        let mut unaccounted: Vec<String> = Vec::new();

        let cpu_fully_measured = faults_for(&snapshot, "cpu").is_empty();
        if cpu_fully_measured
            && snapshot.cpu.dpc_isr_busy_bp == 0
            && snapshot.cpu.context_switches_per_sec == 0
            && faults_for(&snapshot, "cpu.counters").is_empty()
        {
            unaccounted.push(format!(
                "cpu: dpcIsrBusyBp=0 contextSwitchesPerSec=0 with cpu reported fully measured \
                 and no cpu.counters degradation fault; cpu={:?}",
                snapshot.cpu
            ));
        }

        if !snapshot.storage.is_empty() && faults_for(&snapshot, "storage").is_empty() {
            let all_rates_zero = snapshot.storage.iter().all(|device| {
                device.avg_transfer_latency_us == 0
                    && device.read_bytes_per_sec == 0
                    && device.write_bytes_per_sec == 0
            });
            if all_rates_zero && faults_for(&snapshot, "storage.rates").is_empty() {
                unaccounted.push(format!(
                    "storage: {} device(s) measured, every rate/latency field 0, \
                     no storage.rates degradation fault",
                    snapshot.storage.len()
                ));
            }
        }

        // processTop: macos_impl.rs:387-391 silently skips a pid whose
        // proc_pidinfo call fails ("this is not a provider fault"). If every
        // pid fails that call, sample_process_top returns [] with no fault at
        // all — only the earlier proc_listpids<=0 branch is faulted.
        if snapshot.process_top.is_empty() && faults_for(&snapshot, "processTop").is_empty() {
            unaccounted.push("processTop: [] with no fault".to_string());
        }

        assert!(
            unaccounted.is_empty(),
            "collectors returned success with nothing measured (or a permanent-zero field) and \
             nothing declared:\n  {}\nall faults: {:?}",
            unaccounted.join("\n  "),
            snapshot.collector_faults
        );
    }

    /// §41.15 3.C's shape, re-checked here: `capabilities` must not answer
    /// `native` for a subsystem the collector just reported nothing for.
    /// Unlike pre-fix Windows this already reconciles via
    /// `matrix_for_current_platform_observed` (Phase 27) — recorded here as a
    /// property that must keep holding, not a new fix.
    #[test]
    fn capabilities_never_claim_native_for_a_subsystem_that_reported_nothing() {
        use aethercore_platform_capabilities::{Availability, TelemetryObservation};

        let snapshot = real_snapshot_under_load();
        let measured = snapshot.measured_subsystems();
        let observation = TelemetryObservation {
            cpu: measured.cpu,
            memory: measured.memory,
            storage: measured.storage,
            gpu: measured.gpu,
        };
        let matrix =
            aethercore_platform_capabilities::matrix_for_current_platform_observed(observation);
        let state_of = |name: &str| -> Availability {
            matrix
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, a)| a.clone())
                .unwrap_or_else(|| panic!("capability {name} missing from the matrix"))
        };

        let checks: [(&str, bool); 4] = [
            ("telemetryCpu", measured.cpu),
            ("telemetryMemory", measured.memory),
            ("telemetryStorage", measured.storage),
            ("telemetryGpu", measured.gpu),
        ];
        let mut contradictions: Vec<String> = Vec::new();
        for (name, was_measured) in checks {
            if !was_measured && matches!(state_of(name), Availability::Native) {
                contradictions.push(format!("{name}: native while not measured"));
            }
        }
        assert!(
            contradictions.is_empty(),
            "capabilities contradicts the collectors: {}\nfaults: {:?}",
            contradictions.join(", "),
            snapshot.collector_faults
        );
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use aethercore_performance_telemetry::LinuxPerfPlatform;

    fn real_snapshot_under_load() -> PerfSnapshot {
        under_load(|| LinuxPerfPlatform::new().sample(Duration::from_millis(1_000)))
    }

    #[test]
    fn real_linux_provider_reports_non_zero_cpu_under_load() {
        let snapshot = real_snapshot_under_load();
        let cpu = &snapshot.cpu;
        assert!(
            cpu.total_busy_bp > 0,
            "every logical processor was spinning; totalBusyBp must not be 0. \
             cpu={cpu:?} faults={:?}",
            snapshot.collector_faults
        );
    }

    /// §44 1.A: three independent defects, all "success with nothing declared".
    #[test]
    fn no_collector_returns_an_empty_payload_without_a_fault() {
        let snapshot = real_snapshot_under_load();
        let mut unaccounted: Vec<String> = Vec::new();

        let cpu_fully_measured = faults_for(&snapshot, "cpu").is_empty();
        if cpu_fully_measured
            && snapshot.cpu.dpc_isr_busy_bp == 0
            && snapshot.cpu.context_switches_per_sec == 0
            && faults_for(&snapshot, "cpu.counters").is_empty()
        {
            unaccounted.push(format!(
                "cpu: dpcIsrBusyBp=0 contextSwitchesPerSec=0, cpu fully measured, \
                 no cpu.counters fault; cpu={:?}",
                snapshot.cpu
            ));
        }

        if !snapshot.storage.is_empty() && faults_for(&snapshot, "storage").is_empty() {
            let all_rates_zero = snapshot.storage.iter().all(|device| {
                device.active_time_bp == 0
                    && device.avg_transfer_latency_us == 0
                    && device.read_bytes_per_sec == 0
                    && device.write_bytes_per_sec == 0
            });
            if all_rates_zero && faults_for(&snapshot, "storage.rates").is_empty() {
                unaccounted.push(format!(
                    "storage: {} device(s) measured, every rate/latency/active-time field 0, \
                     no storage.rates degradation fault",
                    snapshot.storage.len()
                ));
            }
        }

        // linux_impl.rs:512-514: process_top is Vec::new() unconditionally,
        // with literally zero fault ever pushed for it — the exact §20.1.1
        // site 7 shape Windows deleted.
        if snapshot.process_top.is_empty() && faults_for(&snapshot, "processTop").is_empty() {
            unaccounted.push(
                "processTop: [] with no fault (unconditional, linux_impl.rs:512-514)".to_string(),
            );
        }

        assert!(
            unaccounted.is_empty(),
            "collectors returned success with nothing measured (or a permanent-zero field) and \
             nothing declared:\n  {}\nall faults: {:?}",
            unaccounted.join("\n  "),
            snapshot.collector_faults
        );
    }

    /// linux_impl.rs:472-500: `scan_thermal_zones()` is read only to decide
    /// whether to fault; its millidegree readings are never written into
    /// `PowerSample`. If real thermal zones exist on this host, `power` must
    /// still come back honestly empty with no fault at all — real evidence,
    /// discarded silently.
    #[test]
    fn thermal_zone_evidence_is_not_discarded_without_a_fault() {
        let zones_exist = std::fs::read_dir("/sys/class")
            .map(|entries| {
                entries.flatten().any(|entry| {
                    entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| name.starts_with("thermal_zone"))
                })
            })
            .unwrap_or(false);
        if !zones_exist {
            // Nothing to discard on this host; not this test's job to fabricate a zone.
            return;
        }
        let snapshot = real_snapshot_under_load();
        let power_has_no_reading =
            !snapshot.power.has_temperature && snapshot.power.temperature_c == 0;
        let power_faulted = !faults_for(&snapshot, "thermalPower").is_empty()
            || !faults_for(&snapshot, "power").is_empty();
        assert!(
            !(power_has_no_reading && !power_faulted),
            "real thermal zones exist on this host but PowerSample carries no reading and no \
             fault explains it: power={:?} faults={:?}",
            snapshot.power,
            snapshot.collector_faults
        );
    }

    #[test]
    fn capabilities_never_claim_native_for_a_subsystem_that_reported_nothing() {
        use aethercore_platform_capabilities::{Availability, TelemetryObservation};

        let snapshot = real_snapshot_under_load();
        let measured = snapshot.measured_subsystems();
        let observation = TelemetryObservation {
            cpu: measured.cpu,
            memory: measured.memory,
            storage: measured.storage,
            gpu: measured.gpu,
        };
        let matrix =
            aethercore_platform_capabilities::matrix_for_current_platform_observed(observation);
        let state_of = |name: &str| -> Availability {
            matrix
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, a)| a.clone())
                .unwrap_or_else(|| panic!("capability {name} missing from the matrix"))
        };
        let checks: [(&str, bool); 4] = [
            ("telemetryCpu", measured.cpu),
            ("telemetryMemory", measured.memory),
            ("telemetryStorage", measured.storage),
            ("telemetryGpu", measured.gpu),
        ];
        let mut contradictions: Vec<String> = Vec::new();
        for (name, was_measured) in checks {
            if !was_measured && matches!(state_of(name), Availability::Native) {
                contradictions.push(format!("{name}: native while not measured"));
            }
        }
        assert!(
            contradictions.is_empty(),
            "capabilities contradicts the collectors: {}\nfaults: {:?}",
            contradictions.join(", "),
            snapshot.collector_faults
        );
    }
}
