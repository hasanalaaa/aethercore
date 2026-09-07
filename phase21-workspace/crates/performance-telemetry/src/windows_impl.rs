//! Windows implementation of the Phase 20 performance platform.
//!
//! All native resources are RAII-wrapped. Counter collection uses PDH (Performance Data Helper)
//! in a single passive query per tick — no busy polling, no WMI on the hot path (WMI is only a
//! cold-start fallback for topology). Every collector degrades into `collector_faults` so one
//! broken counter can never stall or fail the snapshot.

use std::time::Duration;

use super::{
    CollectedSubsystems, CollectorFault, CpuSample, GpuEngineSample, GpuSample, MemorySample,
    PerfPlatform, PerfSnapshot, PowerSample, ProcessCpuTopEntry, Reading, StorageQueueSample,
    ThermalThrottleReason,
};
use windows::Win32::System::Power::{
    CallNtPowerInformation, POWER_INFORMATION_LEVEL, PROCESSOR_POWER_INFORMATION,
};

/// The out-parameter `PdhGetFormattedCounterValue` actually writes.
///
/// DBT-P41-002 mechanism 1 (§20.1.3(a)). The shipping binding declared this
/// parameter as `*mut i64` and passed an 8-byte stack local. The real struct
/// (`windows-0.62.2` `System/Performance/mod.rs:9682-9685`) is
/// `{ CStatus: u32, Anonymous: union { longValue: i32, doubleValue: f64,
/// largeValue: i64, ...pointers } }` — `repr(C)`, 16 bytes, align 8, with
/// `largeValue` at **offset 8**. Two consequences, both fixed here:
///
/// 1. the code read bytes 0..8, i.e. `CStatus` plus padding, and
///    `PDH_CSTATUS_VALID_DATA == 0` — so every successful counter read reported
///    a status code as a measurement (§41.15 confirmed `0` across 6 samples);
/// 2. PDH wrote 8 bytes past the end of the destination on **every** counter
///    read — a stack buffer overflow inside an `unsafe` block, present since
///    Phase 20 (DBT-P41-002b). Supplying the correctly-sized destination is the
///    memory-safety fix, not a cosmetic one.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PdhFmtCounterValue {
    pub c_status: u32,
    _padding: u32,
    pub large_value: i64,
}

impl PdhFmtCounterValue {
    /// Safe, explicit re-serialisation so the one interpreter of these bytes is
    /// [`decode_pdh_value`], which is unit-tested without linking PDH.
    fn to_slot(self) -> [u8; PDH_VALUE_SLOT_BYTES] {
        let mut slot = [0u8; PDH_VALUE_SLOT_BYTES];
        slot[0..4].copy_from_slice(&self.c_status.to_le_bytes());
        slot[8..16].copy_from_slice(&self.large_value.to_le_bytes());
        slot
    }
}

/// PDH function table loaded through delayed binding so a missing PDH DLL degrades into a
/// typed fault rather than a load failure of the service binary.
mod pdh {
    use windows::core::{PCWSTR, PWSTR};

    #[link(name = "pdh")]
    unsafe extern "system" {
        pub fn PdhOpenQueryW(datasource: PCWSTR, userdata: usize, query: *mut isize) -> i32;
        pub fn PdhCloseQuery(query: isize) -> i32;
        pub fn PdhAddEnglishCounterW(
            query: isize,
            path: PCWSTR,
            userdata: usize,
            counter: *mut isize,
        ) -> i32;
        pub fn PdhCollectQueryData(query: isize) -> i32;
        // The out-parameter is PPDH_FMT_COUNTERVALUE, not *mut i64. Declaring it
        // correctly is what makes the 8-byte overflow unrepresentable rather
        // than merely fixed at one call site (DBT-P41-002b).
        pub fn PdhGetFormattedCounterValue(
            counter: isize,
            format: u32,
            lptype: *mut u32,
            value: *mut super::PdhFmtCounterValue,
        ) -> i32;
        // P36 (Hermes) bound the real export name `PdhExpandWildCardPathW` to fix
        // LNK2019 on ARM64 — but bound it to the WRONG SIGNATURE, which no linker
        // can catch. **DBT-P42-001, found by this session.** The export takes
        // **five** parameters
        //   (szDataSource, szWildCardPath, mszExpandedPathList, pcchPathListLength, dwFlags)
        // and the shipping declaration had **three**, with the output buffer typed
        // `*mut PWSTR` where the API takes a `PZZWSTR` — a plain buffer. Verified
        // against the vendored bindings this crate already depends on:
        // `windows-0.62.2` `System/Performance/mod.rs:342`.
        //
        // On x64 every argument therefore landed in the wrong register — the
        // wildcard path arrived as `szDataSource`, the out-pointer as
        // `szWildCardPath` — and the callee read `dwFlags` from uninitialised
        // stack beyond the shadow space. That is why `needed` came back 0 with no
        // failure code, which §41.15 could only narrow to "one of the two exits
        // after PdhExpandWildCardPathW": the call never had a chance to succeed.
        // Passing a real buffer through the same broken declaration faults with
        // STATUS_ACCESS_VIOLATION, which is how this was found.
        pub fn PdhExpandWildCardPathW(
            szdatasource: PCWSTR,
            szwildcardpath: PCWSTR,
            mszexpandedpathlist: PWSTR,
            pcchpathlistlength: *mut u32,
            dwflags: u32,
        ) -> i32;
    }

