//! Phase 20 — protocol mappings between the performance domain crates and the v7 wire contract.
//!
//! Every mapping is lossless for the fields the renderer consumes and bounded by the same
//! capacity rules enforced in `performance-telemetry`.

use aethercore_contracts::v1;
use aethercore_performance_bottleneck::{
    Confidence as BottleneckConfidence, Finding, Report, Role,
};
use aethercore_performance_optimization::{ActionKind, Plan, Reversibility};
use aethercore_performance_telemetry::{
    CollectorFault, CpuSample, GpuEngineSample, GpuSample, MemorySample, PerfSnapshot, PowerSample,
    ProcessCpuTopEntry, StorageQueueSample, ThermalThrottleReason,
};

pub(crate) fn thermal_reason_proto(value: ThermalThrottleReason) -> v1::ThermalThrottleReason {
    match value {
        ThermalThrottleReason::Unspecified => v1::ThermalThrottleReason::Unspecified,
        ThermalThrottleReason::None => v1::ThermalThrottleReason::None,
        ThermalThrottleReason::Thermal => v1::ThermalThrottleReason::Thermal,
        ThermalThrottleReason::Power => v1::ThermalThrottleReason::Power,
        ThermalThrottleReason::Vrm => v1::ThermalThrottleReason::Vrm,
        ThermalThrottleReason::Current => v1::ThermalThrottleReason::Current,
    }
}

pub(crate) fn cpu_sample_proto(value: &CpuSample) -> v1::CpuSample {
    v1::CpuSample {
        per_processor_busy_bp: value.per_processor_busy_bp.clone(),
        total_busy_bp: value.total_busy_bp,
        dpc_isr_busy_bp: value.dpc_isr_busy_bp,
        context_switches_per_sec: value.context_switches_per_sec,
        processor_queue_length_x100: value.processor_queue_length_x100,
    }
}

pub(crate) fn power_sample_proto(value: &PowerSample) -> v1::PowerSample {
    v1::PowerSample {
        throttle_active: value.throttle_active,
        throttle_reason: thermal_reason_proto(value.throttle_reason) as i32,
        limit_reasons_raw: value.limit_reasons_raw,
        temperature_c: value.temperature_c,
        has_temperature: value.has_temperature,
    }
}

pub(crate) fn memory_sample_proto(value: &MemorySample) -> v1::MemorySample {
    v1::MemorySample {
        total_physical_bytes: value.total_physical_bytes,
        available_physical_bytes: value.available_physical_bytes,
        standby_cache_bytes: value.standby_cache_bytes,
        modified_page_list_bytes: value.modified_page_list_bytes,
        commit_bytes: value.commit_bytes,
        commit_limit_bytes: value.commit_limit_bytes,
        hard_faults_per_sec: value.hard_faults_per_sec,
        soft_faults_per_sec: value.soft_faults_per_sec,
        memory_load_percent: value.memory_load_percent,
    }
}

pub(crate) fn storage_proto(value: &[StorageQueueSample]) -> Vec<v1::StorageQueueSample> {
    value
        .iter()
        .map(|device| v1::StorageQueueSample {
            device_id: device.device_id.clone(),
            friendly_name: device.friendly_name.clone(),
            active_time_bp: device.active_time_bp,
            queue_depth_x100: device.queue_depth_x100,
            avg_transfer_latency_us: device.avg_transfer_latency_us,
            read_bytes_per_sec: device.read_bytes_per_sec,
            write_bytes_per_sec: device.write_bytes_per_sec,
            total_space_bytes: device.total_space_bytes,
            free_space_bytes: device.free_space_bytes,
        })
        .collect()
}

pub(crate) fn gpu_sample_proto(value: &GpuSample) -> v1::GpuSample {
    v1::GpuSample {
        adapter_id: value.adapter_id.clone(),
        adapter_name: value.adapter_name.clone(),
        dedicated_used_bytes: value.dedicated_used_bytes,
        dedicated_total_bytes: value.dedicated_total_bytes,
        shared_used_bytes: value.shared_used_bytes,
        engines: value
            .engines
            .iter()
            .map(|engine: &GpuEngineSample| v1::GpuEngineSample {
                engine_name: engine.engine_name.clone(),
                utilization_bp: engine.utilization_bp,
            })
            .collect(),
        frametime_jitter_us: value.frametime_jitter_us,
        compositor_lag_detected: value.compositor_lag_detected,
    }
}

