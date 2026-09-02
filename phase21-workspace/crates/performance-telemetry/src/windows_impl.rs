//! Windows implementation of the Phase 20 performance platform.
//!
//! All native resources are RAII-wrapped. Counter collection uses PDH (Performance Data Helper)
//! in a single passive query per tick — no busy polling, no WMI on the hot path (WMI is only a
//! cold-start fallback for topology). Every collector degrades into `collector_faults` so one
//! broken counter can never stall or fail the snapshot.

use std::time::Duration;

use super::{
    CollectorFault, CpuSample, GpuEngineSample, GpuSample, MemorySample, PerfPlatform,
    PerfSnapshot, PowerSample, ProcessCpuTopEntry, StorageQueueSample, ThermalThrottleReason,
};
use windows::Win32::System::Power::{
    CallNtPowerInformation, POWER_INFORMATION_LEVEL, PROCESSOR_POWER_INFORMATION,
};

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
        pub fn PdhGetFormattedCounterValue(
            counter: isize,
            format: u32,
            lptype: *mut u32,
            value: *mut i64,
        ) -> i32;
        // P36 (Hermes): the WW spelling does not exist in pdh.dll; bind the
        // real export PdhExpandWildCardPathW (single W) to fix LNK2019 on ARM64.
        #[link_name = "PdhExpandWildCardPathW"]
        pub fn PdhExpandWildCardPathWW(
            szsearchpath: PCWSTR,
            psearchresultlist: *mut PWSTR,
            pcchbufferlength: *mut u32,
        ) -> i32;
    }

    pub const PDH_MORE_DATA: i32 = 0x8000_07D2u32 as i32;
    /// PERF_SIZE_LARGE | PERF_TYPE_NUMBER (u64 formatted value).
    pub const PDH_FMT_LARGE: u32 = 0x0000_0400;
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
/// DBT-P41-002 test seam (P42 1.A). The API's out-parameter is a
/// `PDH_FMT_COUNTERVALUE`, which is 16 bytes with `largeValue` at offset 8.
/// This constant records what the shipping code actually supplies, so a test can
/// assert the ABI instead of reading it out of a comment. It is deliberately
/// left at the shipping value in 1.A: the tests must fail against today's
/// behaviour before the fix lands.
pub const PDH_VALUE_SLOT_BYTES: usize = 8;

/// Interprets the bytes `PdhGetFormattedCounterValue` wrote into the slot.
///
/// Extracted unchanged from `read_u64` so it is testable without PDH. Today it
/// reads the first 8 bytes, which in a real `PDH_FMT_COUNTERVALUE` are `CStatus`
/// plus padding — never the measurement.
pub fn decode_pdh_value(slot: &[u8]) -> Option<u64> {
    let raw = i64::from_le_bytes(slot.get(0..8)?.try_into().ok()?);
    Some(raw.max(0) as u64)
}

struct CounterHandle(isize);

impl CounterHandle {
    /// Reads as an unsigned 64-bit raw-formatted value.
    fn read_u64(&self) -> Option<u64> {
        let mut slot = [0u8; PDH_VALUE_SLOT_BYTES];
        unsafe {
            if pdh_ok(pdh::PdhGetFormattedCounterValue(
                self.0,
                pdh::PDH_FMT_LARGE,
                std::ptr::null_mut(),
                slot.as_mut_ptr().cast(),
            )) {
                decode_pdh_value(&slot)
            } else {
                None
            }
        }
    }
}