    pub const PDH_MORE_DATA: i32 = 0x8000_07D2u32 as i32;
    /// PERF_SIZE_LARGE | PERF_TYPE_NUMBER (u64 formatted value).
    pub const PDH_FMT_LARGE: u32 = 0x0000_0400;
    /// PDH_FMT_DOUBLE. Percentage and seconds-per-operation counters are
    /// fractional; formatting them as LARGE truncates 7.01% to 7 and
    /// `Avg. Disk sec/Transfer` to 0.
    pub const PDH_FMT_DOUBLE: u32 = 0x0000_0200;
    /// A formatted value is usable under exactly these two statuses.
    pub const PDH_CSTATUS_VALID_DATA: u32 = 0x0000_0000;
    pub const PDH_CSTATUS_NEW_DATA: u32 = 0x0000_0001;
}

fn pdh_ok(code: i32) -> bool {
    code == 0 // ERROR_SUCCESS
}

/// Owned PDH query handle; closes deterministically on drop.
struct QueryHandle(isize);

impl QueryHandle {
    fn open() -> Option<Self> {
        let mut handle = 0isize;
        unsafe {
            if pdh_ok(pdh::PdhOpenQueryW(windows::core::PCWSTR::null(), 0, &mut handle)) {
                Some(Self(handle))
            } else {
                None
            }
        }
    }

    fn add_english_counter(&self, path: &str) -> Option<CounterHandle> {
        let mut wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let mut counter = 0isize;
        unsafe {
            if pdh_ok(pdh::PdhAddEnglishCounterW(
                self.0,
                windows::core::PCWSTR(wide.as_mut_ptr()),
                0,
                &mut counter,
            )) {
                Some(CounterHandle(counter))
            } else {
                None
            }
        }
    }

    fn collect(&self) -> bool {
        unsafe { pdh_ok(pdh::PdhCollectQueryData(self.0)) }
    }

    /// Two observations inside one tick.
    ///
    /// Rate and percentage counters are computed from a delta and have no value
    /// after a single collection — and a query with no counters added yet fails
    /// outright. Both were being got wrong: `sample_storage` collected twice
    /// *before* adding any counter and then read each counter immediately after
    /// adding it, so even with the wildcard expansion working, every read would
    /// have returned nothing. Add counters first, then call this.
    fn collect_twice(&self, gap: Duration) -> bool {
        if !self.collect() {
            return false;
        }
        std::thread::sleep(gap);
        self.collect()
    }
}

impl Drop for QueryHandle {
    fn drop(&mut self) {
        if self.0 != 0 {
            unsafe {
                let _ = pdh::PdhCloseQuery(self.0);
            }
        }
    }
}

/// Size of the destination this code hands `PdhGetFormattedCounterValue`.
///
/// Derived from the type rather than written down, so it cannot drift from the
/// struct the API writes.
pub const PDH_VALUE_SLOT_BYTES: usize = std::mem::size_of::<PdhFmtCounterValue>();

/// Interprets the bytes `PdhGetFormattedCounterValue` wrote into the slot.
///
/// Reads the measurement at **offset 8**, and only when `CStatus` says the value
/// is usable. A status code is never returned as data: that misread is the whole
/// of DBT-P41-002 mechanism 1, and §41.15 measured it producing `0` — i.e.
/// `PDH_CSTATUS_VALID_DATA` formatted as basis points — while the host's own
/// counters read 7.01%.
pub fn decode_pdh_value(slot: &[u8]) -> Option<u64> {
    let c_status = u32::from_le_bytes(slot.get(0..4)?.try_into().ok()?);
    if c_status != pdh::PDH_CSTATUS_VALID_DATA && c_status != pdh::PDH_CSTATUS_NEW_DATA {
        return None;
    }
    let large_value = i64::from_le_bytes(slot.get(8..16)?.try_into().ok()?);
    Some(large_value.max(0) as u64)
}

/// The same 16 bytes read as `doubleValue`, which shares offset 8 with
/// `largeValue` in the union.
pub fn decode_pdh_double(slot: &[u8]) -> Option<f64> {
    let c_status = u32::from_le_bytes(slot.get(0..4)?.try_into().ok()?);
    if c_status != pdh::PDH_CSTATUS_VALID_DATA && c_status != pdh::PDH_CSTATUS_NEW_DATA {
        return None;
    }
    let value = f64::from_le_bytes(slot.get(8..16)?.try_into().ok()?);
    value.is_finite().then_some(value)
}

/// Converts a PDH percentage counter to basis points.
///
/// **A separate defect from the offset**, and one the offset bug hid: the
/// collectors treated a `%` counter's value as basis points directly. §41.15
/// measured `\Processor(_Total)\% Processor Time` at **7.01**, which is 701 bp,
/// not 7 bp. Reading the right offset without this conversion under-reports CPU
/// by 100x — observed as 91 bp under a full-core spin before this was added.
pub fn percentage_to_bp(percent: f64) -> u32 {
    (percent.max(0.0) * 100.0).round().min(10_000.0) as u32
}