pub(crate) fn process_top_proto(value: &[ProcessCpuTopEntry]) -> Vec<v1::ProcessCpuTopEntry> {
    value
        .iter()
        .map(|entry| v1::ProcessCpuTopEntry {
            pid: entry.pid,
            name: entry.name.clone(),
            cpu_busy_bp: entry.cpu_busy_bp,
            read_bytes_per_sec: entry.read_bytes_per_sec,
            write_bytes_per_sec: entry.write_bytes_per_sec,
            working_set_bytes: entry.working_set_bytes,
        })
        .collect()
}

pub(crate) fn faults_proto(value: &[CollectorFault]) -> Vec<v1::PerfCollectorFault> {
    value
        .iter()
        .map(|fault| v1::PerfCollectorFault {
            collector: fault.collector.clone(),
            kind: fault.kind.clone(),
            detail: fault.detail.clone(),
        })
        .collect()
}

pub(crate) fn perf_snapshot_proto(value: &PerfSnapshot) -> v1::PerfSnapshot {
    v1::PerfSnapshot {
        captured_unix_ms: value.captured_unix_ms,
        interval_ms: value.interval_ms,
        cpu: Some(cpu_sample_proto(&value.cpu)),
        power: Some(power_sample_proto(&value.power)),
        memory: Some(memory_sample_proto(&value.memory)),
        storage: storage_proto(&value.storage),
        gpu: Some(gpu_sample_proto(&value.gpu)),
        process_top: process_top_proto(&value.process_top),
        collector_faults: faults_proto(&value.collector_faults),
    }
}

pub(crate) fn role_proto(value: Role) -> v1::BottleneckRole {
    match value {
        Role::Unspecified => v1::BottleneckRole::Unspecified,
        Role::RootCause => v1::BottleneckRole::RootCause,
        Role::ContributingCondition => v1::BottleneckRole::ContributingCondition,
        Role::Symptom => v1::BottleneckRole::Symptom,
    }
}

pub(crate) fn confidence_proto(value: BottleneckConfidence) -> v1::EvidenceConfidenceP20 {
    match value {
        BottleneckConfidence::Low => v1::EvidenceConfidenceP20::Low,
        BottleneckConfidence::Medium => v1::EvidenceConfidenceP20::Medium,
        BottleneckConfidence::High => v1::EvidenceConfidenceP20::High,
        BottleneckConfidence::Confirmed => v1::EvidenceConfidenceP20::Confirmed,
    }
}

pub(crate) fn finding_proto(value: &Finding) -> v1::BottleneckFinding {
    v1::BottleneckFinding {
        id: value.id.clone(),
        code: value.code.clone(),
        role: role_proto(value.role) as i32,
        confidence: confidence_proto(value.confidence) as i32,
        caused_by_finding_ids: value.caused_by_finding_ids.clone(),
        title_key: value.title_key.clone(),
        summary_key: value.summary_key.clone(),
        message_args: value
            .message_args
            .iter()
            .map(|arg| v1::PerfMessageArg {
                key: arg.key.clone(),
                value: arg.value.clone(),
            })
            .collect(),
        evidence: value
            .evidence
            .iter()
            .map(|ev| v1::BottleneckEvidenceRef {
                fact_key: ev.fact_key.clone(),
                observed_value: ev.observed_value,
                threshold: ev.threshold,
                observed_unix_ms: ev.observed_unix_ms,
            })
            .collect(),
        applicable_action_kinds: value.applicable_action_kinds.clone(),
        first_observed_unix_ms: value.first_observed_unix_ms,
        last_observed_unix_ms: value.last_observed_unix_ms,
    }
}

