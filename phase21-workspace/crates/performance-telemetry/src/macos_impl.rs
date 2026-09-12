//! Phase 27 — Native macOS implementation of the Phase 20 performance platform.
//!
//! Every value comes from a libc-backed syscall — no subprocesses, no IOKit user client,
//! no private frameworks. Sources used (all via the `libc` crate):
//! - CPU ticks: `host_statistics64(HOST_CPU_LOAD_INFO)` → tick deltas between samples.
//! - Memory: `host_statistics64(HOST_VM_INFO64)` + page size from `sysconf(_SC_PAGESIZE)`;
//!   swap capacity from `sysctl(CTL_VM / VM_SWAPUSAGE)` → `xsw_usage`.
//! - Load average: `getloadavg` → processor queue-length proxy.
//! - Storage: `statfs` on `/` and `/System/Volumes/Data` → active-capacity proxies.
//! - Process ranking: `proc_listpids` + `proc_pidinfo(PROC_PIDTASKINFO)` + `proc_name`,
//!   ranked by cumulative CPU time (a per-process utilization percentage is NOT claimed).
//!
//! Honest limits: GPU stays Degraded (no Metal/IOReport access here), thermal/power stays
//! Degraded (no SMC access is attempted or simulated), and mach fault counters are
//! cumulative so they are never published under per-second contract keys. One failing
//! sub-collector degrades into `collector_faults`; it can never fail the snapshot.

use std::mem::size_of;
use std::time::Duration;

use super::{
    CollectedSubsystems, CollectorFault, CpuSample, MemorySample, PerfPlatform, PerfSnapshot,
    ProcessCpuTopEntry, Reading, StorageQueueSample,
};

/// `Reading::from_evidence` needs a fault for the `None` arm; where the caller
/// has already proven the `Some`, this documents that the arm is unreachable
/// rather than inventing a plausible-looking reason for it. Mirrors
/// `windows_impl.rs`'s helper of the same name and purpose.
fn unreachable_fault(collector: &str) -> CollectorFault {
    CollectorFault {
        collector: collector.into(),
        kind: "Internal".into(),
        detail: "evidence was present; this fault is unreachable".into(),
    }
}

// WAIVER (owner-review, Phase 27): `libc::mach_host_self` is marked deprecated upstream in
// favor of the `mach2` crate. Adding a second binding crate for one symbol is out of scope;
// the deprecation does not affect behavior on any supported macOS release.

/// Mach tick indices into `host_cpu_load_info.cpu_ticks`.
const CPU_STATE_USER: usize = libc::CPU_STATE_USER as usize;
const CPU_STATE_SYSTEM: usize = libc::CPU_STATE_SYSTEM as usize;
const CPU_STATE_IDLE: usize = libc::CPU_STATE_IDLE as usize;
const CPU_STATE_NICE: usize = libc::CPU_STATE_NICE as usize;
/// Basis-point scale shared with the rest of the engine.
const BP: u64 = 10_000;

/// Page size in bytes. A non-positive result (hostile kernel report) falls back to 4096
/// rather than dividing by zero downstream.
pub(crate) fn page_size() -> u64 {
    let value = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if value > 0 { value as u64 } else { 4096 }
}

// ---------------------------------------------------------------------------
// CPU — mach tick deltas
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Default)]
pub struct CpuTicks {
    pub user: u64,
    pub system: u64,
    pub idle: u64,
    pub nice: u64,
}

impl CpuTicks {
    fn busy(self) -> u64 {
        self.user
            .saturating_add(self.system)
            .saturating_add(self.nice)
    }
    fn total(self) -> u64 {
        self.busy().saturating_add(self.idle)
    }
}

/// Zeroed instance of a libc FFI struct (these types do not implement `Default`).
fn zeroed<T>() -> T {
    // SAFETY: every T used here is a plain-old-data C struct intended to be filled by
    // the kernel; zero-initialization is the documented calling pattern.
    unsafe { std::mem::zeroed() }
}