/// Pulls the instance name out of an expanded PDH counter path.
///
/// `PdhExpandWildCardPathW` returns **full counter paths**
/// (`\PhysicalDisk(0 C:)\% Disk Time`), not bare instance names. The shipping
/// code treated each returned string as an instance and re-wrapped it, which
/// would have produced `\PhysicalDisk(\PhysicalDisk(0 C:)\% Disk Time)\...`.
/// That third mechanism was never observed only because the expansion never
/// succeeded (§20.1.3(c)); it is fixed here rather than left to be discovered.
pub fn instance_from_counter_path(path: &str) -> Option<&str> {
    let open = path.find('(')?;
    let close = path.rfind(')')?;
    if close <= open + 1 {
        return None;
    }
    Some(&path[open + 1..close])
}

struct CounterHandle(isize);

impl CounterHandle {
    fn formatted(&self, format: u32) -> Option<[u8; PDH_VALUE_SLOT_BYTES]> {
        let mut value = PdhFmtCounterValue::default();
        unsafe {
            pdh_ok(pdh::PdhGetFormattedCounterValue(
                self.0,
                format,
                std::ptr::null_mut(),
                &mut value,
            ))
            .then(|| value.to_slot())
        }
    }

    /// Reads as an unsigned 64-bit raw-formatted value (counts, bytes, rates).
    fn read_u64(&self) -> Option<u64> {
        decode_pdh_value(&self.formatted(pdh::PDH_FMT_LARGE)?)
    }

    /// Reads a fractional counter (`%`, `sec/Transfer`) without truncating it.
    fn read_f64(&self) -> Option<f64> {
        decode_pdh_double(&self.formatted(pdh::PDH_FMT_DOUBLE)?)
    }

    /// Reads a percentage counter as basis points.
    fn read_percent_bp(&self) -> Option<u32> {
        self.read_f64().map(percentage_to_bp)
    }
}

/// Expands a wildcard counter path into the concrete paths PDH knows about.
///
/// Bound to a named `Vec` that outlives both calls, and NUL-terminated through
/// `chain(once(0))`. The shipping code built its pattern from a **dropped
/// temporary** and, being a raw string, its trailing `\0` was the two characters
/// backslash and zero rather than a NUL — DBT-P41-002 mechanism 2
/// (§20.1.3(c)). Both are fixed here, and every exit reports a reason.
pub fn expand_wildcard_path(pattern: &str) -> Result<Vec<String>, String> {
    let wide: Vec<u16> = pattern.encode_utf16().chain(std::iter::once(0)).collect();
    let search = windows::core::PCWSTR(wide.as_ptr());
    // `pcchPathListLength` is a character count, not a byte count.
    let mut needed = 0u32;
    let size_rc = unsafe {
        pdh::PdhExpandWildCardPathW(
            windows::core::PCWSTR::null(), // live data, not a log file
            search,
            windows::core::PWSTR::null(), // sizing call
            &mut needed,
            0, // expand both counters and instances
        )
    };
    // The sizing call is expected to report PDH_MORE_DATA; anything else that is
    // not success, or a zero size, is a failure with a reason.
    if !pdh_ok(size_rc) && size_rc != pdh::PDH_MORE_DATA {
        return Err(format!(
            "wildcard expansion sizing failed for {pattern} (rc=0x{size_rc:08x})"
        ));
    }
    if needed == 0 {
        return Err(format!(
            "wildcard expansion returned no buffer size for {pattern} (rc=0x{size_rc:08x})"
        ));
    }
    let mut buffer = vec![0u16; needed as usize];
    let rc = unsafe {
        pdh::PdhExpandWildCardPathW(
            windows::core::PCWSTR::null(),
            search,
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut needed,
            0,
        )
    };
    if !pdh_ok(rc) {
        return Err(format!(
            "wildcard expansion failed for {pattern} (rc=0x{rc:08x})"
        ));
    }
    // Walk the double-NUL-terminated list.
    let mut paths = Vec::new();
    let mut cursor = 0usize;
    while cursor < buffer.len() {
        let Some(end_rel) = buffer[cursor..].iter().position(|c| *c == 0) else {
            break;
        };
        let end = cursor + end_rel;
        if end == cursor {
            break; // the terminating empty string
        }
        paths.push(String::from_utf16_lossy(&buffer[cursor..end]));
        cursor = end + 1;
    }
    Ok(paths)
}

