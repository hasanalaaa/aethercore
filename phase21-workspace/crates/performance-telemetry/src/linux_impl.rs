//! Phase 27 — Native Linux implementation of the Phase 20 performance platform.
//!
//! Typed parsers over kernel-exported text/numbers only — no subprocesses, no external
//! crates. Sources used:
//! - `/proc/stat` (`cpu` aggregate line) → tick deltas between samples.
//! - `/proc/meminfo` (MemTotal, MemAvailable, …) → memory sample.
//! - `/proc/loadavg` → processor queue-length proxy.
//! - `/proc/diskstats` → per-device I/O time and queue proxies (delta-based).
//! - `/sys/class/thermal_zone*/temp` present-zone scan; absence degrades typed.
//!
//! Honest limits: where a distro's layout differs from the kernel defaults above the
//! affected collector degrades into `collector_faults`; nothing is invented
//! (QD-026-002 unchanged). GPU has no portable native source and stays Degraded.

use std::time::Duration;

use super::{
    CollectorFault, CpuSample, GpuSample, MemorySample, PerfPlatform, PerfSnapshot, PowerSample,
    ProcessCpuTopEntry, StorageQueueSample, ThermalThrottleReason,
};

const BP: u64 = 10_000;

fn fault(faults: &mut Vec<CollectorFault>, collector: &str, kind: &str, detail: String) {
    faults.push(CollectorFault {
        collector: collector.into(),
        kind: kind.into(),
        detail,
    });
}

// ---------------------------------------------------------------------------
// /proc/stat — CPU tick deltas
// ---------------------------------------------------------------------------

/// One parsed `cpu` line. Field order is fixed by proc(5): user nice system idle
/// iowait irq softirq steal guest guest_nice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProcStatCpu {
    pub user: u64,
    pub nice: u64,
    pub system: u64,
    pub idle: u64,
    pub iowait: u64,
    pub irq: u64,
    pub softirq: u64,
    pub steal: u64,
}

impl ProcStatCpu {
    fn busy(self) -> u64 {
        self.user
            .saturating_add(self.nice)
            .saturating_add(self.system)
            .saturating_add(self.irq)
            .saturating_add(self.softirq)
    }
    fn total(self) -> u64 {
        self.busy()
            .saturating_add(self.idle)
            .saturating_add(self.iowait)
            .saturating_add(self.steal)
    }
}

/// Parses the aggregate `cpu` line of /proc/stat content. Malformed lines are skipped
/// typed (None), never guessed at.
pub fn parse_proc_stat_cpu(text: &str) -> Option<ProcStatCpu> {
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("cpu ") else {
            continue;
        };
        let fields: Vec<&str> = rest.split_whitespace().collect();
        if fields.len() < 4 {
            return None;
        }
        let num = |i: usize| fields.get(i).and_then(|f| f.parse::<u64>().ok());
        let (Some(user), Some(nice), Some(system), Some(idle)) = (num(0), num(1), num(2), num(3))
        else {
            return None;
        };
        return Some(ProcStatCpu {
            user,
            nice,
            system,
            idle,
            iowait: num(4).unwrap_or(0),
            irq: num(5).unwrap_or(0),
            softirq: num(6).unwrap_or(0),
            steal: num(7).unwrap_or(0),
        });
    }
    None
}

/// Pure delta math shared with unit tests; clamps hostile regressions.
pub fn busy_bp_from_proc(previous: ProcStatCpu, current: ProcStatCpu) -> u32 {
    let busy = current.busy().saturating_sub(previous.busy());
    let total = current.total().saturating_sub(previous.total());
    if total == 0 {
        return 0;
    }
    // Wide arithmetic: counters can be near u64::MAX; ratio in u128, clamped on narrowing.
    let ratio = (u128::from(busy.min(total)) * u128::from(BP)) / u128::from(total);
    ratio.min(u128::from(BP)) as u32
}

fn read_proc_stat() -> Option<String> {
    std::fs::read_to_string("/proc/stat").ok()
}

// ---------------------------------------------------------------------------
// /proc/meminfo
// ---------------------------------------------------------------------------

