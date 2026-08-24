//! Phase 20 — protocol mappings between the performance domain crates and the v7 wire contract.
//!
//! Every mapping is lossless for the fields the renderer consumes and bounded by the same
//! capacity rules enforced in `performance-telemetry`.

use aethercore_contracts::v1;
use aethercore_performance_bottleneck::{
    Confidence as BottleneckConfidence, Finding, Report, Role,
};
use aethercore_performance_optimization::{
    ActionKind, ExecutionItem, ExecutionStatus, Plan, Reversibility,
};
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
            .map(|arg| v1::PerfMessageArg { key: arg.key.clone(), value: arg.value.clone() })
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
        analysis_window_ms: value.analysis_window_ms,
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
    match ActionKind::parse(value) {
        Some(ActionKind::EcoQos) | None if value == "ecoQos" => v1::OptimizationActionKind::EcoQos,
        _ if value == "backgroundPriority" => v1::OptimizationActionKind::BackgroundPriority,
        _ if value == "cooperativeTrimRequest" => v1::OptimizationActionKind::CooperativeTrimRequest,
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
                target_pids: candidate.target_process_keys.iter().map(|key| key.pid).collect(),
                requires_explicit_consent: candidate.requires_explicit_consent,
            })
            .collect(),
    }
}

pub(crate) fn optimization_status_proto(value: &ExecutionStatus) -> v1::OptimizationStatus {
    v1::OptimizationStatus {
        plan_id: value.plan_id.clone(),
        plan_state: value.plan_state.clone(),
        stage: value.stage.clone(),
        progress_known: value.progress_known,
        overall_percent: value.overall_percent,
        current_candidate_id: value.current_candidate_id.clone(),
        detail: value.detail.clone(),
        mutation_started: value.mutation_started,
        recovery_required: value.recovery_required,
        failure_message: value.failure_message.clone(),
        started_unix_ms: value.started_unix_ms,
        updated_unix_ms: value.updated_unix_ms,
        completed_unix_ms: value.completed_unix_ms,
        items: value
            .items
            .iter()
            .map(|item: &ExecutionItem| v1::OptimizationExecutionItem {
                candidate_id: item.candidate_id.clone(),
                stage: item.stage.clone(),
                result_code: item.result_code.clone(),
                detail: item.detail.clone(),
                verified: item.verified,
            })
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// Service-side performance engine (Domain A+B+C composition)
// ---------------------------------------------------------------------------

use std::collections::BTreeMap;
use std::sync::Arc;

use aethercore_collector_runtime::CommitFence;
use aethercore_operation_kernel::MutationSupervisor;
use aethercore_performance_optimization::{ExecutionStatus as OptStatus, OptimizationGovernor};

/// Owns the sampling ring and the optimization governor. Sampling is passive: the engine only
/// runs while an owner has explicitly started it, and every analysis is computed on demand from
/// the bounded ring rather than on a timer.
pub struct PerformanceEngine {
    ring: aethercore_performance_telemetry::PerformanceRing,
    platform: Arc<dyn aethercore_performance_telemetry::PerfPlatform>,
    governor: OptimizationGovernor,
}

impl PerformanceEngine {
    pub fn new(platform: Arc<dyn aethercore_performance_telemetry::PerfPlatform>, mutations: MutationSupervisor) -> Self {
        Self {
            ring: aethercore_performance_telemetry::PerformanceRing::new(),
            platform,
            governor: OptimizationGovernor::new(
                Arc::new(aethercore_performance_optimization::NoopPlatform::new()),
                mutations,
            ),
        }
    }

    #[cfg(not(windows))]
    pub fn with_synthetic(mutations: MutationSupervisor) -> Self {
        Self::new(
            Arc::new(aethercore_performance_telemetry::SyntheticPerfPlatform::new()),
            mutations,
        )
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

    /// Ensures at least one sample exists so snapshot requests are meaningful even before the
    /// background sampler's first tick.
    pub fn ensure_sample(&self, owner: &str, interval_ms: u32) {
        if self.ring.latest(owner).is_none() {
            let snap = self.platform.sample(std::time::Duration::from_millis(u64::from(interval_ms)));
            let _ = self.ring.push(owner, snap);
        }
    }

    pub fn analyze(&self, owner: &str) -> Option<(aethercore_performance_bottleneck::Report, aethercore_performance_telemetry::WindowAggregate)> {
        let aggregate = self.ring.aggregate(owner);
        if aggregate.sample_count == 0 {
            return None;
        }
        let window = self.ring.window(owner);
        let now = chrono::Utc::now().timestamp_millis();
        Some((aethercore_performance_bottleneck::analyze(&aggregate, &window, now), aggregate))
    }

    pub fn create_plan(
        &self,
        report: &aethercore_performance_bottleneck::Report,
        selected: &[String],
        offenders: &BTreeMap<String, Vec<aethercore_performance_optimization::ProcessKey>>,
    ) -> Result<aethercore_performance_optimization::Plan, aethercore_performance_optimization::OptimizationError> {
        self.governor.create_plan(report, selected, offenders)
    }

    pub fn start_optimization(
        &self,
        owner: &str,
        plan: &aethercore_performance_optimization::Plan,
        fence: CommitFence,
    ) -> Result<OptStatus, aethercore_performance_optimization::OptimizationError> {
        self.governor.start(owner, plan, fence)
    }

    pub fn optimization_status(&self, plan_id: &str) -> Option<OptStatus> {
        self.governor.status(plan_id)
    }
}