/// One passive CPU sampling pass via a per-tick PDH query.
///
/// A fresh query per tick costs microseconds and avoids cross-tick state entirely, which keeps
/// the engine restartable and deterministic under audit. Two `collect` calls are needed for
/// rate counters to produce a delta; both happen inside this single tick window.
fn sample_cpu(partial: &mut Vec<CollectorFault>, interval: Duration) -> Reading<CpuSample> {
    let _ = interval;
    let unavailable = |detail: &str, kind: &str| {
        Reading::unavailable(CollectorFault {
            collector: "cpu".into(),
            kind: kind.into(),
            detail: detail.into(),
        })
    };
    let Some(query) = QueryHandle::open() else {
        return unavailable("PDH query could not be opened", "Unavailable");
    };
    let total = query.add_english_counter(r"\Processor Information(_Total)\% Processor Time");
    let dpc = query.add_english_counter(r"\Processor Information(_Total)\% DPC Time");
    let isr = query.add_english_counter(r"\Processor Information(_Total)\% Interrupt Time");
    let ctx = query.add_english_counter(r"\System\Context Switches/sec");
    let queue = query.add_english_counter(r"\System\Processor Queue Length");
    if !query.collect() {
        return unavailable("first PDH collection failed", "ProviderFailure");
    }
    // Rate counters need a second observation inside the same tick.
    std::thread::sleep(Duration::from_millis(120).min(Duration::from_millis(100)));
    if !query.collect() {
        return unavailable("second PDH collection failed", "ProviderFailure");
    }
    let read_bp = |counter: &Option<CounterHandle>| -> Option<u32> {
        counter.as_ref().and_then(|c| c.read_percent_bp())
    };
    // The primary counter IS the evidence. `0` from a counter that read is a
    // legal measurement of an idle machine; `0` substituted for a counter that
    // did not read is the defect — so the two never share a representation here.
    let Some(total_busy_bp) = read_bp(&total) else {
        return unavailable(
            r"primary counter \Processor Information(_Total)\% Processor Time could not be read",
            "ProviderFailure",
        );
    };
    // Secondary counters degrade individually, and say which. Nothing is
    // silently defaulted to zero (§20.1.3(a): `.unwrap_or(0)` is how a failed
    // read became a confident measurement).
    let mut degraded: Vec<&str> = Vec::new();
    let dpc_bp = read_bp(&dpc).unwrap_or_else(|| {
        degraded.push("% DPC Time");
        0
    });
    let isr_bp = read_bp(&isr).unwrap_or_else(|| {
        degraded.push("% Interrupt Time");
        0
    });
    let context_switches_per_sec = ctx
        .as_ref()
        .and_then(|c| c.read_u64())
        .unwrap_or_else(|| {
            degraded.push("Context Switches/sec");
            0
        });
    let processor_queue_length_x100 = queue
        .as_ref()
        .and_then(|c| c.read_u64())
        .map(|v| v.saturating_mul(100))
        .unwrap_or_else(|| {
            degraded.push("Processor Queue Length");
            0
        });
    if !degraded.is_empty() {
        partial.push(CollectorFault {
            collector: "cpu.counters".into(),
            kind: "Degraded".into(),
            detail: format!("counters unreadable: {}", degraded.join(", ")),
        });
    }
    Reading::from_evidence(
        Some(CpuSample {
            // Per-processor breakdown has never been written on Windows
            // (§20.1.3(b)); the aggregate is what this provider measures.
            per_processor_busy_bp: Vec::new(),
            total_busy_bp,
            dpc_isr_busy_bp: dpc_bp.saturating_add(isr_bp).min(10_000),
            context_switches_per_sec,
            processor_queue_length_x100,
        }),
        || unreachable_fault("cpu"),
    )
}

/// `Reading::from_evidence` needs a fault for the `None` arm; where the caller
/// has already proven the `Some`, this documents that the arm is unreachable
/// rather than inventing a plausible-looking reason for it.
fn unreachable_fault(collector: &str) -> CollectorFault {
    CollectorFault {
        collector: collector.into(),
        kind: "Internal".into(),
        detail: "evidence was present; this fault is unreachable".into(),
    }
}