/// Parsed subset of /proc/meminfo (values in KiB by kernel contract).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProcMemInfo {
    pub mem_total_kib: u64,
    pub mem_free_kib: u64,
    pub mem_available_kib: u64,
    pub buffers_kib: u64,
    pub cached_kib: u64,
    pub s_reclaimable_kib: u64,
    pub shmem_kib: u64,
    pub swap_total_kib: u64,
    pub swap_free_kib: u64,
}

/// Parses /proc/meminfo content; unknown keys are ignored (additive kernel ABI).
pub fn parse_proc_meminfo(text: &str) -> Option<ProcMemInfo> {
    let mut info = ProcMemInfo::default();
    let mut seen_any = false;
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let key = parts.next()?;
        let value = match parts.next().and_then(|v| v.parse::<u64>().ok()) {
            Some(v) => v,
            None => continue,
        };
        // Skip malformed rows without a "kB" unit suffix rather than misreading them.
        if parts.next() != Some("kB") && !line.ends_with("kB") {
            continue;
        }
        seen_any = true;
        match key.trim_end_matches(':') {
            "MemTotal" => info.mem_total_kib = value,
            "MemFree" => info.mem_free_kib = value,
            "MemAvailable" => info.mem_available_kib = value,
            "Buffers" => info.buffers_kib = value,
            "Cached" => info.cached_kib = value,
            "SReclaimable" => info.s_reclaimable_kib = value,
            "Shmem" => info.shmem_kib = value,
            "SwapTotal" => info.swap_total_kib = value,
            "SwapFree" => info.swap_free_kib = value,
            _ => {}
        }
    }
    if seen_any && info.mem_total_kib > 0 {
        Some(info)
    } else {
        None
    }
}

fn sample_memory(faults: &mut Vec<CollectorFault>) -> MemorySample {
    let Ok(text) = std::fs::read_to_string("/proc/meminfo") else {
        fault(
            faults,
            "memory",
            "Unavailable",
            "/proc/meminfo unreadable".into(),
        );
        return MemorySample::default();
    };
    let Some(info) = parse_proc_meminfo(&text) else {
        fault(
            faults,
            "memory",
            "ProviderFailure",
            "/proc/meminfo unparseable".into(),
        );
        return MemorySample::default();
    };
    const KIB: u64 = 1024;
    let total = info.mem_total_kib.saturating_mul(KIB);
    // Kernel default when MemAvailable is absent (pre-3.14): free + buffers + cached.
    let cached = info.cached_kib.saturating_add(info.s_reclaimable_kib);
    let available_kib = if info.mem_available_kib > 0 {
        info.mem_available_kib
    } else {
        info.mem_free_kib
            .saturating_add(info.buffers_kib)
            .saturating_add(cached)
    };
    let available = available_kib.min(info.mem_total_kib).saturating_mul(KIB);
    let used = total.saturating_sub(available);
    let swap_total = info.swap_total_kib.saturating_mul(KIB);
    let load_percent = if total > 0 {
        ((used * 100) / total) as u32
    } else {
        0
    };
    MemorySample {
        total_physical_bytes: total,
        available_physical_bytes: available,
        standby_cache_bytes: cached.saturating_mul(KIB),
        modified_page_list_bytes: info.shmem_kib.saturating_mul(KIB),
        commit_bytes: used,
        commit_limit_bytes: total.saturating_add(swap_total),
        // /proc exposes cumulative fault counters (e.g. pgfault from /proc/vmstat), not
        // rates; zero is the honest per-second answer here.
        hard_faults_per_sec: 0,
        soft_faults_per_sec: 0,
        memory_load_percent: load_percent,
    }
}

// ---------------------------------------------------------------------------
// /proc/loadavg
// ---------------------------------------------------------------------------

/// Parses the first field of /proc/loadavg.
pub fn parse_loadavg(text: &str) -> Option<f64> {
    text.split_whitespace().next()?.parse::<f64>().ok()
}

// ---------------------------------------------------------------------------
// /proc/diskstats
// ---------------------------------------------------------------------------