pub(crate) fn bottleneck_report_proto(value: &Report) -> v1::BottleneckReport {
    v1::BottleneckReport {
        report_id: value.report_id.clone(),
        generated_unix_ms: value.generated_unix_ms,
        analyzed_sample_count: value.analyzed_sample_count,
        analysis_window_ms: u32::try_from(value.analysis_window_ms).unwrap_or(u32::MAX),
        findings: value.findings.iter().map(finding_proto).collect(),
        digest_sha256: value.digest_sha256.clone(),
        rule_engine_version: value.rule_engine_version.clone(),
    }
}

fn reversibility_proto(value: Reversibility) -> v1::ReversibilityKind {
    match value {
        Reversibility::Unspecified => v1::ReversibilityKind::Unspecified,
        Reversibility::AutomaticRestore => v1::ReversibilityKind::AutomaticRestore,
        Reversibility::SessionOnly => v1::ReversibilityKind::SessionOnly,
        Reversibility::ManualReview => v1::ReversibilityKind::ManualReview,
    }
}

fn action_kind_from_str(value: &str) -> v1::OptimizationActionKind {
    // ActionKind::parse is crate-private; mirror its exact string mapping here.
    let parsed = match value {
        "ecoQos" => Some(ActionKind::EcoQos),
        "backgroundPriority" => Some(ActionKind::BackgroundPriority),
        "cooperativeTrimRequest" => Some(ActionKind::CooperativeTrimRequest),
        "gameModeProfile" => Some(ActionKind::GameModeProfile),
        _ => None,
    };
    match parsed {
        Some(ActionKind::EcoQos) | None if value == "ecoQos" => v1::OptimizationActionKind::EcoQos,
        _ if value == "backgroundPriority" => v1::OptimizationActionKind::BackgroundPriority,
        _ if value == "cooperativeTrimRequest" => {
            v1::OptimizationActionKind::CooperativeTrimRequest
        }
        _ if value == "gameModeProfile" => v1::OptimizationActionKind::GameModeProfile,
        _ => v1::OptimizationActionKind::Unspecified,
    }
}

pub(crate) fn plan_proto(value: &Plan) -> v1::OptimizationPlanSnapshot {
    v1::OptimizationPlanSnapshot {
        plan_id: value.plan_id.clone(),
        report_id: value.report_id.clone(),
        digest_sha256: value.digest_sha256.clone(),
        created_unix_ms: value.created_unix_ms,
        immutable: value.immutable,
        candidates: value
            .candidates
            .iter()
            .map(|candidate| v1::OptimizationCandidate {
                candidate_id: candidate.candidate_id.clone(),
                kind: action_kind_from_str(candidate.kind.as_str()) as i32,
                finding_ids: candidate.finding_ids.clone(),
                title_key: candidate.title_key.clone(),
                description_key: candidate.description_key.clone(),
                reversibility: reversibility_proto(candidate.reversibility) as i32,
                expected_effect_metric_keys: candidate.expected_effect_metric_keys.clone(),
                target_pids: candidate
                    .target_process_keys
                    .iter()
                    .map(|key| key.pid)
                    .collect(),
                requires_explicit_consent: candidate.requires_explicit_consent,
            })
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// Service-side performance engine (Domain A+B+C composition)
// ---------------------------------------------------------------------------

use std::collections::BTreeMap;
use std::sync::Arc;

use aethercore_operation_kernel::MutationSupervisor;
use aethercore_performance_optimization::OptimizationGovernor;

/// Owns the sampling ring and the optimization governor. Sampling is passive: the engine only
/// runs while an owner has explicitly started it, and every analysis is computed on demand from
/// the bounded ring rather than on a timer.
pub struct PerformanceEngine {
    ring: aethercore_performance_telemetry::PerformanceRing,
    platform: Arc<dyn aethercore_performance_telemetry::PerfPlatform>,
    governor: OptimizationGovernor,
}

/// Phase 27 (T5): honest engine-source label surfaced to the renderer. The synthetic
/// platform is used ONLY when explicitly requested (audits/tests/offline UI); native
/// builds report "native" per the cfg-selected provider.
pub(crate) fn engine_source() -> &'static str {
    #[cfg(feature = "force-synthetic-perf")]
    {
        "synthetic"
    }
    #[cfg(not(feature = "force-synthetic-perf"))]
    {
        if cfg!(windows) || cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            "native"
        } else {
            "synthetic"
        }
    }
}