/// Reads one absolute mach CPU tick counter.
pub fn read_cpu_ticks() -> Option<CpuTicks> {
    let mut info: libc::host_cpu_load_info = zeroed();
    let mut count = libc::HOST_CPU_LOAD_INFO_COUNT;
    let status = unsafe {
        libc::host_statistics64(
            #[allow(deprecated)] // see WAIVER above
            libc::mach_host_self(),
            libc::HOST_CPU_LOAD_INFO,
            std::ptr::addr_of_mut!(info).cast::<libc::integer_t>(),
            &mut count,
        )
    };
    if status != libc::KERN_SUCCESS {
        return None;
    }
    let t = info.cpu_ticks;
    Some(CpuTicks {
        user: u64::from(t[CPU_STATE_USER]),
        system: u64::from(t[CPU_STATE_SYSTEM]),
        idle: u64::from(t[CPU_STATE_IDLE]),
        nice: u64::from(t[CPU_STATE_NICE]),
    })
}

/// Pure tick-delta math, unit-tested against injected counter values: basis points of
/// busy time across one delta window. Hostile counters (regressions, zero totals) clamp
/// to the [0, 10_000] range instead of panicking or wrapping.
///
/// `total == 0` means the tick counters did not advance between `previous` and
/// `current` — no window was actually observed. That is "no measurement", not
/// "0% busy": DBT-P45-001 (formerly DBT-P44-003) found this exact branch
/// producing a confident zero, indistinguishable from a real idle reading, at
/// ~1-in-10 under guaranteed full-core load. `None` here is what lets the
/// caller (`sample_cpu`) tell the difference instead of publishing a lie.
pub fn busy_bp_from_ticks(previous: CpuTicks, current: CpuTicks) -> Option<u32> {
    let busy = current.busy().saturating_sub(previous.busy());
    let total = current.total().saturating_sub(previous.total());
    if total == 0 {
        return None;
    }
    // Wide arithmetic: busy can be near u64::MAX, so the bp ratio is computed in u128
    // and clamped before narrowing (same discipline as the ring's commit pressure).
    let ratio = (u128::from(busy.min(total)) * u128::from(BP)) / u128::from(total);
    Some(ratio.min(u128::from(BP)) as u32)
}