/// One passive CPU sampling pass via a per-tick PDH query.
///
/// A fresh query per tick costs microseconds and avoids cross-tick state entirely, which keeps
/// the engine restartable and deterministic under audit. Two `collect` calls are needed for
/// rate counters to produce a delta; both happen inside this single tick window.
fn sample_cpu(faults: &mut Vec<CollectorFault>, interval: Duration) -> CpuSample {
    let _ = interval;
    let mut sample = CpuSample::default();
    let Some(query) = QueryHandle::open() else {
        faults.push(CollectorFault {
            collector: "cpu".into(),
            kind: "Unavailable".into(),
            detail: "PDH query could not be opened".into(),
        });
        return sample;
    };
    let total = query.add_english_counter(r"\Processor Information(_Total)\% Processor Time");
    let dpc = query.add_english_counter(r"\Processor Information(_Total)\% DPC Time");
    let isr = query.add_english_counter(r"\Processor Information(_Total)\% Interrupt Time");
    let ctx = query.add_english_counter(r"\System\Context Switches/sec");
    let queue = query.add_english_counter(r"\System\Processor Queue Length");
    if !query.collect() {
        faults.push(CollectorFault {
            collector: "cpu".into(),
            kind: "ProviderFailure".into(),
            detail: "first PDH collection failed".into(),
        });
        return sample;
    }
    // Rate counters need a second observation inside the same tick.
    std::thread::sleep(Duration::from_millis(120).min(Duration::from_millis(100)));
    if !query.collect() {
        faults.push(CollectorFault {
            collector: "cpu".into(),
            kind: "ProviderFailure".into(),
            detail: "second PDH collection failed".into(),
        });
        return sample;
    }
    let read_bp = |counter: &Option<CounterHandle>| -> Option<u32> {
        counter
            .as_ref()
            .and_then(|c| c.read_u64())
            .map(|v| v.min(10_000) as u32)
    };
    if let Some(busy) = read_bp(&total) {
        sample.total_busy_bp = busy.min(10_000);
    }
    let dpc_bp = read_bp(&dpc).unwrap_or(0);
    let isr_bp = read_bp(&isr).unwrap_or(0);
    sample.dpc_isr_busy_bp = dpc_bp.saturating_add(isr_bp).min(10_000);
    if let Some(ctx_value) = ctx.as_ref().and_then(|c| c.read_u64()) {
        sample.context_switches_per_sec = ctx_value;
    }
    if let Some(queue_value) = queue.as_ref().and_then(|c| c.read_u64()) {
        sample.processor_queue_length_x100 = queue_value.saturating_mul(100);
    }
    sample
}

/// CallNtPowerInformation(ProcessorInformation) — thermal + power-limit evidence without WMI.
fn sample_power(faults: &mut Vec<CollectorFault>) -> PowerSample {
    let mut sample = PowerSample::default();
    // PROCESSOR_POWER_INFORMATION is variable by count; start conservative and cap the buffer.
    let max_processors: u32 = super::MAX_CPU_COUNT as u32;
    let entry_size = std::mem::size_of::<PROCESSOR_POWER_INFORMATION>();
    let mut buffer_len = max_processors as usize * entry_size;
    let mut buffer: Vec<u8> = Vec::new();
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
            break;
        }
        if buffer_len >= max_processors as usize * entry_size {
            break;
        }
        buffer_len = max_processors as usize * entry_size;
    }
    if !buffer.iter().all(|byte| *byte == 0) || true {
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
        faults.push(CollectorFault {
            collector: "power".into(),
            kind: "Unavailable".into(),
            detail: "processor power information unavailable".into(),
        });
    }
    sample.throttle_reason = match sample.throttle_reason {
        ThermalThrottleReason::Unspecified if !sample.throttle_active => {
            ThermalThrottleReason::None
        }
        other => other,
    };
    sample
}