/// CallNtPowerInformation(ProcessorInformation) — thermal + power-limit evidence without WMI.
fn sample_power(partial: &mut Vec<CollectorFault>) -> Reading<PowerSample> {
    let mut sample = PowerSample::default();
    // PROCESSOR_POWER_INFORMATION is variable by count; start conservative and cap the buffer.
    let max_processors: u32 = super::MAX_CPU_COUNT as u32;
    let entry_size = std::mem::size_of::<PROCESSOR_POWER_INFORMATION>();
    let mut buffer_len = max_processors as usize * entry_size;
    let mut buffer: Vec<u8> = Vec::new();
    let mut queried = false;
    loop {
        buffer.resize(buffer_len, 0);
        let status = unsafe {
            CallNtPowerInformation(
                POWER_INFORMATION_LEVEL(11), // ProcessorInformation
                None,
                0,
                Some(buffer.as_mut_ptr().cast()),
                buffer.len() as u32,
            )
        };
        if status.is_ok() {
            queried = true;
            break;
        }
        if buffer_len >= max_processors as usize * entry_size {
            break;
        }
        buffer_len = max_processors as usize * entry_size;
    }
    // §20.1.1 site 3: this condition shipped as `!buffer.iter().all(..) || true`,
    // which is unconditionally true, so the `else` below — and with it the power
    // `Unavailable` fault — was dead code. The availability decision now reads
    // the actual outcome of `CallNtPowerInformation`.
    if queried && !buffer.iter().all(|byte| *byte == 0) {
        // Parse the current MHz fields to detect clamping relative to max MHz when available.
        // Fields after MaxMhz/CurrentMhz are informational only; we never fabricate values.
        let stride = entry_size;
        if stride > 0 && buffer.len() >= stride {
            // Layout (x64): Number(u32), MaxMhz(u32), CurrentMhz(u32), MhzLimit(u32),
            // MaxIdleState(u32), CurrentIdleState(u32), IdleTime(u64), ... We read conservatively
            // within the first entry only; multi-packet aggregation stays product debt.
            let read_u32 = |offset: usize| -> Option<u32> {
                if offset + 4 <= buffer.len() {
                    Some(u32::from_le_bytes(
                        buffer[offset..offset + 4].try_into().ok()?,
                    ))
                } else {
                    None
                }
            };
            let max_mhz = read_u32(4);
            let current_mhz = read_u32(8);
            let mhz_limit = read_u32(12);
            if let (Some(max), Some(current)) = (max_mhz, current_mhz) {
                if max > 0 && current > 0 {
                    let ratio_bp =
                        ((u64::from(current) * 10_000) / u64::from(max)).min(10_000) as u32;
                    // A sustained ratio materially below max indicates a clamp in effect.
                    if ratio_bp < 8_500 {
                        sample.throttle_active = true;
                        // Without vendor MSRs we cannot distinguish VRM vs package power here;
                        // report the conservative generic power clamp and surface raw evidence.
                        sample.throttle_reason = ThermalThrottleReason::Power;
                    }
                }
            }
            if let Some(limit) = mhz_limit {
                if limit < 100 {
                    sample.throttle_active = true;
                    if sample.throttle_reason == ThermalThrottleReason::Unspecified {
                        sample.throttle_reason = ThermalThrottleReason::Thermal;
                    }
                }
            }
        }
    } else {
        return Reading::unavailable(CollectorFault {
            collector: "power".into(),
            kind: "Unavailable".into(),
            detail: if queried {
                "CallNtPowerInformation(ProcessorInformation) returned an all-zero buffer".into()
            } else {
                "CallNtPowerInformation(ProcessorInformation) failed".into()
            },
        });
    }
    sample.throttle_reason = match sample.throttle_reason {
        ThermalThrottleReason::Unspecified if !sample.throttle_active => {
            ThermalThrottleReason::None
        }
        other => other,
    };
    // DBT-P45-003 (found in this session's own sweep, not in the brief's
    // starter list): no line of this file has ever written `has_temperature`
    // or `temperature_c` — `CallNtPowerInformation(ProcessorInformation)`
    // exposes clock/throttle state, not raw temperature, and no other source
    // is read here. `power` was otherwise reported fully measured (no fault),
    // so a consumer could not tell "genuinely no thermal event" from "this
    // platform never measures temperature" — the exact §44 1.A cpu.counters /
    // storage.rates shape, in a sub-field the brief's starter list missed.
    partial.push(CollectorFault {
        collector: "power.temperature".into(),
        kind: "Degraded".into(),
        detail: "raw temperature not measured on Windows: CallNtPowerInformation exposes \
                 clock/throttle state only"
            .into(),
    });
    Reading::from_evidence(Some(sample), || unreachable_fault("power"))
}

fn sample_memory(partial: &mut Vec<CollectorFault>) -> Reading<MemorySample> {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut sample = MemorySample::default();
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // The headline fields come from GlobalMemoryStatusEx, not PDH — which is why
    // §41.15 saw real memory numbers beside all-zero PDH ones in the same object.
    let headline_ok = unsafe { GlobalMemoryStatusEx(&mut status).is_ok() };
    if !headline_ok {
        return Reading::unavailable(CollectorFault {
            collector: "memory".into(),
            kind: "Unavailable".into(),
            detail: "GlobalMemoryStatusEx failed".into(),
        });
    }
    sample.total_physical_bytes = status.ullTotalPhys;
    sample.available_physical_bytes = status.ullAvailPhys;
    sample.commit_bytes = status
        .ullTotalPageFile
        .saturating_sub(status.ullAvailPageFile);
    sample.commit_limit_bytes = status.ullTotalPageFile;
    sample.memory_load_percent = status.dwMemoryLoad.min(100);

    // Standby cache / page-fault dynamics come from perf OS counters when present.
    // These are a degraded *part* of a measured subsystem, so they report under
    // the dotted `memory.counters` name rather than marking memory unavailable.
    match QueryHandle::open() {
        None => partial.push(CollectorFault {
            collector: "memory.counters".into(),
            kind: "Unavailable".into(),
            detail: "PDH query could not be opened for memory counters".into(),
        }),
        Some(query) => {
            let standby = query.add_english_counter(r"\Memory\Standby Cache Normal Priority Bytes");
            let standby_reserve =
                query.add_english_counter(r"\Memory\Standby Cache Reserve Priority Bytes");
            let modified = query.add_english_counter(r"\Memory\Modified Page List Bytes");
            let hard = query.add_english_counter(r"\Memory\Pages Input/sec");
            let soft = query.add_english_counter(r"\Memory\Pages Output/sec");
            // `Pages Input/sec` and `Pages Output/sec` are rate counters and have
            // no value after one collection.
            if !query.collect_twice(Duration::from_millis(80)) {
                partial.push(CollectorFault {
                    collector: "memory.counters".into(),
                    kind: "ProviderFailure".into(),
                    detail: "PDH collection failed for memory counters".into(),
                });
            } else {
                let mut degraded: Vec<&str> = Vec::new();
                let mut read = |counter: &Option<CounterHandle>, name: &'static str| -> u64 {
                    match counter.as_ref().and_then(|c| c.read_u64()) {
                        Some(value) => value,
                        None => {
                            degraded.push(name);
                            0
                        }
                    }
                };
                sample.standby_cache_bytes = read(&standby, "Standby Cache Normal Priority Bytes")
                    .saturating_add(read(
                        &standby_reserve,
                        "Standby Cache Reserve Priority Bytes",
                    ));
                sample.modified_page_list_bytes = read(&modified, "Modified Page List Bytes");
                sample.hard_faults_per_sec = read(&hard, "Pages Input/sec");
                sample.soft_faults_per_sec = read(&soft, "Pages Output/sec");
                if !degraded.is_empty() {
                    partial.push(CollectorFault {
                        collector: "memory.counters".into(),
                        kind: "Degraded".into(),
                        detail: format!("counters unreadable: {}", degraded.join(", ")),
                    });
                }
            }
        }
    }
    Reading::from_evidence(Some(sample), || unreachable_fault("memory"))
}