impl PerformanceEngine {
    pub fn new(
        platform: Arc<dyn aethercore_performance_telemetry::PerfPlatform>,
        mutations: MutationSupervisor,
    ) -> Self {
        Self {
            ring: aethercore_performance_telemetry::PerformanceRing::new(),
            platform,
            governor: OptimizationGovernor::new(
                Arc::new(aethercore_performance_optimization::NoopPlatform::new()),
                mutations,
            ),
        }
    }

    pub fn start_sampling(&self, owner: &str, interval_ms: u32) -> Result<(), String> {
        self.ring
            .start(self.platform.clone(), owner, interval_ms)
            .map_err(|error| error.to_string())
    }

    pub fn stop_sampling(&self) {
        self.ring.stop();
    }

    pub fn latest(&self, owner: &str) -> Option<aethercore_performance_telemetry::PerfSnapshot> {
        self.ring.latest(owner)
    }

    /// Phase 53 (DBT-P50-005): ordered performance history window for UI sparklines.
    pub fn window(&self, owner: &str, max_samples: u32) -> v1::PerformanceWindowResponse {
        let max = if max_samples == 0 {
            aethercore_performance_telemetry::MAX_RING_SAMPLES
        } else {
            (max_samples as usize).clamp(1, aethercore_performance_telemetry::MAX_RING_SAMPLES)
        };
        let mut samples = self.ring.window(owner);
        if samples.len() > max {
            let start = samples.len() - max;
            samples = samples.split_off(start);
        }
        v1::PerformanceWindowResponse {
            samples: samples.iter().map(perf_snapshot_proto).collect(),
        }
    }

    /// Ensures at least one sample exists so snapshot requests are meaningful even before the
    /// background sampler's first tick.
    /// Ensures the owner's ring holds a reading that is actually current.
    ///
    /// **DBT-P42-008.** This read `if self.ring.latest(owner).is_none()`, so the
    /// ring was seeded exactly once per owner per service lifetime and every
    /// later `perf snapshot` returned that first reading unchanged. Measured on
    /// this machine: three calls two seconds apart all returned
    /// `capturedUnixMs 1788349015839`, byte-identical to the reading §41.15
    /// recorded **three hours earlier**, from a service that had been up since
    /// 12:09:29. The background sampler only runs after an explicit `perf start`,
    /// so for the common case nothing ever refreshed it.
    ///
    /// The value returned was real; it was simply not a measurement of *now*,
    /// which is the same class of defect as DBT-P41-002 — presenting something
    /// that is not a current reading as if it were. A live sampler running at
    /// this cadence keeps the ring fresh and this adds no extra tick.
    pub fn ensure_sample(&self, owner: &str, interval_ms: u32) {
        let latest = self.ring.latest(owner).map(|s| s.captured_unix_ms);
        if sample_is_stale(latest, chrono::Utc::now().timestamp_millis(), interval_ms) {
            let snap = self
                .platform
                .sample(std::time::Duration::from_millis(u64::from(interval_ms)));
            let _ = self.ring.push(owner, snap);
        }
    }

    pub fn analyze(
        &self,
        owner: &str,
    ) -> Option<(
        aethercore_performance_bottleneck::Report,
        aethercore_performance_telemetry::WindowAggregate,
    )> {
        let aggregate = self.ring.aggregate(owner);
        if aggregate.sample_count == 0 {
            return None;
        }
        let window = self.ring.window(owner);
        let now = chrono::Utc::now().timestamp_millis();
        Some((
            aethercore_performance_bottleneck::analyze(&aggregate, &window, now),
            aggregate,
        ))
    }