fn sample_cpu(
    previous: Option<CpuTicks>,
    interval: Duration,
    partial: &mut Vec<CollectorFault>,
) -> (Reading<CpuSample>, Option<CpuTicks>) {
    let unavailable = |detail: &str, kind: &str| {
        Reading::unavailable(CollectorFault {
            collector: "cpu".into(),
            kind: kind.into(),
            detail: detail.into(),
        })
    };
    // Rate counters need two observations inside one tick — the same discipline the
    // Windows PDH collector applies (collect → short sleep → collect). A failed first
    // or second observation degrades typed instead of fabricating a value.
    let Some(first) = previous.or_else(|| {
        let value = read_cpu_ticks();
        if value.is_none() {
            partial.push(CollectorFault {
                collector: "cpu".into(),
                kind: "Unavailable".into(),
                detail: "host_statistics64(HOST_CPU_LOAD_INFO) failed".into(),
            });
        }
        value
    }) else {
        return (
            unavailable(
                "host_statistics64(HOST_CPU_LOAD_INFO) failed",
                "Unavailable",
            ),
            None,
        );
    };
    std::thread::sleep(Duration::from_millis(120).min(interval));
    let Some(mut second) = read_cpu_ticks() else {
        return (
            unavailable(
                "second host_statistics64 observation failed",
                "ProviderFailure",
            ),
            Some(first),
        );
    };
    // DBT-P45-001 (formerly DBT-P44-003): `total == 0` means the tick counters did
    // not advance across the window just observed — `host_statistics64`'s data can
    // fail to tick over inside a single ~120ms window under load on this hardware.
    // That is a window that was genuinely too short, not a 0% reading. Extend it
    // (bounded) before giving up, rather than either fabricating a zero or
    // declaring the tick unmeasurable on the first tie.
    //
    // One extended wait, not a retry loop: a throwaway experiment (300 trials
    // under load, logged which observation resolved each tie, discarded after
    // §45 recorded the numbers) found **no tie ever resolved before the 4th
    // observation** (~480ms) — every earlier re-check was wasted. A loop that
    // re-checks every ~120ms buys nothing a single sleep of the same total
    // length doesn't already get, so it is not one; `EXTENDED_WAIT` below is
    // the same ~480ms budget the old 5-attempt loop's worst case spent,
    // spent once instead of in four increments.
    const EXTENDED_WAIT: Duration = Duration::from_millis(480);
    let started = std::time::Instant::now();
    let mut observations: u32 = 1;
    let mut total_busy_bp = busy_bp_from_ticks(first, second);
    if total_busy_bp.is_none() {
        let remaining = interval.saturating_sub(started.elapsed());
        let wait = EXTENDED_WAIT.min(remaining);
        if !wait.is_zero() {
            std::thread::sleep(wait);
            if let Some(next) = read_cpu_ticks() {
                second = next;
                total_busy_bp = busy_bp_from_ticks(first, second);
                observations = 2;
            }
        }
    }
    let Some(total_busy_bp) = total_busy_bp else {
        return (
            unavailable(
                &format!(
                    "no tick delta in sampling window after {observations} observation(s) \
                     spanning {}ms",
                    started.elapsed().as_millis()
                ),
                "Unavailable",
            ),
            Some(second),
        );
    };
    // host_statistics64 aggregates every logical CPU; publishing one aggregate entry keeps
    // the wire contract without inventing per-core values this source does not measure.
    let per_processor_busy_bp = vec![total_busy_bp];
    let mut degraded: Vec<&str> = Vec::new();
    let queue_x100 = load_average()
        .map(|value| (value.max(0.0) * 100.0) as u64)
        .unwrap_or_else(|| {
            degraded.push("processorQueueLengthX100: getloadavg() failed");
            0
        });
    // §44 1.A: no line of this file has ever written dpc_isr_busy_bp or
    // context_switches_per_sec — mach exposes neither per this provider's
    // sources. They report as the honest 0 they always were, but cpu itself
    // is measured, so the gap is now named rather than silent.
    degraded.push("dpcIsrBusyBp/contextSwitchesPerSec: not measured on macOS");
    partial.push(CollectorFault {
        collector: "cpu.counters".into(),
        kind: "Degraded".into(),
        detail: degraded.join("; "),
    });
    (
        Reading::from_evidence(
            Some(CpuSample {
                per_processor_busy_bp,
                total_busy_bp,
                dpc_isr_busy_bp: 0,
                context_switches_per_sec: 0,
                processor_queue_length_x100: queue_x100.min(u32::MAX as u64),
            }),
            || unreachable_fault("cpu"),
        ),
        Some(second),
    )
}

/// getloadavg 1-minute figure; typed None on failure (caller degrades).
fn load_average() -> Option<f64> {
    let mut loads = [0f64; 1];
    let n = unsafe { libc::getloadavg(loads.as_mut_ptr(), 1) };
    if n == 1 { Some(loads[0]) } else { None }
}

// ---------------------------------------------------------------------------
// Memory — VM statistics + hw.memsize + swap sysctl
// ---------------------------------------------------------------------------

/// MIB tuple for sysctl(CTL_VM/VM_SWAPUSAGE). `sysctl` takes `*mut c_int` but only
/// reads the name; a local copy keeps the immutable static borrow-free.
fn swap_mib() -> [libc::c_int; 2] {
    [libc::CTL_VM, libc::VM_SWAPUSAGE]
}