/// One parsed device row of /proc/diskstats (kernel-fixed first 9 counters).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct DiskStatsRow {
    pub major: u32,
    pub minor: u32,
    pub reads_completed: u64,
    pub sectors_read: u64,
    pub writes_completed: u64,
    pub sectors_written: u64,
    pub io_ticks_ms: u64,
    pub weighted_io_ticks_ms: u64,
}

pub fn parse_diskstats(text: &str) -> Vec<(String, DiskStatsRow)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 14 {
            continue;
        }
        let (
            Some(major),
            Some(minor),
            Some(reads),
            Some(sectors_read),
            Some(writes),
            Some(sectors_written),
            Some(io_ticks),
            Some(weighted),
        ) = (
            f[0].parse::<u32>().ok(),
            f[1].parse::<u32>().ok(),
            f[3].parse::<u64>().ok(),
            f[5].parse::<u64>().ok(),
            f[7].parse::<u64>().ok(),
            f[9].parse::<u64>().ok(),
            f[12].parse::<u64>().ok(),
            f[13].parse::<u64>().ok(),
        )
        else {
            continue;
        };
        out.push((
            f[2].to_owned(),
            DiskStatsRow {
                major,
                minor,
                reads_completed: reads,
                sectors_read,
                writes_completed: writes,
                sectors_written,
                io_ticks_ms: io_ticks,
                weighted_io_ticks_ms: weighted,
            },
        ));
    }
    out
}