fn sample_storage(partial: &mut Vec<CollectorFault>) -> Reading<Vec<StorageQueueSample>> {
    let unavailable = |kind: &str, detail: String| {
        Reading::unavailable(CollectorFault {
            collector: "storage".into(),
            kind: kind.into(),
            detail,
        })
    };
    let Some(query) = QueryHandle::open() else {
        return unavailable("Unavailable", "PDH query could not be opened".into());
    };
    // §20.1.2: the four exits below returned an empty Vec with no fault pushed.
    // Every one of them now reports its reason.
    // Enumerate physical disk instances through the wildcard expansion API.
    let expanded = match expand_wildcard_path(r"\PhysicalDisk(*)\% Disk Time") {
        Ok(paths) => paths,
        Err(detail) => return unavailable("ProviderFailure", detail),
    };
    // Counters must be added BEFORE the collections that give them data: a
    // handle added after the last `collect` has nothing to format, and rate
    // counters need two observations. The shipping code added each counter and
    // read it immediately, so even had the expansion succeeded these reads would
    // have produced nothing. Add every counter, then collect twice, then read.
    struct Pending {
        instance: String,
        active: Option<CounterHandle>,
        queue: Option<CounterHandle>,
        latency: Option<CounterHandle>,
        read: Option<CounterHandle>,
        write: Option<CounterHandle>,
    }
    let mut pending: Vec<Pending> = Vec::new();
    let expanded_count = expanded.len();
    let mut skipped_total = 0usize;
    for path in &expanded {
        if pending.len() >= super::MAX_STORAGE_DEVICES {
            break;
        }
        let Some(instance) = instance_from_counter_path(path) else {
            continue;
        };
        // `_Total` is PDH's aggregate across the real instances, not a device.
        if instance.eq_ignore_ascii_case("_total") {
            skipped_total += 1;
            continue;
        }
        let instance = instance.to_string();
        pending.push(Pending {
            active: query.add_english_counter(&format!(
                r"\PhysicalDisk({instance})\% Disk Time"
            )),
            queue: query.add_english_counter(&format!(
                r"\PhysicalDisk({instance})\Current Disk Queue Length"
            )),
            latency: query.add_english_counter(&format!(
                r"\PhysicalDisk({instance})\Avg. Disk sec/Transfer"
            )),
            read: query.add_english_counter(&format!(
                r"\PhysicalDisk({instance})\Disk Read Bytes/sec"
            )),
            write: query.add_english_counter(&format!(
                r"\PhysicalDisk({instance})\Disk Write Bytes/sec"
            )),
            instance,
        });
    }
    if pending.is_empty() {
        return unavailable(
            "Unavailable",
            format!(
                "wildcard expansion returned {expanded_count} path(s), \
                 {skipped_total} aggregate, 0 usable PhysicalDisk instances"
            ),
        );
    }
    if !query.collect_twice(Duration::from_millis(80)) {
        return unavailable(
            "ProviderFailure",
            "PDH collection failed after adding PhysicalDisk counters".into(),
        );
    }
    // DBT-P45-002: the four secondary counters below used to fall back to
    // `.unwrap_or(0)` with nothing recorded — the exact §20.1.3(a) shape
    // this file's own comment at :386 names. Not a stale comment describing
    // fixed code: `git blame cc9c51e` shows :386's comment and this
    // function's four `.unwrap_or(0)` calls were written in the SAME commit
    // (P42, DBT-P41-002) — the commit that introduced the disciplined
    // `degraded.push()` pattern for `cpu`'s and `memory`'s secondary
    // counters left `storage`'s newly-rewritten secondary counters on the
    // old, un-degraded `.unwrap_or(0)` shape, three functions away from its
    // own stated principle. `active_time_bp` is this device's primary
    // evidence and is gated by `?` above; the four secondary counters now
    // get the same `degraded.push()` discipline `cpu`/`memory` already had.
    let mut devices_with_missing_secondary: Vec<String> = Vec::new();
    let devices: Vec<StorageQueueSample> = pending
        .into_iter()
        .filter_map(|device| {
            // The active-time counter is this device's evidence. A device whose
            // primary counter did not read is dropped rather than published as a
            // confident zero.
            let active_time_bp = device.active.as_ref().and_then(|c| c.read_percent_bp())?;
            let read_or_zero =
                |counter: &Option<CounterHandle>| counter.as_ref().and_then(|c| c.read_u64());
            let mut missing: Vec<&str> = Vec::new();
            let queue_depth_x100 = read_or_zero(&device.queue)
                .map(|value| value.saturating_mul(100))
                .unwrap_or_else(|| {
                    missing.push("Current Disk Queue Length");
                    0
                });
            // `Avg. Disk sec/Transfer` is fractional seconds; LARGE truncated
            // every sub-second latency to 0.
            let avg_transfer_latency_us = device
                .latency
                .as_ref()
                .and_then(|c| c.read_f64())
                .map(|seconds| (seconds.max(0.0) * 1_000_000.0).round() as u64)
                .unwrap_or_else(|| {
                    missing.push("Avg. Disk sec/Transfer");
                    0
                });
            let read_bytes_per_sec = read_or_zero(&device.read).unwrap_or_else(|| {
                missing.push("Disk Read Bytes/sec");
                0
            });
            let write_bytes_per_sec = read_or_zero(&device.write).unwrap_or_else(|| {
                missing.push("Disk Write Bytes/sec");
                0
            });
            if !missing.is_empty() {
                devices_with_missing_secondary.push(format!(
                    "{}: {}",
                    device.instance,
                    missing.join(", ")
                ));
            }
            let (total_space_bytes, free_space_bytes) = query_disk_space(&device.instance);
            Some(StorageQueueSample {
                device_id: format!("physicaldisk:{}", device.instance),
                active_time_bp,
                queue_depth_x100,
                avg_transfer_latency_us,
                read_bytes_per_sec,
                write_bytes_per_sec,
                friendly_name: device.instance,
                total_space_bytes,
                free_space_bytes,
            })
        })
        .collect();
    if !devices_with_missing_secondary.is_empty() {
        partial.push(CollectorFault {
            collector: "storage.rates".into(),
            kind: "Degraded".into(),
            detail: format!(
                "secondary PDH counters unreadable this tick: {}",
                devices_with_missing_secondary.join("; ")
            ),
        });
    }
    Reading::from_collection(devices, || CollectorFault {
        collector: "storage".into(),
        kind: "Unavailable".into(),
        detail: format!(
            "{expanded_count} PhysicalDisk path(s) expanded but none produced a readable \
             % Disk Time counter"
        ),
    })
}