    pub fn create_plan(
        &self,
        report: &aethercore_performance_bottleneck::Report,
        selected: &[String],
        offenders: &BTreeMap<String, Vec<aethercore_performance_optimization::ProcessKey>>,
    ) -> Result<
        aethercore_performance_optimization::Plan,
        aethercore_performance_optimization::OptimizationError,
    > {
        self.governor.create_plan(report, selected, offenders)
    }
}

/// One passive sample, reduced to what the collectors measured.
///
/// The `capabilities` verb reports through this so it cannot answer `native` for
/// a subsystem whose collector declared a fault or returned nothing — the
/// contradiction §41.15 3.C measured (16/16 `native` beside `"storage": []`).
pub(crate) fn observe_telemetry() -> aethercore_platform_capabilities::TelemetryObservation {
    let interval =
        std::time::Duration::from_millis(aethercore_performance_telemetry::MIN_INTERVAL_MS as u64);
    let measured = aethercore_performance_telemetry::default_platform()
        .sample(interval)
        .measured_subsystems();
    aethercore_platform_capabilities::TelemetryObservation {
        cpu: measured.cpu,
        memory: measured.memory,
        storage: measured.storage,
        gpu: measured.gpu,
    }
}

/// Freshness predicate for [`PerformanceEngine::ensure_sample`], extracted so the
/// DBT-P42-008 condition is testable without a service, a ring or a clock.
///
/// A reading is stale when there is none, when it is older than the requested
/// interval, or when it is dated in the future (a clock step must not pin a
/// stale reading in place forever).
pub(crate) fn sample_is_stale(
    latest_captured_unix_ms: Option<i64>,
    now_unix_ms: i64,
    interval_ms: u32,
) -> bool {
    match latest_captured_unix_ms {
        None => true,
        Some(captured) => {
            let age_ms = now_unix_ms - captured;
            age_ms < 0 || age_ms > i64::from(interval_ms)
        }
    }
}

#[cfg(test)]
mod dbt_p42_008 {
    use super::sample_is_stale;

    #[test]
    fn an_empty_ring_is_stale() {
        assert!(sample_is_stale(None, 1_788_359_855_093, 1_000));
    }

    /// The measured case: the reading §41.15 recorded, served three hours later.
    #[test]
    fn a_three_hour_old_reading_is_not_a_current_measurement() {
        assert!(
            sample_is_stale(Some(1_788_349_015_839), 1_788_359_855_093, 1_000),
            "a reading 10_839_254 ms old was being returned as a live snapshot"
        );
    }

    #[test]
    fn a_reading_inside_the_requested_interval_is_reused() {
        let now = 1_788_359_855_093;
        assert!(!sample_is_stale(Some(now), now, 1_000));
        assert!(!sample_is_stale(Some(now - 999), now, 1_000));
        assert!(sample_is_stale(Some(now - 1_001), now, 1_000));
    }

    /// A clock step backwards must not pin a stale reading in place.
    #[test]
    fn a_future_dated_reading_is_stale() {
        let now = 1_788_359_855_093;
        assert!(sample_is_stale(Some(now + 60_000), now, 1_000));
    }
}

#[cfg(test)]
mod dbt_p50_005 {
    use super::*;
    use aethercore_operation_kernel::MutationSupervisor;
    use aethercore_performance_telemetry::SyntheticPerfPlatform;
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn window_returns_ordered_samples_bounded_by_max() {
        let platform = Arc::new(SyntheticPerfPlatform::new());
        let supervisor = MutationSupervisor::new();
        let engine = PerformanceEngine::new(platform, supervisor);
        let owner = "test-principal-p53";

        for _ in 0..10 {
            let snap = engine.platform.sample(Duration::from_millis(1000));
            engine.ring.push(owner, snap).expect("push sample");
        }

        let resp_all = engine.window(owner, 0);
        assert_eq!(resp_all.samples.len(), 10);

        let resp_clamped = engine.window(owner, 4);
        assert_eq!(resp_clamped.samples.len(), 4);

        // Different owner principal isolates samples
        let resp_other = engine.window("foreign-principal", 10);
        assert_eq!(resp_other.samples.len(), 0);
    }
}