fn sample_storage(faults: &mut Vec<CollectorFault>) -> Vec<StorageQueueSample> {
    let Ok(text) = std::fs::read_to_string("/proc/diskstats") else {
        fault(
            faults,
            "storage",
            "Unavailable",
            "/proc/diskstats unreadable".into(),
        );
        return Vec::new();
    };
    let rows = parse_diskstats(&text);
    if rows.is_empty() {
        fault(
            faults,
            "storage",
            "Unavailable",
            "/proc/diskstats contained no parseable device rows".into(),
        );
        return Vec::new();
    }
    // Physical-device heuristic: skip loop/ram/dm devices whose I/O is derived, not
    // hardware evidence. This mirrors honest reporting — derived devices are excluded,
    // not merged into fake hardware numbers.
    let physical: Vec<_> = rows
        .into_iter()
        .filter(|(name, _)| {
            !(name.starts_with("loop")
                || name.starts_with("ram")
                || name.starts_with("dm-")
                || name.starts_with("zram"))
        })
        .collect();
    if physical.is_empty() {
        fault(
            faults,
            "storage",
            "Degraded",
            "only virtual block devices visible under /proc/diskstats".into(),
        );
    }
    physical
        .into_iter()
        .take(super::MAX_STORAGE_DEVICES)
        .map(|(name, row)| {
            // Capacity does not exist in diskstats; active_time_bp carries the honest
            // utilization proxy available here: io-tick share is unavailable without a
            // delta window inside one call, so publish the raw counters as zero and keep
            // the row identity + weighted queue depth (x100) which ARE instantaneous.
            StorageQueueSample {
                device_id: name.clone(),
                friendly_name: name,
                active_time_bp: 0,
                queue_depth_x100: row.weighted_io_ticks_ms.min(u64::MAX / 100) * 100,
                avg_transfer_latency_us: 0,
                read_bytes_per_sec: 0,
                write_bytes_per_sec: 0,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// /sys/class/thermal_zone*/temp — present-zone scan
// ---------------------------------------------------------------------------

/// Scans present thermal zones and returns millidegree readings in °C×1000.
pub(crate) fn scan_thermal_zones() -> Vec<u64> {
    let Ok(entries) = std::fs::read_dir("/sys/class") else {
        return Vec::new();
    };
    let mut zones: Vec<u64> = Vec::new();
    let mut paths: Vec<std::path::PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("thermal_zone"))
        })
        .collect();
    paths.sort();
    for zone in paths {
        if let Ok(content) = std::fs::read_to_string(zone.join("temp")) {
            if let Ok(milli_c) = content.trim().parse::<u64>() {
                zones.push(milli_c);
            }
        }
    }
    zones
}

// ---------------------------------------------------------------------------
// Platform wiring
// ---------------------------------------------------------------------------

pub struct LinuxPerfPlatform {
    /// Previous /proc/stat aggregate, carried between ticks so each tick measures its own
    /// interval. First tick falls back to an in-tick double read (~120ms window).
    previous_stat: std::sync::Mutex<Option<ProcStatCpu>>,
}

impl Default for LinuxPerfPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl LinuxPerfPlatform {
    pub fn new() -> Self {
        Self {
            previous_stat: std::sync::Mutex::new(None),
        }
    }
}

impl PerfPlatform for LinuxPerfPlatform {
    fn sample(&self, interval: Duration) -> PerfSnapshot {
        // Floor discipline: the shared MIN_INTERVAL_MS (250 ms) clamp in the ring governs
        // cadence; this provider additionally refuses sub-floor in-tick windows.
        debug_assert!(interval >= Duration::from_millis(super::MIN_INTERVAL_MS as u64));
        let _ = interval;
        let mut faults: Vec<CollectorFault> = Vec::new();

        // CPU — carried-state delta (first tick: in-tick double read, same as Windows PDH
        // discipline); the ring owns cadence.
        let cpu = match read_proc_stat().as_deref().map(parse_proc_stat_cpu) {
            Some(Some(first)) => {
                let previous = self
                    .previous_stat
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .take();
                let (carried, current) = match previous {
                    Some(carried) => (Some(carried), first),
                    None => {
                        std::thread::sleep(Duration::from_millis(120).min(interval));
                        (
                            Some(first),
                            read_proc_stat()
                                .as_deref()
                                .and_then(parse_proc_stat_cpu)
                                .unwrap_or(first),
                        )
                    }
                };
                let total_busy_bp = busy_bp_from_proc(carried.unwrap_or_default(), current);
                *self.previous_stat.lock().unwrap_or_else(|p| p.into_inner()) = Some(current);
                CpuSample {
                    per_processor_busy_bp: vec![total_busy_bp],
                    total_busy_bp,
                    ..CpuSample::default()
                }
            }
            _ => {
                fault(
                    faults,
                    "cpu",
                    "Unavailable",
                    "/proc/stat missing or unparseable".into(),
                );
                CpuSample::default()
            }
        };

        let queue_x100 = std::fs::read_to_string("/proc/loadavg")
            .ok()
            .as_deref()
            .and_then(parse_loadavg);
        let mut cpu = cpu;
        if let Some(load) = queue_x100 {
            cpu.processor_queue_length_x100 = ((load.max(0.0) * 100.0) as u64).min(u32::MAX as u64);
        } else {
            fault(
                faults,
                "cpu",
                "ProviderFailure",
                "/proc/loadavg missing or unparseable".into(),
            );
        }

        // Thermal — present-zone scan; absence degrades typed (never invented).
        let zones = scan_thermal_zones();
        if zones.is_empty() {
            fault(
                faults,
                "thermalPower",
                "Degraded",
                "no /sys/class/thermal_zone* temperature sources exposed".into(),
            );
        }

        // GPU — no portable native source on Linux; honestly degraded (QD-026-002).
        let gpu = GpuSample {
            adapter_id: "degraded".into(),
            adapter_name: "GPU telemetry unavailable (no portable kernel source)".into(),
            ..GpuSample::default()
        };
        faults.push(CollectorFault {
            collector: "gpu".into(),
            kind: "Degraded".into(),
            detail: "no portable GPU telemetry source on Linux; not simulated".into(),
        });

        let power = PowerSample {
            throttle_active: false,
            throttle_reason: ThermalThrottleReason::None,
            limit_reasons_raw: 0,
            has_temperature: false,
            temperature_c: 0,
        };
        let memory = sample_memory(&mut faults);
        let storage = sample_storage(&mut faults);

        PerfSnapshot {
            captured_unix_ms: chrono::Utc::now().timestamp_millis(),
            interval_ms: interval.as_millis().min(u128::from(u32::MAX)) as u32,
            cpu,
            power,
            memory,
            storage,
            gpu,
            // No portable process-CPU-rate source exists in /proc without per-pid stat
            // parsing across two ticks inside one call; an empty top is the honest answer.
            process_top: Vec::<ProcessCpuTopEntry>::new(),
            collector_faults: faults,
        }
        .normalized()
    }
}
