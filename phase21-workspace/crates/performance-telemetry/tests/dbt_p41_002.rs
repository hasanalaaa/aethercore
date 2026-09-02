//! P42 1.A — the regression tests for DBT-P41-002, written before the fix.
//!
//! §20.1.5 records why this file has to exist: **no test anywhere constructs
//! `WindowsPerfPlatform`**, and both real-provider tests assert upper bounds
//! only (`phase27_real_sample.rs:41-43`, `phase28_cli_matrix.rs:177-181`), so an
//! all-zero snapshot satisfies every one of them. That is the §17.7 shape — eight
//! tests passing without one traversing the real writer — recurring exactly.
//!
//! These four tests assert the four things the shipping code gets wrong:
//!
//! 1. the real Windows provider must report a **non-zero** CPU on a busy machine
//!    (a LOWER bound; the upper bound is the hole that hid this);
//! 2. `read_u64` must decode `largeValue` at offset 8, not `CStatus` at offset 0;
//! 3. no collector may return a payload with no evidence and no fault;
//! 4. `capabilities` may not answer `native` for a subsystem whose collector
//!    reported nothing.
//!
//! Tests 1, 3 and 4 are Windows-only by nature: they exercise the Windows
//! provider on a real Windows host. Test 2 is Windows-only because the seam it
//! reads is `#[cfg(windows)]`.

#![cfg(windows)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use aethercore_performance_telemetry::{
    CollectorFault, PerfPlatform, PerfSnapshot, WindowsPerfPlatform,
};

/// Loads every logical processor hard for the duration of `body`.
///
/// §41.15 measured `\Processor(_Total)\% Processor Time` at 7.01% while the
/// product reported `totalBusyBp: 0`, so the machine was already provably not
/// idle. This makes the point unarguable: the assertion below must not be able
/// to fail because the box happened to be quiet.
fn under_load<T>(body: impl FnOnce() -> T) -> T {
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
    // Let the load land before the counters are read.
    std::thread::sleep(Duration::from_millis(300));
    let out = body();
    stop.store(true, Ordering::Relaxed);
    for thread in threads {
        let _ = thread.join();
    }
    out
}

fn real_snapshot_under_load() -> PerfSnapshot {
    under_load(|| WindowsPerfPlatform.sample(Duration::from_millis(1_000)))
}

fn faults_for<'a>(snapshot: &'a PerfSnapshot, collector: &str) -> Vec<&'a CollectorFault> {
    snapshot
        .collector_faults
        .iter()
        .filter(|fault| fault.collector == collector || fault.collector.starts_with(collector))
        .collect()
}

// ---------------------------------------------------------------------------
// 1 — the real provider, on a machine that is provably doing work
// ---------------------------------------------------------------------------

/// §20.1.5: the only real-provider assertions in the tree are `<= 10_000`, which
/// an all-zero CPU satisfies. This asserts the lower bound instead.
#[test]
fn real_windows_provider_reports_non_zero_cpu_under_load() {
    let snapshot = real_snapshot_under_load();
    let cpu = &snapshot.cpu;
    assert!(
        cpu.total_busy_bp > 0,
        "every logical processor was spinning; totalBusyBp must not be 0. \
         cpu={cpu:?} faults={:?}",
        snapshot.collector_faults
    );
    // §41.15 measured 7.01% (701 bp) through PDH on an *unloaded* box. Under a
    // full-core spin the product must be at least in that order of magnitude.
    assert!(
        cpu.total_busy_bp >= 500,
        "expected at least ~5% busy under a full-core spin, got {} bp",
        cpu.total_busy_bp
    );
    // Context switches are never 0 on a live Windows host; a 0 here is the same
    // misread wearing a different field name.
    assert!(
        cpu.context_switches_per_sec > 0,
        "contextSwitchesPerSec must not be 0 on a running Windows machine"
    );
}

// ---------------------------------------------------------------------------
// 2 — the ABI: largeValue at offset 8, not CStatus at offset 0
// ---------------------------------------------------------------------------

/// §20.1.3(a): `PDH_FMT_COUNTERVALUE` is `{ CStatus: u32, <pad>, union }` —
/// 16 bytes, `largeValue` at offset 8. The shipping code passes an 8-byte
/// destination, so it reads `CStatus`, and `PDH_CSTATUS_VALID_DATA == 0`.
#[test]
fn pdh_value_is_decoded_from_large_value_not_cstatus() {
    use aethercore_performance_telemetry::__test::{PDH_VALUE_SLOT_BYTES, decode_pdh_value};

    // A destination smaller than the struct PDH writes is an out-of-bounds write
    // on every counter read, not merely a wrong number (§41.17 DBT-P41-002b).
    assert!(
        PDH_VALUE_SLOT_BYTES >= 16,
        "PdhGetFormattedCounterValue writes a 16-byte PDH_FMT_COUNTERVALUE; \
         supplying {PDH_VALUE_SLOT_BYTES} bytes overflows the destination"
    );

    // Offset 0 and offset 8 are deliberately different so the two are
    // distinguishable: CStatus = PDH_CSTATUS_VALID_DATA (0), measurement = 701 bp
    // — the value §41.15 measured on this machine.
    let mut slot = [0u8; 16];
    slot[0..4].copy_from_slice(&0u32.to_le_bytes()); // CStatus = VALID_DATA
    slot[8..16].copy_from_slice(&701i64.to_le_bytes()); // largeValue
    assert_eq!(
        decode_pdh_value(&slot),
        Some(701),
        "decoded the status code instead of the measurement"
    );

    // The other direction: a non-zero CStatus must never be mistaken for data.
    let mut slot = [0u8; 16];
    slot[0..4].copy_from_slice(&1u32.to_le_bytes()); // PDH_CSTATUS_NEW_DATA
    slot[8..16].copy_from_slice(&4_242i64.to_le_bytes());
    assert_eq!(
        decode_pdh_value(&slot),
        Some(4_242),
        "a status code leaked into the reading"
    );

    // A negative formatted value is not a measurement.
    let mut slot = [0u8; 16];
    slot[8..16].copy_from_slice(&(-5i64).to_le_bytes());
    assert_eq!(decode_pdh_value(&slot), Some(0));
}