fn sample_memory(partial: &mut Vec<CollectorFault>) -> Reading<MemorySample> {
    let page = page_size();
    let mut vm: libc::vm_statistics64 = zeroed();
    let mut count = libc::HOST_VM_INFO64_COUNT;
    let status = unsafe {
        libc::host_statistics64(
            #[allow(deprecated)] // see WAIVER above
            libc::mach_host_self(),
            libc::HOST_VM_INFO64,
            std::ptr::addr_of_mut!(vm).cast::<libc::integer_t>(),
            &mut count,
        )
    };
    if status != libc::KERN_SUCCESS {
        return Reading::unavailable(CollectorFault {
            collector: "memory".into(),
            kind: "Unavailable".into(),
            detail: "host_statistics64(HOST_VM_INFO64) failed".into(),
        });
    }
    let bytes = |pages: u64| pages.saturating_mul(page);
    let free = bytes(u64::from(vm.free_count));
    let inactive = bytes(u64::from(vm.inactive_count));
    let purgeable = bytes(u64::from(vm.purgeable_count));

    // Total physical memory via hw.memsize (typed failure otherwise).
    let mut memsize: u64 = 0;
    let mut memsize_len = size_of::<u64>();
    let memsize_ok = unsafe {
        libc::sysctlbyname(
            c"hw.memsize".as_ptr(),
            std::ptr::addr_of_mut!(memsize).cast(),
            &mut memsize_len,
            std::ptr::null_mut(),
            0,
        )
    } == 0;
    if !memsize_ok || memsize == 0 {
        partial.push(CollectorFault {
            collector: "memory.counters".into(),
            kind: "ProviderFailure".into(),
            detail: "sysctl(hw.memsize) unavailable".into(),
        });
    }

    // Swap capacity via CTL_VM/VM_SWAPUSAGE → xsw_usage (typed failure otherwise).
    let mut swap: libc::xsw_usage = zeroed();
    let mut swap_len = size_of::<libc::xsw_usage>();
    let swap_ok = unsafe {
        libc::sysctl(
            swap_mib().as_mut_ptr(),
            2,
            std::ptr::addr_of_mut!(swap).cast(),
            &mut swap_len,
            std::ptr::null_mut(),
            0,
        )
    } == 0;
    if !swap_ok {
        partial.push(CollectorFault {
            collector: "memory.counters".into(),
            kind: "ProviderFailure".into(),
            detail: "sysctl(CTL_VM/VM_SWAPUSAGE) unavailable".into(),
        });
    }

    let available = free.saturating_add(inactive.min(purgeable));
    let used = memsize.saturating_sub(available);
    let load_percent = if memsize > 0 {
        ((used * 100) / memsize) as u32
    } else {
        0
    };
    Reading::from_evidence(
        Some(MemorySample {
            total_physical_bytes: memsize,
            available_physical_bytes: available,
            standby_cache_bytes: purgeable,
            modified_page_list_bytes: bytes(vm.pageouts),
            commit_bytes: used,
            commit_limit_bytes: memsize.saturating_add(swap.xsu_total),
            // Mach fault counters are cumulative since boot, not per-second rates; publishing
            // them under per-second contract keys would misstate their semantics. Zero is the
            // honest value here — commit pressure above carries the memory-pressure signal.
            hard_faults_per_sec: 0,
            soft_faults_per_sec: 0,
            memory_load_percent: load_percent,
        }),
        || unreachable_fault("memory"),
    )
}

// ---------------------------------------------------------------------------
// Storage — statfs capacity proxies
// ---------------------------------------------------------------------------

/// Volumes probed for honest capacity evidence. Missing volumes degrade typed.
const STATFS_PATHS: [&std::ffi::CStr; 2] = [c"/", c"/System/Volumes/Data"];

