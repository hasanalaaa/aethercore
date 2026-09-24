//! Phase 20 Domain A — zero-overhead performance telemetry engine.
//!
//! The sampler is a passive, bounded, ring-buffered pipeline. Design invariants:
//!
//! 1. **No observer effect.** The sampling thread sleeps between ticks; every collector runs
//!    under a timeout via the shared `collector-runtime` isolation gate. A collector that
//!    misbehaves becomes a typed fault, never a stalled pipeline.
//! 2. **Bounded memory.** The ring holds at most [`MAX_RING_SAMPLES`] samples and each sample
//!    bounds its own repeated collections (processors, devices, processes).
//! 3. **Owner-scoped.** Samples belong to one principal key; nothing leaks across owners.
//! 4. **Deterministic aggregation.** Window statistics are computed from ordered ring contents,
//!    so identical input produces identical aggregates (auditable, testable).
#![deny(unsafe_op_in_unsafe_fn)]

use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::thread;
use std::time::Duration;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Hard cap on retained samples per owner stream (~5 min at 1 s cadence).
pub const MAX_RING_SAMPLES: usize = 300;
/// Requested intervals outside this range are clamped; sub-250 ms would risk observer effects.
pub const MIN_INTERVAL_MS: u32 = 250;
pub const MAX_INTERVAL_MS: u32 = 60_000;
/// Per-sample collection caps.
pub const MAX_STORAGE_DEVICES: usize = 32;
pub const MAX_PROCESS_TOP: usize = 16;
pub const MAX_GPU_ENGINES: usize = 16;
pub const MAX_COLLECTOR_FAULTS: usize = 16;
pub const MAX_FAULT_DETAIL_CHARS: usize = 256;
/// Logical processor cap mirrors `MAXIMUM_PROCESSORS` on Windows.
pub const MAX_CPU_COUNT: usize = 256;

#[derive(Debug, Error)]
pub enum TelemetryError {
    #[error("sampling is already active")]
    AlreadyActive,
    #[error("sampling is not active or identity is missing")]
    NotActive,
    #[error("sample stream belongs to a different principal")]
    OwnerMismatch,
}