fn query_disk_space(instance: &str) -> (u64, u64) {
    // If instance contains a drive letter (e.g., "0 C:" or "C:"), construct "X:\"
    let drive_path = instance.split_whitespace().find_map(|part| {
        let part = part.trim();
        if part.len() == 2 && part.ends_with(':') && part.chars().next().map_or(false, |c| c.is_ascii_alphabetic()) {
            Some(format!("{}\\", part))
        } else {
            None
        }
    });

    let mut free_bytes = 0u64;
    let mut total_bytes = 0u64;
    let mut total_free = 0u64;

    let res = if let Some(path) = drive_path {
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
                windows::core::PCWSTR(wide.as_ptr()),
                Some(&mut free_bytes),
                Some(&mut total_bytes),
                Some(&mut total_free),
            )
        }
    } else {
        unsafe {
            windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
                windows::core::PCWSTR::null(),
                Some(&mut free_bytes),
                Some(&mut total_bytes),
                Some(&mut total_free),
            )
        }
    };

    if res.is_ok() {
        (total_bytes, free_bytes)
    } else {
        (0, 0)
    }
}

fn sample_gpu() -> Reading<GpuSample> {
    // GPU engine enumeration requires DXGI adapter traversal; the passive PDH fallback reads the
    // "GPU Engine" utilization instances the adapter exposes. Adapter identity and VRAM
    // budget come from the existing hardware-telemetry inventory where available.
    let unavailable = |kind: &str, detail: String| {
        Reading::unavailable(CollectorFault {
            collector: "gpu".into(),
            kind: kind.into(),
            detail,
        })
    };
    let Some(query) = QueryHandle::open() else {
        // §20.1.1 site 6 / §20.1.2: this exit returned a default GpuSample with
        // no fault at all — gpu's famous self-check sat one branch further down.
        return unavailable("Unavailable", "PDH query could not be opened".into());
    };
    // DBT-P41-002a. §20.1.6 predicted and §41.15 measured the cause: PdhAddEnglishCounterW
    // refuses an *unexpanded* wildcard, so the single `add_english_counter` call
    // for `\GPU Engine(*engtype_3D)\Utilization Percentage` returned None, engines
    // stayed empty, and the collector reported "no GPU engine counters exposed by
    // this adapter/driver" on a box exposing **568** of them across two healthy
    // adapters (§41.16 3b). Expand first, then add the concrete paths.
    let expanded = match expand_wildcard_path(r"\GPU Engine(*engtype_3D)\Utilization Percentage") {
        Ok(paths) => paths,
        Err(detail) => return unavailable("ProviderFailure", detail),
    };
    let expanded_count = expanded.len();
    let pending: Vec<(String, CounterHandle)> = expanded
        .iter()
        .take(super::MAX_GPU_ENGINES)
        .filter_map(|path| {
            let name = instance_from_counter_path(path)
                .map(str::to_string)
                .unwrap_or_else(|| "3D".to_string());
            query.add_english_counter(path).map(|counter| (name, counter))
        })
        .collect();
    if pending.is_empty() {
        return unavailable(
            "Unavailable",
            format!(
                "no GPU Engine 3D utilization counters on this host \
                 (wildcard expanded to {expanded_count} path(s), none addable)"
            ),
        );
    }
    // DBT-P42-010, VRAM half. `\GPU Adapter Memory(*)\Dedicated Usage` and
    // `\Shared Usage` are ordinary PDH counters, so they ride on the query that
    // is already open and inside the `collect_twice` already being paid for:
    // no DXGI, no COM in a session-0 LocalSystem service, no extra sleep, and
    // the sampling cadence is unchanged. That is the whole reason this half is
    // implemented and the capacity half is not -- `dedicated_total_bytes` has
    // no PDH counter, and the alternatives (DXGI DedicatedVideoMemory, or WMI
    // AdapterRAM, whose 32-bit field wraps above 4 GB) would either add a real
    // observer or report a confidently wrong number where 0 is honest today.
    //
    // Instances are per adapter, not per process (`GPU Process Memory` is the
    // per-process counter set), so summing them answers "VRAM in use on this
    // machine" -- the same whole-machine aggregation `engines` already applies.
    let memory_counters = |suffix: &str| -> Vec<CounterHandle> {
        expand_wildcard_path(&format!(r"\GPU Adapter Memory(*)\{suffix}"))
            .unwrap_or_default()
            .iter()
            .take(super::MAX_GPU_ENGINES)
            .filter_map(|path| query.add_english_counter(path))
            .collect()
    };
    let dedicated_pending = memory_counters("Dedicated Usage");
    let shared_pending = memory_counters("Shared Usage");

    if !query.collect_twice(Duration::from_millis(80)) {
        return unavailable(
            "ProviderFailure",
            "PDH collection failed after adding GPU Engine counters".into(),
        );
    }
    // An adapter whose counter will not read is left out of the sum rather than
    // counted as zero; if none read, the total stays the honest 0 it was before.
    let sum_bytes = |counters: Vec<CounterHandle>| -> u64 {
        counters
            .iter()
            .filter_map(CounterHandle::read_u64)
            .fold(0u64, u64::saturating_add)
    };
    let dedicated_used_bytes = sum_bytes(dedicated_pending);
    let shared_used_bytes = sum_bytes(shared_pending);
    let engines: Vec<GpuEngineSample> = pending
        .into_iter()
        .filter_map(|(engine_name, counter)| {
            counter.read_percent_bp().map(|utilization_bp| GpuEngineSample {
                engine_name,
                utilization_bp,
            })
        })
        .collect();
    Reading::from_collection(
        engines,
        || CollectorFault {
            collector: "gpu".into(),
            kind: "Unavailable".into(),
            detail: format!(
                "{expanded_count} GPU Engine path(s) expanded but none produced a readable \
                 utilization counter"
            ),
        },
    )
    .into_gpu_sample(dedicated_used_bytes, shared_used_bytes)
}