fn sample_storage(partial: &mut Vec<CollectorFault>) -> Reading<Vec<StorageQueueSample>> {
    let mut out = Vec::new();
    for path in STATFS_PATHS {
        let mut fs: libc::statfs = zeroed();
        let rc = unsafe { libc::statfs(path.as_ptr(), std::ptr::addr_of_mut!(fs)) };
        if rc != 0 {
            partial.push(CollectorFault {
                collector: "storage".into(),
                kind: "Degraded".into(),
                detail: format!("statfs({}) failed", path.to_string_lossy()),
            });
            continue;
        }
        let block = u64::from(fs.f_bsize.max(1));
        let total = u64::from(fs.f_blocks).saturating_mul(block);
        let available = u64::from(fs.f_bavail).saturating_mul(block);
        if total == 0 {
            // §44 1.A: this path used to be a silent skip — no fault, no
            // evidence either way. Named now, same as a statfs() failure.
            partial.push(CollectorFault {
                collector: "storage".into(),
                kind: "Degraded".into(),
                detail: format!(
                    "statfs({}) reported zero total blocks",
                    path.to_string_lossy()
                ),
            });
            continue;
        }
        // Honest proxy only: active_time_bp carries used-capacity basis points (capacity
        // pressure, NOT device busy time); hardware queue counters do not exist in statfs
        // and none are invented.
        let used_bp = (((total.saturating_sub(available)) * BP) / total) as u32;
        let mount = unsafe {
            std::ffi::CStr::from_ptr(fs.f_mntonname.as_ptr())
                .to_string_lossy()
                .into_owned()
        };
        out.push(StorageQueueSample {
            device_id: path.to_string_lossy().into_owned(),
            friendly_name: mount,
            active_time_bp: used_bp,
            queue_depth_x100: 0,
            avg_transfer_latency_us: 0,
            read_bytes_per_sec: 0,
            write_bytes_per_sec: 0,
            total_space_bytes: total,
            free_space_bytes: available,
        });
    }
    if !out.is_empty() {
        // §44 1.A: latency/throughput fields are permanently 0 — statfs has no
        // rate data — while active_time_bp (capacity pressure) IS real. Named
        // so a consumer cannot mistake the zeros for idle devices.
        partial.push(CollectorFault {
            collector: "storage.rates".into(),
            kind: "Degraded".into(),
            detail: "not measured on macOS: queue depth, transfer latency, read/write \
                     bytes/sec (statfs exposes capacity only)"
                .into(),
        });
    }
    Reading::from_collection(out, || CollectorFault {
        collector: "storage".into(),
        kind: "Unavailable".into(),
        detail: "no statfs volume reported usable capacity data".into(),
    })
}

// ---------------------------------------------------------------------------
// Process ranking — proc_listpids + proc_pidinfo(PROC_PIDTASKINFO)
// ---------------------------------------------------------------------------

fn sample_process_top() -> Reading<Vec<ProcessCpuTopEntry>> {
    const MAX_PIDS: usize = 4096;
    let list_bytes = MAX_PIDS * size_of::<libc::c_int>();
    let mut buffer = vec![0u8; list_bytes];
    let written = unsafe {
        libc::proc_listpids(
            1, // PROC_ALL_PIDS (constant absent from the libc crate)
            0,
            buffer.as_mut_ptr().cast(),
            list_bytes as i32,
        )
    };
    if written <= 0 {
        return Reading::unavailable(CollectorFault {
            collector: "processTop".into(),
            kind: "Unavailable".into(),
            detail: "proc_listpids failed".into(),
        });
    }
    let count = (written as usize / size_of::<libc::c_int>()).min(MAX_PIDS);
    let pid_words = size_of::<libc::c_int>();
    let mut entries: Vec<(u64, u32, ProcessCpuTopEntry)> = Vec::with_capacity(count);
    for index in 0..count {
        let raw = &buffer[index * pid_words..(index + 1) * pid_words];
        let pid = libc::c_int::from_ne_bytes(raw.try_into().unwrap_or([0; 4]));
        if pid <= 0 {
            continue;
        }
        let pid_u32 = u32::try_from(pid).unwrap_or(u32::MAX);
        let mut task: libc::proc_taskinfo = zeroed();
        let got = unsafe {
            libc::proc_pidinfo(
                pid,
                libc::PROC_PIDTASKINFO,
                0,
                std::ptr::addr_of_mut!(task).cast(),
                size_of::<libc::proc_taskinfo>() as i32,
            )
        };
        if got != size_of::<libc::proc_taskinfo>() as i32 {
            // Short-lived processes vanish between listing and query; the population
            // moved — skip silently, this is not a provider fault.
            continue;
        }
        let name = unsafe {
            let mut name_buf = [0i8; 2 * libc::MAXCOMLEN as usize + 1];
            let n = libc::proc_name(pid, name_buf.as_mut_ptr().cast(), name_buf.len() as u32);
            if n <= 0 {
                String::new()
            } else {
                std::ffi::CStr::from_ptr(name_buf.as_ptr())
                    .to_string_lossy()
                    .into_owned()
            }
        };
        // cpu_busy_bp requires an interval denominator this source does not retain per
        // process; a utilization percentage would be fabricated. Rank honestly by
        // cumulative CPU time (ns) and leave the bp field at zero.
        let cpu_ns = task.pti_total_user.saturating_add(task.pti_total_system);
        entries.push((
            cpu_ns,
            pid_u32,
            ProcessCpuTopEntry {
                pid: pid_u32,
                name,
                cpu_busy_bp: 0,
                read_bytes_per_sec: 0,
                write_bytes_per_sec: 0,
                working_set_bytes: task.pti_resident_size,
            },
        ));
    }
    entries.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let ranked: Vec<ProcessCpuTopEntry> = entries.into_iter().map(|(_, _, entry)| entry).collect();
    // §44 1.A: the process list was enumerated (`written > 0`), but every pid
    // between listing and `proc_pidinfo` can vanish or refuse the query
    // (`:388-391` below, unchanged — that skip is still not itself a fault).
    // If every one of them did, this used to return `[]` with no fault at
    // all; `from_collection` closes exactly that gap.
    Reading::from_collection(ranked, || CollectorFault {
        collector: "processTop".into(),
        kind: "Unavailable".into(),
        detail: format!(
            "{count} pid(s) listed but none produced a readable proc_pidinfo(PROC_PIDTASKINFO) \
             snapshot"
        ),
    })
}