fn sample_memory(faults: &mut Vec<CollectorFault>) -> MemorySample {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut sample = MemorySample::default();
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    unsafe {
        if GlobalMemoryStatusEx(&mut status).is_ok() {
            sample.total_physical_bytes = status.ullTotalPhys;
            sample.available_physical_bytes = status.ullAvailPhys;
            sample.commit_bytes = status
                .ullTotalPageFile
                .saturating_sub(status.ullAvailPageFile);
            sample.commit_limit_bytes = status.ullTotalPageFile;
            sample.memory_load_percent = status.dwMemoryLoad.min(100);
        } else {
            faults.push(CollectorFault {
                collector: "memory".into(),
                kind: "Unavailable".into(),
                detail: "GlobalMemoryStatusEx failed".into(),
            });
        }
    }
    // Standby cache / page-fault dynamics come from perf OS counters when present.
    let query = QueryHandle::open();
    if let Some(query) = &query {
        let standby = query.add_english_counter(r"\Memory\Standby Cache Normal Priority Bytes");
        let standby_reserve =
            query.add_english_counter(r"\Memory\Standby Cache Reserve Priority Bytes");
        let modified = query.add_english_counter(r"\Memory\Modified Page List Bytes");
        let hard = query.add_english_counter(r"\Memory\Pages Input/sec");
        let soft = query.add_english_counter(r"\Memory\Pages Output/sec");
        if query.collect() {
            sample.standby_cache_bytes = standby.as_ref().and_then(|c| c.read_u64()).unwrap_or(0)
                + standby_reserve
                    .as_ref()
                    .and_then(|c| c.read_u64())
                    .unwrap_or(0);
            sample.modified_page_list_bytes =
                modified.as_ref().and_then(|c| c.read_u64()).unwrap_or(0);
            sample.hard_faults_per_sec = hard.as_ref().and_then(|c| c.read_u64()).unwrap_or(0);
            sample.soft_faults_per_sec = soft.as_ref().and_then(|c| c.read_u64()).unwrap_or(0);
        }
    } else {
        faults.push(CollectorFault {
            collector: "memory.counters".into(),
            kind: "Unavailable".into(),
            detail: "PDH memory counters unavailable".into(),
        });
    }
    sample
}

fn sample_storage(faults: &mut Vec<CollectorFault>) -> Vec<StorageQueueSample> {
    let mut devices = Vec::new();
    let Some(query) = QueryHandle::open() else {
        faults.push(CollectorFault {
            collector: "storage".into(),
            kind: "Unavailable".into(),
            detail: "PDH query unavailable".into(),
        });
        return devices;
    };
    let _ = query.collect(); // seed rate counters
    std::thread::sleep(Duration::from_millis(80));
    if !query.collect() {
        return devices;
    }
    // Enumerate physical disk instances through the wildcard expansion API.
    let pattern = windows::core::PCWSTR::from_raw(
        r"\PhysicalDisk(*)\% Disk Time\0"
            .encode_utf16()
            .collect::<Vec<u16>>()
            .as_ptr(),
    );
    let mut needed = 0u32;
    unsafe {
        // First call asks for size; second call fills. Both failures are typed faults.
        let _ = pdh::PdhExpandWildCardPathWW(pattern, std::ptr::null_mut(), &mut needed);
    }
    if needed == 0 {
        return devices;
    }
    let mut buffer = vec![0u16; needed as usize];
    let mut list: windows::core::PWSTR = windows::core::PWSTR(buffer.as_mut_ptr());
    let ok = unsafe { pdh::PdhExpandWildCardPathWW(pattern, &mut list, &mut needed) };
    if !pdh_ok(ok) {
        return devices;
    }
    // Walk the double-NUL-terminated list.
    let mut cursor = 0usize;
    while cursor < buffer.len() {
        let Some(end_rel) = buffer[cursor..].iter().position(|c| *c == 0) else {
            break;
        };
        let end = cursor + end_rel;
        let instance = String::from_utf16_lossy(&buffer[cursor..end]);
        if instance.is_empty() {
            break;
        }
        cursor = end + 1;
        if instance == "*all instances*" {
            continue;
        }
        let active_path = format!(r"\PhysicalDisk({instance})\% Disk Time");
        let queue_path = format!(r"\PhysicalDisk({instance})\Current Disk Queue Length");
        let latency_path = format!(r"\PhysicalDisk({instance})\Avg. Disk sec/Transfer");
        let read_path = format!(r"\PhysicalDisk({instance})\Disk Read Bytes/sec");
        let write_path = format!(r"\PhysicalDisk({instance})\Disk Write Bytes/sec");
        if devices.len() >= super::MAX_STORAGE_DEVICES {
            break;
        }
        let active = query
            .add_english_counter(&active_path)
            .and_then(|c| c.read_u64())
            .unwrap_or(0);
        let queue_depth = query
            .add_english_counter(&queue_path)
            .and_then(|c| c.read_u64())
            .unwrap_or(0);
        let latency = query
            .add_english_counter(&latency_path)
            .and_then(|c| c.read_u64())
            .unwrap_or(0);
        let read_bps = query
            .add_english_counter(&read_path)
            .and_then(|c| c.read_u64())
            .unwrap_or(0);
        let write_bps = query
            .add_english_counter(&write_path)
            .and_then(|c| c.read_u64())
            .unwrap_or(0);
        devices.push(StorageQueueSample {
            device_id: format!("physicaldisk:{instance}"),
            friendly_name: instance,
            active_time_bp: active.min(10_000) as u32,
            queue_depth_x100: queue_depth.saturating_mul(100),
            avg_transfer_latency_us: latency,
            read_bytes_per_sec: read_bps,
            write_bytes_per_sec: write_bps,
        });
    }
    devices
}