// ---------------------------------------------------------------------------
// 3 — the contract: no evidence means a fault, always
// ---------------------------------------------------------------------------

/// §20.1.4 — this is the defect itself. `PerfSnapshot::default()` is a legal,
/// type-checking return from `PerfPlatform::sample`, indistinguishable from a
/// real reading of an idle machine. Nine hand-written rules compensate; gpu's is
/// the only one that fires, and §20.1.6 records that it fires by accident.
#[test]
fn no_collector_returns_an_empty_payload_without_a_fault() {
    let snapshot = real_snapshot_under_load();
    let mut unaccounted: Vec<String> = Vec::new();

    // cpu: every PDH-sourced scalar zero at once is not a reading (§41.15 3.B).
    let cpu_measured = snapshot.cpu.total_busy_bp > 0
        || !snapshot.cpu.per_processor_busy_bp.is_empty()
        || snapshot.cpu.context_switches_per_sec > 0;
    if !cpu_measured && faults_for(&snapshot, "cpu").is_empty() {
        unaccounted.push(format!("cpu: {:?} with no fault", snapshot.cpu));
    }

    // storage: `[]` with no fault — four fault-free early returns (§20.1.2).
    if snapshot.storage.is_empty() && faults_for(&snapshot, "storage").is_empty() {
        unaccounted.push("storage: [] with no fault".to_string());
    }

    // memory: headline fields come from GlobalMemoryStatusEx, so a zero total is
    // a genuine failure to read.
    if snapshot.memory.total_physical_bytes == 0 && faults_for(&snapshot, "memory").is_empty() {
        unaccounted.push("memory: totalPhysicalBytes 0 with no fault".to_string());
    }

    // gpu: the one collector that already checks itself.
    let gpu_measured =
        !snapshot.gpu.engines.is_empty() || !snapshot.gpu.adapter_id.is_empty();
    if !gpu_measured && faults_for(&snapshot, "gpu").is_empty() {
        unaccounted.push("gpu: no engines, no adapter, no fault".to_string());
    }

    // processTop: `let _ = faults;` then `Vec::new()`, unconditionally
    // (§20.1.1 site 7). "By design" still owes the caller a reason.
    if snapshot.process_top.is_empty() && faults_for(&snapshot, "processTop").is_empty() {
        unaccounted.push("processTop: [] with no fault".to_string());
    }

    assert!(
        unaccounted.is_empty(),
        "collectors returned success with nothing measured and nothing declared:\n  {}\n\
         all faults: {:?}",
        unaccounted.join("\n  "),
        snapshot.collector_faults
    );
}

// ---------------------------------------------------------------------------
// 4 — capabilities may not contradict the collectors
// ---------------------------------------------------------------------------

/// §41.15 3.C measured the contradiction: 16/16 `native` in the same session in
/// which storage returns `[]` and gpu declares `Unavailable`. `windows_table()`
/// maps every capability to `Native` with no runtime input at all
/// (`platform-capabilities/src/lib.rs:254-257`), so it will say `native` on every
/// Windows host in every state.
#[test]
fn capabilities_never_claim_native_for_a_subsystem_that_reported_nothing() {
    use aethercore_platform_capabilities::Availability;

    let snapshot = real_snapshot_under_load();
    let matrix = aethercore_platform_capabilities::matrix_for_current_platform();
    let state_of = |name: &str| -> Availability {
        matrix
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, a)| a.clone())
            .unwrap_or_else(|| panic!("capability {name} missing from the matrix"))
    };

    let observed: [(&str, bool, String); 4] = [
        (
            "telemetryCpu",
            snapshot.cpu.total_busy_bp > 0 || snapshot.cpu.context_switches_per_sec > 0,
            format!("{:?}", snapshot.cpu),
        ),
        (
            "telemetryMemory",
            snapshot.memory.total_physical_bytes > 0,
            format!("totalPhysicalBytes={}", snapshot.memory.total_physical_bytes),
        ),
        (
            "telemetryStorage",
            !snapshot.storage.is_empty(),
            format!("{} devices", snapshot.storage.len()),
        ),
        (
            "telemetryGpu",
            !snapshot.gpu.engines.is_empty() || !snapshot.gpu.adapter_id.is_empty(),
            format!("{} engines", snapshot.gpu.engines.len()),
        ),
    ];

    let mut contradictions: Vec<String> = Vec::new();
    for (name, measured, evidence) in observed {
        if !measured && matches!(state_of(name), Availability::Native) {
            contradictions.push(format!(
                "{name}: reported `native` while the collector produced {evidence}"
            ));
        }
    }

    assert!(
        contradictions.is_empty(),
        "capabilities contradicts the collectors in the same process:\n  {}\n\
         faults: {:?}",
        contradictions.join("\n  "),
        snapshot.collector_faults
    );
}