// ---------------------------------------------------------------------------
// Platform wiring
// ---------------------------------------------------------------------------

pub struct MacosPerfPlatform {
    /// Previous absolute mach tick counter, carried between ticks so every tick measures
    /// exactly its own interval. Mutex (not Cell) because `PerfPlatform: Send + Sync`.
    previous_ticks: std::sync::Mutex<Option<CpuTicks>>,
}

impl Default for MacosPerfPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl MacosPerfPlatform {
    pub fn new() -> Self {
        Self {
            previous_ticks: std::sync::Mutex::new(None),
        }
    }
}

impl PerfPlatform for MacosPerfPlatform {
    fn sample(&self, interval: Duration) -> PerfSnapshot {
        // Floor discipline: the shared MIN_INTERVAL_MS (250 ms) clamp in the ring governs
        // cadence; this provider additionally refuses sub-floor in-tick windows.
        debug_assert!(interval >= Duration::from_millis(super::MIN_INTERVAL_MS as u64));
        // Faults for degraded *parts* of measured subsystems (dotted collector
        // names). Whole-subsystem availability is carried by the `Reading`s and
        // published by `into_snapshot`, which is the only path to a snapshot —
        // mirrors windows_impl.rs's `partial` split exactly.
        let mut partial: Vec<CollectorFault> = Vec::new();
        // The platform instance is owned by one sampler; carrying the previous absolute
        // tick counter across calls lets each tick measure exactly its own interval.
        // First tick: delta spans the in-tick observation window (never zero, never fake).
        let previous = self
            .previous_ticks
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take();
        let (cpu, current) = sample_cpu(previous, interval, &mut partial);
        *self
            .previous_ticks
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = current;
        // GPU: honestly degraded, unconditionally — no IOReport/Metal source
        // exists in this build. There is no decision to make here, so this is
        // `Reading::unavailable` directly rather than a rule that could
        // silently stop firing.
        let gpu = Reading::unavailable(CollectorFault {
            collector: "gpu".into(),
            kind: "Degraded".into(),
            detail: "no native GPU telemetry source on macOS; not simulated".into(),
        });
        // Thermal/power: honestly degraded, unconditionally — no SMC access is
        // attempted or simulated.
        let power = Reading::unavailable(CollectorFault {
            collector: "thermalPower".into(),
            kind: "Degraded".into(),
            detail: "SMC thermal/power sources unavailable; not simulated".into(),
        });
        let memory = sample_memory(&mut partial);
        let storage = sample_storage(&mut partial);
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