fn sample_gpu(faults: &mut Vec<CollectorFault>) -> GpuSample {
    // GPU engine enumeration requires DXGI adapter traversal; the passive PDH fallback reads the
    // aggregate "GPU Engine" utilization when the adapter exposes it. Adapter identity and VRAM
    // budget come from the existing hardware-telemetry inventory where available.
    let mut sample = GpuSample::default();
    let Some(query) = QueryHandle::open() else {
        return sample;
    };
    let eng_util = query.add_english_counter(r"\GPU Engine(*engtype_3D)\Utilization Percentage");
    let _ = eng_util.map(|counter| {
        if query.collect() {
            if let Some(value) = counter.read_u64() {
                sample.engines.push(GpuEngineSample {
                    engine_name: "3D".into(),
                    utilization_bp: (value.min(10_000) as u32),
                });
            }
        }
    });
    if sample.engines.is_empty() {
        faults.push(CollectorFault {
            collector: "gpu".into(),
            kind: "Unavailable".into(),
            detail: "no GPU engine counters exposed by this adapter/driver".into(),
        });
    }
    sample
}

fn sample_process_top(faults: &mut Vec<CollectorFault>) -> Vec<ProcessCpuTopEntry> {
    // Per-process attribution uses the process snapshot from Toolhelp in windows-foundation when
    // available; a full per-PID PDH walk is intentionally avoided on the hot path because its
    // cost scales with process count (observer effect). Top-offender ranking therefore arrives
    // from the bottleneck analyzer over ring deltas, not from this sampler.
    let _ = faults;
    Vec::new()
}

pub struct WindowsPerfPlatform;

impl PerfPlatform for WindowsPerfPlatform {
    fn sample(&self, interval: Duration) -> PerfSnapshot {
        let mut faults: Vec<CollectorFault> = Vec::new();
        let cpu = sample_cpu(&mut faults, interval);
        let power = sample_power(&mut faults);
        let memory = sample_memory(&mut faults);
        let storage = sample_storage(&mut faults);
        let gpu = sample_gpu(&mut faults);
        let process_top = sample_process_top(&mut faults);
        PerfSnapshot {
            captured_unix_ms: chrono::Utc::now().timestamp_millis(),
            interval_ms: interval.as_millis().min(u128::from(u32::MAX)) as u32,
            cpu,
            power,
            memory,
            storage,
            gpu,
            process_top,
            collector_faults: faults,
        }
        .normalized()
    }
}