// ---------------------------------------------------------------------------
// Sample model (mirrors the proto contract; camelCase for renderer consumption)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CpuSample {
    pub per_processor_busy_bp: Vec<u32>,
    pub total_busy_bp: u32,
    pub dpc_isr_busy_bp: u32,
    pub context_switches_per_sec: u64,
    pub processor_queue_length_x100: u64,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ThermalThrottleReason {
    #[default]
    Unspecified,
    None,
    Thermal,
    Power,
    Vrm,
    Current,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PowerSample {
    pub throttle_active: bool,
    pub throttle_reason: ThermalThrottleReason,
    /// Processor power/thermal limit reason bitmask (raw platform evidence only).
    pub limit_reasons_raw: u64,
    pub has_temperature: bool,
    pub temperature_c: i32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MemorySample {
    pub total_physical_bytes: u64,
    pub available_physical_bytes: u64,
    pub standby_cache_bytes: u64,
    pub modified_page_list_bytes: u64,
    pub commit_bytes: u64,
    pub commit_limit_bytes: u64,
    pub hard_faults_per_sec: u64,
    pub soft_faults_per_sec: u64,
    pub memory_load_percent: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StorageQueueSample {
    pub device_id: String,
    pub friendly_name: String,
    pub active_time_bp: u32,
    pub queue_depth_x100: u64,
    pub avg_transfer_latency_us: u64,
    pub read_bytes_per_sec: u64,
    pub write_bytes_per_sec: u64,
    pub total_space_bytes: u64,
    pub free_space_bytes: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GpuEngineSample {
    pub engine_name: String,
    pub utilization_bp: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GpuSample {
    pub adapter_id: String,
    pub adapter_name: String,
    pub dedicated_used_bytes: u64,
    pub dedicated_total_bytes: u64,
    pub shared_used_bytes: u64,
    pub engines: Vec<GpuEngineSample>,
    pub frametime_jitter_us: u64,
    pub compositor_lag_detected: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessCpuTopEntry {
    pub pid: u32,
    pub name: String,
    pub cpu_busy_bp: u32,
    pub read_bytes_per_sec: u64,
    pub write_bytes_per_sec: u64,
    pub working_set_bytes: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CollectorFault {
    pub collector: String,
    pub kind: String,
    pub detail: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PerfSnapshot {
    pub captured_unix_ms: i64,
    pub interval_ms: u32,
    pub cpu: CpuSample,
    pub power: PowerSample,
    pub memory: MemorySample,
    pub storage: Vec<StorageQueueSample>,
    pub gpu: GpuSample,
    pub process_top: Vec<ProcessCpuTopEntry>,
    pub collector_faults: Vec<CollectorFault>,
}

impl PerfSnapshot {
    /// Enforces every documented capacity bound. All collectors publish through this so a hostile
    /// or buggy counter source cannot inflate renderer memory or produce out-of-range values.
    pub fn normalized(mut self) -> Self {
        let clamp_bp = |bp: &mut u32| *bp = (*bp).min(10_000);
        self.cpu.per_processor_busy_bp.truncate(MAX_CPU_COUNT);
        self.cpu.per_processor_busy_bp.iter_mut().for_each(clamp_bp);
        clamp_bp(&mut self.cpu.total_busy_bp);
        clamp_bp(&mut self.cpu.dpc_isr_busy_bp);
        self.storage.truncate(MAX_STORAGE_DEVICES);
        self.storage
            .iter_mut()
            .for_each(|device| clamp_bp(&mut device.active_time_bp));
        self.gpu.engines.truncate(MAX_GPU_ENGINES);
        self.gpu
            .engines
            .iter_mut()
            .for_each(|engine| clamp_bp(&mut engine.utilization_bp));
        self.process_top.truncate(MAX_PROCESS_TOP);
        self.process_top
            .iter_mut()
            .for_each(|entry| clamp_bp(&mut entry.cpu_busy_bp));
        self.memory.memory_load_percent = self.memory.memory_load_percent.min(100);
        self.collector_faults.truncate(MAX_COLLECTOR_FAULTS);
        for fault in &mut self.collector_faults {
            fault.detail = fault.detail.chars().take(MAX_FAULT_DETAIL_CHARS).collect();
            fault.collector = fault.collector.chars().take(64).collect();
        }
        self
    }

    pub fn interval(&self) -> Duration {
        Duration::from_millis(
            self.interval_ms
                .clamp(MIN_INTERVAL_MS, MAX_INTERVAL_MS)
                .into(),
        )
    }

    /// Interval clamped into the legal range for a start request.
    pub fn clamped_interval_ms(requested_ms: u32) -> u32 {
        requested_ms.clamp(MIN_INTERVAL_MS, MAX_INTERVAL_MS)
    }
}

// ---------------------------------------------------------------------------
// The collector contract (P42 1.B — DBT-P41-002)
// ---------------------------------------------------------------------------

/// A subsystem reading. **There is no third state.**
///
/// §20.1.4 records the defect this type exists to remove: `PerfPlatform::sample`
/// returns `PerfSnapshot`, `PerfSnapshot` derives `Default`, and so an all-zero
/// payload with an empty fault list was a legal, type-checking return —
/// indistinguishable from a real reading of an idle machine. Nine hand-written
/// availability rules (§20.1.1) existed to compensate for that missing contract;
/// gpu's was the only one that fired, and §20.1.6 records that it fired **by
/// accident**. A fix at any one call site left the other eight free to regress.
///
/// So the decision moves into the type. A collector either hands over something
/// it actually read, or it says why it could not. The variants are private and
/// the constructors take the evidence, so "success with nothing measured and
/// nothing declared" is not representable:
///
/// - [`Reading::from_evidence`] takes an `Option<T>` — the shape a failed native
///   read already has — and turns `None` into the fault the caller must supply.
///   This is why the Windows collectors no longer `.unwrap_or(0)`: §20.1.3(a)
///   shows that is exactly how a failed read became a confident zero.
/// - [`Reading::from_collection`] refuses an empty collection.
/// - [`Reading::unavailable`] is the explicit "I could not" for a collector that
///   never got as far as a payload.
pub struct Reading<T>(ReadingInner<T>);

/// Private on purpose: `Reading::Measured` must be unconstructible, inside this
/// crate as well as outside it, without passing evidence to a constructor.
enum ReadingInner<T> {
    Measured(T),
    Unavailable(CollectorFault),
}

impl<T> Reading<T> {
    /// Measured if the collector produced a value, otherwise the supplied fault.
    pub fn from_evidence(
        measured: Option<T>,
        no_evidence: impl FnOnce() -> CollectorFault,
    ) -> Self {
        match measured {
            Some(value) => Self(ReadingInner::Measured(value)),
            None => Self(ReadingInner::Unavailable(no_evidence())),
        }
    }

    /// The collector could not produce a payload at all, and says why.
    pub fn unavailable(fault: CollectorFault) -> Self {
        Self(ReadingInner::Unavailable(fault))
    }

    /// True when this reading carries a measurement.
    pub fn is_measured(&self) -> bool {
        matches!(self.0, ReadingInner::Measured(_))
    }

    /// Splits into the payload the wire format needs and the fault, if any.
    ///
    /// The fallback is only ever reached on the `Unavailable` arm, so a snapshot
    /// field that holds a fallback **always** has a fault standing beside it.
    /// That is the whole guarantee: a zero can appear, but never unexplained.
    pub fn into_parts(self, fallback: impl FnOnce() -> T) -> (T, Option<CollectorFault>) {
        match self.0 {
            ReadingInner::Measured(value) => (value, None),
            ReadingInner::Unavailable(fault) => (fallback(), Some(fault)),
        }
    }
}

impl<T> Reading<Vec<T>> {
    /// Measured if the collection is non-empty; an empty collection is not a
    /// reading. §20.1.2: storage returns a `Vec` exactly like gpu and *could*
    /// have expressed "I got nothing" — it simply never did, across four
    /// fault-free early returns.
    pub fn from_collection(items: Vec<T>, empty: impl FnOnce() -> CollectorFault) -> Self {
        if items.is_empty() {
            Self(ReadingInner::Unavailable(empty()))
        } else {
            Self(ReadingInner::Measured(items))
        }
    }
}

/// Every subsystem of one tick, each one already answered.
///
/// A provider cannot build this without deciding, per subsystem, measured-or-why-not
/// — which is the single contract that replaced the nine rules of §20.1.1.
pub struct CollectedSubsystems {
    pub cpu: Reading<CpuSample>,
    pub power: Reading<PowerSample>,
    pub memory: Reading<MemorySample>,
    pub storage: Reading<Vec<StorageQueueSample>>,
    pub gpu: Reading<GpuSample>,
    pub process_top: Reading<Vec<ProcessCpuTopEntry>>,
}

impl CollectedSubsystems {
    /// Publishes the tick. Every `Unavailable` reading contributes its fault;
    /// there is no path that drops one.
    pub fn into_snapshot(self, interval: Duration) -> PerfSnapshot {
        let mut faults: Vec<CollectorFault> = Vec::new();
        let mut take = |fault: Option<CollectorFault>| {
            if let Some(fault) = fault {
                faults.push(fault);
            }
        };
        let (cpu, fault) = self.cpu.into_parts(CpuSample::default);
        take(fault);
        let (power, fault) = self.power.into_parts(PowerSample::default);
        take(fault);
        let (memory, fault) = self.memory.into_parts(MemorySample::default);
        take(fault);
        let (storage, fault) = self.storage.into_parts(Vec::new);
        take(fault);
        let (gpu, fault) = self.gpu.into_parts(GpuSample::default);
        take(fault);
        let (process_top, fault) = self.process_top.into_parts(Vec::new);
        take(fault);
        PerfSnapshot {
            captured_unix_ms: Utc::now().timestamp_millis(),
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

/// Which telemetry subsystems this snapshot actually measured.
///
/// §41.15 3.C measured `capabilities` reporting 16/16 `native` in the same
/// session in which storage returned `[]` and gpu declared a fault — three
/// deciders disagreeing. This is the one answer all of them now read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MeasuredSubsystems {
    pub cpu: bool,
    pub memory: bool,
    pub storage: bool,
    pub gpu: bool,
}

impl PerfSnapshot {
    /// Naming convention this reads: a fault whose `collector` is exactly the
    /// subsystem name means the subsystem is unavailable; a dotted suffix
    /// (`memory.counters`, `cpu.counters`) means a measured subsystem with a
    /// degraded part.
    pub fn measured_subsystems(&self) -> MeasuredSubsystems {
        let available = |name: &str| {
            !self
                .collector_faults
                .iter()
                .any(|fault| fault.collector == name)
        };
        MeasuredSubsystems {
            cpu: available("cpu"),
            memory: available("memory") && self.memory.total_physical_bytes > 0,
            storage: available("storage") && !self.storage.is_empty(),
            gpu: available("gpu") && !self.gpu.engines.is_empty(),
        }
    }
}

// ---------------------------------------------------------------------------
// Platform abstraction
// ---------------------------------------------------------------------------

pub trait PerfPlatform: Send + Sync + 'static {
    /// One passive tick. Every fallible sub-collector degrades into `collector_faults`
    /// instead of failing the snapshot: partial truth beats no truth, but never lies.
    ///
    /// Providers publish through [`CollectedSubsystems::into_snapshot`], which is
    /// what makes the second half of that sentence true rather than aspirational.
    fn sample(&self, interval: Duration) -> PerfSnapshot;
}

#[cfg(windows)]
mod windows_impl;

#[cfg(windows)]
pub use windows_impl::WindowsPerfPlatform;

// Phase 27 — native unix providers (additive; Windows paths untouched).
#[cfg(target_os = "linux")]
mod linux_impl;
#[cfg(target_os = "macos")]
mod macos_impl;

#[cfg(target_os = "linux")]
pub use linux_impl::LinuxPerfPlatform;
#[cfg(target_os = "macos")]
pub use macos_impl::MacosPerfPlatform;

/// Composition-time provider selection by cfg. `--synthetic` (and every test/audit)
/// forces the deterministic synthetic platform explicitly; on Windows the frozen
/// native provider stays the default exactly as shipped.
pub fn default_platform() -> std::sync::Arc<dyn PerfPlatform> {
    #[cfg(windows)]
    {
        std::sync::Arc::new(WindowsPerfPlatform)
    }
    #[cfg(target_os = "macos")]
    {
        std::sync::Arc::new(MacosPerfPlatform::new())
    }
    #[cfg(target_os = "linux")]
    {
        std::sync::Arc::new(LinuxPerfPlatform::new())
    }
}

/// Test-injection surface for the pure provider math (integration tests only; never
/// part of the production call graph). Re-exports typed views of private helpers.
#[doc(hidden)]
pub mod __test {
    /// DBT-P41-002 seams: the PDH formatted-value decode, the percentage
    /// conversion, and the counter-path parser — exposed so tests can assert the
    /// ABI, the units and the path shape without linking PDH.
    #[cfg(windows)]
    pub use super::windows_impl::{
        PDH_VALUE_SLOT_BYTES, decode_pdh_double, decode_pdh_value, expand_wildcard_path,
        instance_from_counter_path, percentage_to_bp,
    };

    #[cfg(target_os = "macos")]
    pub use super::macos_impl::{CpuTicks, busy_bp_from_ticks, read_cpu_ticks};

    /// Constructs an injected tick counter for delta-math tests.
    #[cfg(target_os = "macos")]
    pub fn cpu_ticks(user: u64, system: u64, idle: u64, nice: u64) -> CpuTicks {
        CpuTicks {
            user,
            system,
            idle,
            nice,
        }
    }

    #[cfg(target_os = "linux")]
    pub use super::linux_impl::{
        busy_bp_from_proc, parse_diskstats as rows_from_diskstats, parse_loadavg as loadavg_from,
        parse_proc_meminfo as meminfo_from_proc, parse_proc_stat_cpu as cpu_from_proc_stat,
    };
}

/// Deterministic synthetic platform used by tests, adversarial audits, and offline UI work.
/// Values derive from a monotonic tick counter plus scenario knobs, so windowed aggregates are
/// exactly predictable and reproducible across hosts.
#[derive(Default)]
pub struct SyntheticPerfPlatform {
    tick: AtomicU64,
    pub cpu_busy_bp: AtomicU64,
    pub hard_faults_per_sec: AtomicU64,
}

impl SyntheticPerfPlatform {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_cpu_busy_bp(&self, bp: u32) -> &Self {
        self.cpu_busy_bp
            .store(u64::from(bp.min(10_000)), Ordering::Relaxed);
        self
    }

    pub fn with_hard_faults(&self, per_sec: u64) -> &Self {
        self.hard_faults_per_sec.store(per_sec, Ordering::Relaxed);
        self
    }
}

impl PerfPlatform for SyntheticPerfPlatform {
    fn sample(&self, interval: Duration) -> PerfSnapshot {
        let tick = self.tick.fetch_add(1, Ordering::Relaxed);
        let busy = self.cpu_busy_bp.load(Ordering::Relaxed).min(10_000) as u32;
        let hard = self.hard_faults_per_sec.load(Ordering::Relaxed);
        PerfSnapshot {
            captured_unix_ms: Utc::now().timestamp_millis(),
            interval_ms: interval.as_millis().min(u128::from(u32::MAX)) as u32,
            cpu: CpuSample {
                per_processor_busy_bp: vec![busy],
                total_busy_bp: busy,
                dpc_isr_busy_bp: ((tick % 11) as u32) * 10,
                context_switches_per_sec: 1_000 + tick % 500,
                processor_queue_length_x100: u64::from(busy) / 100,
            },
            power: PowerSample::default(),
            memory: MemorySample {
                total_physical_bytes: 16 * 1024 * 1024 * 1024,
                available_physical_bytes: 8 * 1024 * 1024 * 1024,
                standby_cache_bytes: 2 * 1024 * 1024 * 1024,
                modified_page_list_bytes: 128 * 1024 * 1024,
                commit_bytes: 9 * 1024 * 1024 * 1024,
                commit_limit_bytes: 24 * 1024 * 1024 * 1024,
                hard_faults_per_sec: hard,
                soft_faults_per_sec: hard.saturating_mul(20),
                memory_load_percent: 50,
            },
            storage: vec![StorageQueueSample {
                device_id: "synthetic-0".into(),
                friendly_name: "Synthetic NVMe".into(),
                active_time_bp: busy / 2,
                queue_depth_x100: u64::from(busy),
                avg_transfer_latency_us: 120 + tick % 40,
                read_bytes_per_sec: 1_048_576,
                write_bytes_per_sec: 262_144,
                total_space_bytes: 2 * 1024 * 1024 * 1024 * 1024,
                free_space_bytes: 1_200 * 1024 * 1024 * 1024,
            }],
            gpu: GpuSample {
                adapter_id: "synthetic-gpu".into(),
                adapter_name: "Synthetic Adapter".into(),
                dedicated_used_bytes: 512 * 1024 * 1024,
                dedicated_total_bytes: 8 * 1024 * 1024 * 1024,
                shared_used_bytes: 0,
                engines: vec![GpuEngineSample {
                    engine_name: "3D".into(),
                    utilization_bp: busy,
                }],
                frametime_jitter_us: (tick % 7) * 100,
                compositor_lag_detected: false,
            },
            process_top: vec![ProcessCpuTopEntry {
                pid: 4242,
                name: "synthetic-workload".into(),
                cpu_busy_bp: busy,
                read_bytes_per_sec: 131_072,
                write_bytes_per_sec: 65_536,
                working_set_bytes: 256 * 1024 * 1024,
            }],
            collector_faults: Vec::new(),
        }
        .normalized()
    }
}

// ---------------------------------------------------------------------------
// Ring buffer + deterministic aggregation
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WindowAggregate {
    pub sample_count: u32,
    pub window_ms: u64,
    pub cpu_busy_bp_avg: u32,
    pub cpu_busy_bp_peak: u32,
    pub dpc_isr_busy_bp_avg: u32,
    pub hard_faults_per_sec_avg: u64,
    pub hard_faults_per_sec_peak: u64,
    pub commit_pressure_bp: u32,
    pub storage_active_bp_avg: u32,
    pub storage_active_bp_peak: u32,
    pub gpu_busy_bp_avg: u32,
}

struct RingState {
    samples: VecDeque<PerfSnapshot>,
    owner_principal_key: String,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

static PUBLISHED_TOTAL: AtomicU64 = AtomicU64::new(0);

/// Owner-scoped bounded sample ring plus lifecycle for the background sampler thread.
///
/// Concurrency model: one short-lived mutex guards the deque; the worker holds it for a single
/// push per tick. There is no lock-free trickery to audit — contention is negligible by design
/// because the producer runs at >= 250 ms cadence.
pub struct PerformanceRing {
    state: Arc<Mutex<RingState>>,
    /// Generation of the sampler that may run, `0` when stopped. A sampler thread
    /// exits as soon as this is not its own generation, so `stop()` + `start()`
    /// inside one interval cannot leave the old thread running beside the new one
    /// (a bool could not tell the two apart).
    running: Arc<AtomicU64>,
    generation: AtomicU64,
}

impl Default for PerformanceRing {
    fn default() -> Self {
        Self::new()
    }
}

impl PerformanceRing {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(RingState {
                samples: VecDeque::with_capacity(MAX_RING_SAMPLES),
                owner_principal_key: String::new(),
            })),
            running: Arc::new(AtomicU64::new(0)),
            generation: AtomicU64::new(0),
        }
    }

    pub fn is_active(&self) -> bool {
        self.running.load(Ordering::Acquire) != 0
    }

    /// Installs an owner and starts the background sampler. A second concurrent start is
    /// rejected; switching owners requires an explicit [`PerformanceRing::stop`] first.
    pub fn start(
        &self,
        platform: Arc<dyn PerfPlatform>,
        owner_principal_key: &str,
        requested_interval_ms: u32,
    ) -> Result<(), TelemetryError> {
        if owner_principal_key.trim().is_empty() {
            return Err(TelemetryError::NotActive);
        }
        // `+ 1` keeps 0 meaning "stopped". One atomic claim, not a check and a store.
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        if self
            .running
            .compare_exchange(0, generation, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(TelemetryError::AlreadyActive);
        }
        {
            let mut state = lock(&self.state);
            state.owner_principal_key = owner_principal_key.to_owned();
            state.samples.clear();
        }
        let interval = Duration::from_millis(u64::from(PerfSnapshot::clamped_interval_ms(
            requested_interval_ms,
        )));

        let running = self.running.clone();
        let state = self.state.clone();
        let spawned = thread::Builder::new()
            .name("aether-perf-sampler".into())
            .spawn(move || {
                while running.load(Ordering::Acquire) == generation {
                    let started = std::time::Instant::now();
                    let snapshot = platform.sample(interval).normalized();
                    {
                        let mut guard = lock(&state);
                        // Checked under the ring lock: a tick that finishes after
                        // stop() must not land in the next owner's ring.
                        if running.load(Ordering::Acquire) != generation {
                            break;
                        }
                        PUBLISHED_TOTAL.fetch_add(1, Ordering::Relaxed);
                        if guard.samples.len() >= MAX_RING_SAMPLES {
                            guard.samples.pop_front();
                        }
                        guard.samples.push_back(snapshot);
                    }
                    let elapsed = started.elapsed();
                    if elapsed < interval {
                        thread::sleep(interval - elapsed);
                    } else {
                        // Overrun: yield instead of spinning hot; next tick re-times itself.
                        thread::yield_now();
                    }
                }
            });
        if spawned.is_err() {
            let _ =
                self.running
                    .compare_exchange(generation, 0, Ordering::AcqRel, Ordering::Acquire);
            return Err(TelemetryError::NotActive);
        }
        Ok(())
    }

    pub fn stop(&self) {
        self.running.store(0, Ordering::Release);
    }

    /// Pushes one externally produced snapshot (service-driven mode / tests). Enforces the bound;
    /// drops oldest first. The first push binds the stream's owner; afterwards snapshots from a
    /// different principal are rejected.
    pub fn push(
        &self,
        owner_principal_key: &str,
        snapshot: PerfSnapshot,
    ) -> Result<(), TelemetryError> {
        let mut state = lock(&self.state);
        if state.owner_principal_key.is_empty() {
            state.owner_principal_key = owner_principal_key.to_owned();
        } else if state.owner_principal_key != owner_principal_key {
            return Err(TelemetryError::OwnerMismatch);
        }
        if state.samples.len() >= MAX_RING_SAMPLES {
            state.samples.pop_front();
        }
        state.samples.push_back(snapshot.normalized());
        PUBLISHED_TOTAL.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn ensure_owner(&self, owner_principal_key: &str) -> bool {
        lock(&self.state).owner_principal_key == owner_principal_key
    }

    pub fn len(&self, owner_principal_key: &str) -> usize {
        if !self.ensure_owner(owner_principal_key) {
            return 0;
        }
        lock(&self.state).samples.len()
    }

    pub fn is_empty(&self, owner_principal_key: &str) -> bool {
        self.len(owner_principal_key) == 0
    }

    /// Newest sample, cloned out under a short lock.
    pub fn latest(&self, owner_principal_key: &str) -> Option<PerfSnapshot> {
        let state = lock(&self.state);
        if state.owner_principal_key != owner_principal_key {
            return None;
        }
        state.samples.back().cloned()
    }

    /// Ordered copy of the ring for deterministic analysis (bounded by MAX_RING_SAMPLES).
    pub fn window(&self, owner_principal_key: &str) -> Vec<PerfSnapshot> {
        let state = lock(&self.state);
        if state.owner_principal_key != owner_principal_key {
            return Vec::new();
        }
        state.samples.iter().cloned().collect()
    }

    /// Deterministic aggregate over the current ring. An empty or foreign ring yields a zero
    /// aggregate with `sample_count == 0`; callers must treat that as insufficient evidence.
    pub fn aggregate(&self, owner_principal_key: &str) -> WindowAggregate {
        let state = lock(&self.state);
        let mut agg = WindowAggregate::default();
        if state.owner_principal_key != owner_principal_key || state.samples.is_empty() {
            return agg;
        }
        let count = state.samples.len() as u64;
        agg.sample_count = u32::try_from(count).unwrap_or(u32::MAX);
        // The `is_empty` guard above already returned, so both ends exist. Expressed as a
        // let-else rather than `expect`, so a future edit that moves the guard degrades to the
        // documented zero aggregate instead of panicking in the collector. P63.
        let (Some(front), Some(back)) = (state.samples.front(), state.samples.back()) else {
            return agg;
        };
        agg.window_ms = back
            .captured_unix_ms
            .saturating_sub(front.captured_unix_ms)
            .max(0) as u64;

        let mut cpu_sum = 0u64;
        let mut cpu_peak = 0u32;
        let mut dpc_sum = 0u64;
        let mut hard_sum = 0u64;
        let mut hard_peak = 0u64;
        let mut commit_bp_sum = 0u64;
        let mut store_sum = 0u64;
        let mut store_n = 0u64;
        let mut store_peak = 0u32;
        let mut gpu_sum = 0u64;
        let mut gpu_n = 0u64;
        for snap in &state.samples {
            cpu_sum = cpu_sum.saturating_add(u64::from(snap.cpu.total_busy_bp));
            cpu_peak = cpu_peak.max(snap.cpu.total_busy_bp);
            dpc_sum = dpc_sum.saturating_add(u64::from(snap.cpu.dpc_isr_busy_bp));
            hard_sum = hard_sum.saturating_add(snap.memory.hard_faults_per_sec);
            hard_peak = hard_peak.max(snap.memory.hard_faults_per_sec);
            if snap.memory.commit_limit_bytes > 0 {
                // Wide arithmetic: commit bytes can approach u64::MAX, so the basis-point ratio
                // is computed in u128 and clamped before narrowing.
                let committed = snap.memory.commit_bytes.min(snap.memory.commit_limit_bytes);
                let ratio = ((u128::from(committed) * 10_000)
                    / u128::from(snap.memory.commit_limit_bytes))
                .min(10_000);
                commit_bp_sum = commit_bp_sum.saturating_add(ratio as u64);
            }
            if !snap.storage.is_empty() {
                let peak_device = snap
                    .storage
                    .iter()
                    .map(|d| d.active_time_bp)
                    .max()
                    .unwrap_or(0);
                store_sum = store_sum.saturating_add(u64::from(peak_device));
                store_n += 1;
                store_peak = store_peak.max(peak_device);
            }
            if let Some(engine) = snap.gpu.engines.first() {
                gpu_sum = gpu_sum.saturating_add(u64::from(engine.utilization_bp));
                gpu_n += 1;
            }
        }
        agg.cpu_busy_bp_avg = (cpu_sum / count) as u32;
        agg.cpu_busy_bp_peak = cpu_peak;
        agg.dpc_isr_busy_bp_avg = (dpc_sum / count) as u32;
        agg.hard_faults_per_sec_avg = hard_sum / count;
        agg.hard_faults_per_sec_peak = hard_peak;
        agg.commit_pressure_bp = (commit_bp_sum / count) as u32;
        if let Some(avg) = store_sum.checked_div(store_n) {
            agg.storage_active_bp_avg = avg as u32;
        }
        agg.storage_active_bp_peak = store_peak;
        if let Some(avg) = gpu_sum.checked_div(gpu_n) {
            agg.gpu_busy_bp_avg = avg as u32;
        }
        agg
    }
}

pub fn published_total() -> u64 {
    PUBLISHED_TOTAL.load(Ordering::Relaxed)
}