/// Wraps the engine list back into the wire payload without losing the reading's
/// measured-or-not decision.
trait IntoGpuSample {
    fn into_gpu_sample(self, dedicated_used_bytes: u64, shared_used_bytes: u64) -> Reading<GpuSample>;
}

impl IntoGpuSample for Reading<Vec<GpuEngineSample>> {
    fn into_gpu_sample(self, dedicated_used_bytes: u64, shared_used_bytes: u64) -> Reading<GpuSample> {
        let (engines, fault) = self.into_parts(Vec::new);
        match fault {
            Some(fault) => Reading::unavailable(fault),
            None => Reading::from_evidence(
                Some(GpuSample {
                    engines,
                    dedicated_used_bytes,
                    shared_used_bytes,
                    // adapter_id, adapter_name and dedicated_total_bytes stay
                    // empty on purpose -- DBT-P42-010's capacity/identity half
                    // is recorded as out of scope in §47, not silently zeroed.
                    ..GpuSample::default()
                }),
                || unreachable_fault("gpu"),
            ),
        }
    }
}

fn sample_process_top() -> Reading<Vec<ProcessCpuTopEntry>> {
    // §20.1.1 site 7: this returned `Vec::new()` unconditionally after `let _ = faults;`
    // — success, empty payload, no fault, by design. The design is sound; the
    // silence is not. A full per-PID PDH walk is deliberately avoided on the hot
    // path because its cost scales with process count (observer effect), so
    // top-offender ranking arrives from the bottleneck analyzer over ring deltas.
    // That is now stated to the caller instead of being left to be inferred from
    // an empty array.
    Reading::unavailable(CollectorFault {
        collector: "processTop".into(),
        kind: "NotCollected".into(),
        detail: "per-process CPU attribution is produced by the bottleneck analyzer over ring \
                 deltas; this sampler does not walk per-PID PDH (observer effect)"
            .into(),
    })
}

pub struct WindowsPerfPlatform;

impl PerfPlatform for WindowsPerfPlatform {
    fn sample(&self, interval: Duration) -> PerfSnapshot {
        // Faults for degraded *parts* of measured subsystems (dotted collector
        // names). Whole-subsystem availability is carried by the `Reading`s and
        // published by `into_snapshot`, which is the only path to a snapshot.
        let mut partial: Vec<CollectorFault> = Vec::new();
        let cpu = sample_cpu(&mut partial, interval);
        let power = sample_power(&mut partial);
        let memory = sample_memory(&mut partial);
        let storage = sample_storage(&mut partial);
        let gpu = sample_gpu();
        let process_top = sample_process_top();
        let mut snapshot = CollectedSubsystems {
            cpu,
            power,
            memory,
            storage,
            gpu,
            process_top,
        }
        .into_snapshot(interval);
        snapshot.collector_faults.extend(partial);
        snapshot.normalized()
    }
}
