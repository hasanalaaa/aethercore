//! Phase 20 Domain B — bottleneck diagnosis & causal attribution engine.
//!
//! Every finding is produced by a deterministic rule over a bounded evidence window. Rules
//! classify each observation as [`Role::RootCause`], [`Role::ContributingCondition`], or
//! [`Role::Symptom`] and must cite concrete measured facts (value + threshold + time). A rule
//! that cannot gather its full evidence matrix emits nothing — speculation is a defect here,
//! not a feature.
//!
//! Causality is expressed as `caused_by` edges between finding ids, computed *after* the fact
//! set exists, so the graph is acyclic by construction (rules only ever point at strictly
//! lower-or-equal rule tiers).
#![deny(unsafe_op_in_unsafe_fn)]

use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub use aethercore_performance_telemetry::{PerfSnapshot, WindowAggregate};

/// Version stamped into every report; bumps whenever thresholds change so historical digests
/// stay interpretable.
pub const RULE_ENGINE_VERSION: &str = "p20.bottleneck-rules.v1";

// ---------------------------------------------------------------------------
// Thresholds — all in one place, documented, unit-testable
// ---------------------------------------------------------------------------

pub mod thresholds {
    /// Sustained CPU saturation (avg busy basis points).
    pub const CPU_SATURATION_AVG_BP: u32 = 9_000;
    /// CPU considered "elevated enough to matter" for contributing conditions.
    pub const CPU_ELEVATED_BP: u32 = 7_000;
    /// DPC/ISR share above which deferred procedure calls are blamed for latency.
    pub const DPC_ISR_PRESSURE_BP: u32 = 2_500;
    /// Hard-fault rate (per second) indicating standby-cache starvation.
    pub const HARD_FAULT_STARVATION_PER_SEC: u64 = 500;
    /// Modified-list growth (bytes) beyond which the working-set trim policy is suspect.
    pub const MODIFIED_LIST_GROWTH_BYTES: u64 = 512 * 1024 * 1024;
    /// Commit charge pressure (basis points of the commit limit).
    pub const COMMIT_PRESSURE_BP: u32 = 8_500;
    /// Peak storage active time for an I/O-bound verdict.
    pub const STORAGE_SATURATION_PEAK_BP: u32 = 9_200;
    /// Average storage active time for sustained I/O saturation.
    pub const STORAGE_SATURATION_AVG_BP: u32 = 7_500;
    /// Transfer latency (microseconds) above which queueing is user-visible.
    pub const TRANSFER_LATENCY_US: u64 = 25_000;
    /// GPU 3D-engine utilization treated as "the workload owns the GPU".
    pub const GPU_SATURATION_BP: u32 = 9_400;
    /// Frame-time jitter (microseconds stddev) above which compositor lag is plausible.
    pub const FRAMETIME_JITTER_US: u64 = 3_000;
    /// Minimum samples before any rule may fire at CONFIRMED confidence.
    pub const MIN_SAMPLES_FOR_CONFIRMED: u32 = 10;
    /// Minimum samples for any finding at all (below this the report is empty).
    pub const MIN_SAMPLES_FOR_FINDINGS: u32 = 5;
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    #[default]
    Unspecified,
    RootCause,
    ContributingCondition,
    Symptom,
}

impl Role {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unspecified => "unspecified",
            Self::RootCause => "rootCause",
            Self::ContributingCondition => "contributingCondition",
            Self::Symptom => "symptom",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum Confidence {
    #[default]
    Low,
    Medium,
    High,
    Confirmed,
}

/// One measured anomaly. `observed` and `threshold` are always in the same unit.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceRef {
    pub fact_key: String,
    pub observed_value: f64,
    pub threshold: f64,
    pub observed_unix_ms: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MessageArg {
    pub key: String,
    pub value: String,
}

/// Rule tier constrains causality direction: a finding may only depend on findings from
/// strictly higher tiers (lower ordinal = more fundamental).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Tier {
    Hardware = 0,
    Resource = 1,
    Workload = 2,
}

struct RuleOutput {
    code: &'static str,
    tier: Tier,
    role: Role,
    confidence: Confidence,
    title_key: &'static str,
    summary_key: &'static str,
    args: Vec<MessageArg>,
    evidence: Vec<EvidenceRef>,
    depends_on: Vec<&'static str>,
    applicable_actions: Vec<&'static str>,
}

/// A produced finding with stable identity.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub id: String,
    pub code: String,
    pub role: Role,
    pub confidence: Confidence,
    pub caused_by_finding_ids: Vec<String>,
    pub title_key: String,
    pub summary_key: String,
    pub message_args: Vec<MessageArg>,
    pub evidence: Vec<EvidenceRef>,
    pub applicable_action_kinds: Vec<String>,
    pub first_observed_unix_ms: i64,
    pub last_observed_unix_ms: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub report_id: String,
    pub generated_unix_ms: i64,
    pub analyzed_sample_count: u32,
    pub analysis_window_ms: u64,
    pub findings: Vec<Finding>,
    pub digest_sha256: String,
    pub rule_engine_version: String,
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} findings={}", self.report_id, self.findings.len())
    }
}

// ---------------------------------------------------------------------------
// Analysis entrypoint
// ---------------------------------------------------------------------------

/// Analyzes an ordered sample window plus its precomputed aggregate. Deterministic: identical
/// inputs yield byte-identical reports including the digest.
pub fn analyze(aggregate: &WindowAggregate, window: &[PerfSnapshot], now_unix_ms: i64) -> Report {
    let mut outputs: Vec<RuleOutput> = Vec::new();

    // Guard: insufficient evidence produces an honest empty report, never guesses.
    if aggregate.sample_count < thresholds::MIN_SAMPLES_FOR_FINDINGS {
        return finalize(Vec::new(), aggregate, now_unix_ms);
    }

    if let Some(out) = cpu_saturation(aggregate, window) {
        outputs.push(out);
    }
    if let Some(out) = dpc_pressure(aggregate, window) {
        outputs.push(out);
    }
    if let Some(out) = thermal_clamp(window) {
        outputs.push(out);
    }
    if let Some(out) = standby_starvation(aggregate, window) {
        outputs.push(out);
    }
    if let Some(out) = commit_pressure(aggregate, window) {
        outputs.push(out);
    }
    if let Some(out) = io_saturation(aggregate, window) {
        outputs.push(out);
    }
    if let Some(out) = gpu_bound(window) {
        outputs.push(out);
    }
    if let Some(out) = working_set_bloat(window) {
        outputs.push(out);
    }

    finalize(outputs, aggregate, now_unix_ms)
}

fn finalize(mut outputs: Vec<RuleOutput>, aggregate: &WindowAggregate, now_unix_ms: i64) -> Report {
    // Stable order: tier asc, then code asc. IDs derive from codes => deterministic.
    outputs.sort_by(|a, b| {
        format!("{}{}", a.tier as u8, a.code).cmp(&format!("{}{}", b.tier as u8, b.code))
    });

    let mut findings: Vec<Finding> = Vec::new();
    for output in &outputs {
        let id = finding_id(output.code);
        findings.push(Finding {
            id: id.clone(),
            code: output.code.to_string(),
            role: output.role,
            confidence: output.confidence,
            caused_by_finding_ids: Vec::new(),
            title_key: output.title_key.to_string(),
            summary_key: output.summary_key.to_string(),
            message_args: output.args.clone(),
            evidence: output.evidence.clone(),
            applicable_action_kinds: output
                .applicable_actions
                .iter()
                .map(|s| s.to_string())
                .collect(),
            first_observed_unix_ms: output
                .evidence
                .first()
                .map(|e| e.observed_unix_ms)
                .unwrap_or(now_unix_ms),
            last_observed_unix_ms: output
                .evidence
                .last()
                .map(|e| e.observed_unix_ms)
                .unwrap_or(now_unix_ms),
        });
        let _ = aggregate;
    }

    // Resolve causal edges after all ids exist; unknown dependencies are dropped rather than
    // left dangling. Tier ordering guarantees acyclicity. The map owns its strings so `findings`
    // can be mutably borrowed while dependencies resolve.
    let code_to_id: std::collections::HashMap<String, String> = findings
        .iter()
        .map(|f| (f.code.clone(), f.id.clone()))
        .collect();
    for output in &outputs {
        let Some(finding) = findings.iter_mut().find(|f| f.code == output.code) else {
            continue;
        };
        for dep in &output.depends_on {
            let dep_code = (*dep).to_string();
            if let Some(dep_id) = code_to_id.get(&dep_code) {
                finding.caused_by_finding_ids.push(dep_id.clone());
            }
        }
        finding.caused_by_finding_ids.sort();
        finding.caused_by_finding_ids.dedup();
    }

    let mut report = Report {
        report_id: String::new(),
        generated_unix_ms: now_unix_ms,
        analyzed_sample_count: aggregate.sample_count,
        analysis_window_ms: aggregate.window_ms,
        findings,
        digest_sha256: String::new(),
        rule_engine_version: RULE_ENGINE_VERSION.to_string(),
    };
    report.digest_sha256 = compute_digest(&report);
    report.report_id = format!(
        "bottleneck-{}",
        &report.digest_sha256[..12.min(report.digest_sha256.len())]
    );
    report
}

fn finding_id(code: &str) -> String {
    format!("finding:{}", code.to_ascii_lowercase())
}

fn compute_digest(report: &Report) -> String {
    let mut hasher = Sha256::new();
    hasher.update(report.rule_engine_version.as_bytes());
    hasher.update(report.analyzed_sample_count.to_le_bytes());
    hasher.update(report.analysis_window_ms.to_le_bytes());
    for finding in &report.findings {
        hasher.update(finding.id.as_bytes());
        hasher.update(finding.role.as_str().as_bytes());
        hasher.update(format!("{:?}", finding.confidence).as_bytes());
        for arg in &finding.message_args {
            hasher.update(arg.key.as_bytes());
            hasher.update(arg.value.as_bytes());
        }
        for ev in &finding.evidence {
            hasher.update(ev.fact_key.as_bytes());
            hasher.update(ev.observed_value.to_le_bytes());
            hasher.update(ev.threshold.to_le_bytes());
        }
    }
    hex_lower(&hasher.finalize())
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

// ---------------------------------------------------------------------------
// Evidence helpers
// ---------------------------------------------------------------------------

fn evidence(key: &str, observed: u64, threshold: u64, at: i64) -> EvidenceRef {
    EvidenceRef {
        fact_key: key.to_string(),
        observed_value: observed as f64,
        threshold: threshold as f64,
        observed_unix_ms: at,
    }
}

fn peak_of(window: &[PerfSnapshot], pick: impl Fn(&PerfSnapshot) -> u64) -> Option<(u64, i64)> {
    window
        .iter()
        .map(|snap| (pick(snap), snap.captured_unix_ms))
        .max_by_key(|(value, _)| *value)
}

/// Peak over the snapshots that actually reported the value.
///
/// DBT-P46-B21: `pick` returning `None` means that snapshot measured nothing,
/// which is not the same as measuring zero — a snapshot with no storage device
/// or no GPU engine must neither supply an evidence chip nor lend its timestamp
/// to one. `None` when no snapshot in the window reported it at all.
fn peak_of_reported(
    window: &[PerfSnapshot],
    pick: impl Fn(&PerfSnapshot) -> Option<u64>,
) -> Option<(u64, i64)> {
    window
        .iter()
        .filter_map(|snap| pick(snap).map(|value| (value, snap.captured_unix_ms)))
        .max_by_key(|(value, _)| *value)
}

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

fn cpu_saturation(aggregate: &WindowAggregate, window: &[PerfSnapshot]) -> Option<RuleOutput> {
    if aggregate.cpu_busy_bp_avg >= thresholds::CPU_SATURATION_AVG_BP {
        let confidence = if aggregate.sample_count >= thresholds::MIN_SAMPLES_FOR_CONFIRMED {
            Confidence::Confirmed
        } else {
            Confidence::High
        };
        let last = window.last()?;
        return Some(RuleOutput {
            code: "CPU_SATURATION",
            tier: Tier::Workload,
            role: Role::RootCause,
            confidence,
            title_key: "perf.finding.cpuSaturation.title",
            summary_key: "perf.finding.cpuSaturation.summary",
            args: vec![
                arg(
                    "averagePercent",
                    format!("{}", aggregate.cpu_busy_bp_avg / 100),
                ),
                arg(
                    "peakPercent",
                    format!("{}", aggregate.cpu_busy_bp_peak / 100),
                ),
                arg("sampleCount", format!("{}", aggregate.sample_count)),
            ],
            evidence: vec![evidence(
                "cpu.busyBp.avg",
                u64::from(aggregate.cpu_busy_bp_avg),
                u64::from(thresholds::CPU_SATURATION_AVG_BP),
                last.captured_unix_ms,
            )],
            depends_on: vec![],
            applicable_actions: vec!["ecoQos", "backgroundPriority"],
        });
    }
    None
}

fn dpc_pressure(aggregate: &WindowAggregate, window: &[PerfSnapshot]) -> Option<RuleOutput> {
    if aggregate.dpc_isr_busy_bp_avg < thresholds::DPC_ISR_PRESSURE_BP {
        return None;
    }
    let last = window.last()?;
    Some(RuleOutput {
        code: "DPC_ISR_PRESSURE",
        tier: Tier::Hardware,
        role: Role::ContributingCondition,
        confidence: Confidence::High,
        title_key: "perf.finding.dpcPressure.title",
        summary_key: "perf.finding.dpcPressure.summary",
        args: vec![arg(
            "dpcIsrPercent",
            format!("{}", aggregate.dpc_isr_busy_bp_avg / 100),
        )],
        evidence: vec![evidence(
            "cpu.dpcIsrBp.avg",
            u64::from(aggregate.dpc_isr_busy_bp_avg),
            u64::from(thresholds::DPC_ISR_PRESSURE_BP),
            last.captured_unix_ms,
        )],
        depends_on: vec![],
        applicable_actions: vec![],
    })
}

fn thermal_clamp(window: &[PerfSnapshot]) -> Option<RuleOutput> {
    let throttled = window
        .iter()
        .filter(|snap| snap.power.throttle_active)
        .count();
    if throttled == 0 {
        return None;
    }
    let majority = throttled * 2 > window.len();
    let reason = window
        .iter()
        .filter(|snap| snap.power.throttle_active)
        .last()
        .map(|snap| snap.power.throttle_reason)
        .unwrap_or_default();
    let (code, title, summary) = match reason {
        aethercore_performance_telemetry::ThermalThrottleReason::Power
        | aethercore_performance_telemetry::ThermalThrottleReason::Vrm
        | aethercore_performance_telemetry::ThermalThrottleReason::Current => (
            "POWER_LIMIT_CLAMP",
            "perf.finding.powerClamp.title",
            "perf.finding.powerClamp.summary",
        ),
        _ => (
            "THERMAL_CLAMP",
            "perf.finding.thermalClamp.title",
            "perf.finding.thermalClamp.summary",
        ),
    };
    Some(RuleOutput {
        code,
        tier: Tier::Hardware,
        role: Role::RootCause,
        confidence: if majority {
            Confidence::Confirmed
        } else {
            Confidence::High
        },
        title_key: title,
        summary_key: summary,
        args: vec![arg("throttledSamples", format!("{throttled}"))],
        evidence: window
            .iter()
            .filter(|snap| snap.power.throttle_active)
            .take(3)
            .map(|snap| evidence("power.throttleActive", 1, 0, snap.captured_unix_ms))
            .collect(),
        depends_on: vec![],
        applicable_actions: vec![],
    })
}

fn standby_starvation(aggregate: &WindowAggregate, window: &[PerfSnapshot]) -> Option<RuleOutput> {
    if aggregate.hard_faults_per_sec_avg < thresholds::HARD_FAULT_STARVATION_PER_SEC {
        return None;
    }
    let last = window.last()?;
    Some(RuleOutput {
        code: "STANDBY_STARVATION",
        tier: Tier::Resource,
        role: Role::RootCause,
        confidence: if aggregate.sample_count >= thresholds::MIN_SAMPLES_FOR_CONFIRMED {
            Confidence::Confirmed
        } else {
            Confidence::High
        },
        title_key: "perf.finding.standbyStarvation.title",
        summary_key: "perf.finding.standbyStarvation.summary",
        args: vec![
            arg(
                "hardFaultsAvg",
                format!("{}", aggregate.hard_faults_per_sec_avg),
            ),
            arg(
                "hardFaultsPeak",
                format!("{}", aggregate.hard_faults_per_sec_peak),
            ),
        ],
        evidence: vec![evidence(
            "memory.hardFaults.avg",
            aggregate.hard_faults_per_sec_avg,
            thresholds::HARD_FAULT_STARVATION_PER_SEC,
            last.captured_unix_ms,
        )],
        depends_on: vec![],
        applicable_actions: vec![],
    })
}

fn commit_pressure(aggregate: &WindowAggregate, window: &[PerfSnapshot]) -> Option<RuleOutput> {
    if aggregate.commit_pressure_bp < thresholds::COMMIT_PRESSURE_BP {
        return None;
    }
    let last = window.last()?;
    Some(RuleOutput {
        code: "COMMIT_PRESSURE",
        tier: Tier::Resource,
        role: Role::ContributingCondition,
        confidence: Confidence::High,
        title_key: "perf.finding.commitPressure.title",
        summary_key: "perf.finding.commitPressure.summary",
        args: vec![arg(
            "commitPressurePercent",
            format!("{}", aggregate.commit_pressure_bp / 100),
        )],
        evidence: vec![evidence(
            "memory.commitBp.avg",
            u64::from(aggregate.commit_pressure_bp),
            u64::from(thresholds::COMMIT_PRESSURE_BP),
            last.captured_unix_ms,
        )],
        depends_on: vec![],
        applicable_actions: vec!["cooperativeTrimRequest"],
    })
}

fn io_saturation(aggregate: &WindowAggregate, window: &[PerfSnapshot]) -> Option<RuleOutput> {
    let saturated = aggregate.storage_active_bp_peak >= thresholds::STORAGE_SATURATION_PEAK_BP
        || aggregate.storage_active_bp_avg >= thresholds::STORAGE_SATURATION_AVG_BP;
    if !saturated {
        return None;
    }
    let latency_evidence = peak_of_reported(window, |snap| {
        snap.storage
            .iter()
            .map(|device| device.avg_transfer_latency_us)
            .max()
    });
    let mut evidence_vec = vec![evidence(
        "storage.activeBp.peak",
        u64::from(aggregate.storage_active_bp_peak),
        u64::from(thresholds::STORAGE_SATURATION_PEAK_BP),
        window.last()?.captured_unix_ms,
    )];
    let slow_transfer = latency_evidence
        .as_ref()
        .is_some_and(|(latency, _)| *latency >= thresholds::TRANSFER_LATENCY_US);
    if let Some((latency, at)) = latency_evidence {
        evidence_vec.push(evidence(
            "storage.transferLatencyUs",
            latency,
            thresholds::TRANSFER_LATENCY_US,
            at,
        ));
    }
    Some(RuleOutput {
        code: "IO_SATURATION",
        tier: Tier::Resource,
        role: Role::RootCause,
        confidence: if slow_transfer {
            Confidence::Confirmed
        } else {
            Confidence::High
        },
        title_key: "perf.finding.ioSaturation.title",
        summary_key: "perf.finding.ioSaturation.summary",
        args: vec![
            arg(
                "activePeakPercent",
                format!("{}", aggregate.storage_active_bp_peak / 100),
            ),
            arg(
                "activeAvgPercent",
                format!("{}", aggregate.storage_active_bp_avg / 100),
            ),
        ],
        evidence: evidence_vec,
        depends_on: vec![],
        applicable_actions: vec!["backgroundPriority"],
    })
}

fn gpu_bound(window: &[PerfSnapshot]) -> Option<RuleOutput> {
    let peak = peak_of_reported(window, |snap| {
        snap.gpu
            .engines
            .first()
            .map(|engine| u64::from(engine.utilization_bp))
    })?;
    if peak.0 < u64::from(thresholds::GPU_SATURATION_BP) {
        return None;
    }
    let jitter_peak = peak_of(window, |snap| snap.gpu.frametime_jitter_us);
    let lag = window.iter().any(|snap| snap.gpu.compositor_lag_detected);
    let confidence = if lag {
        Confidence::Confirmed
    } else {
        Confidence::High
    };
    let mut deps = Vec::new();
    let jitter_bad = jitter_peak
        .as_ref()
        .is_some_and(|(jitter, _)| *jitter >= thresholds::FRAMETIME_JITTER_US);
    if jitter_bad || lag {
        deps.push("DPC_ISR_PRESSURE");
    }
    Some(RuleOutput {
        code: "GPU_BOUND_WORKLOAD",
        tier: Tier::Workload,
        role: Role::RootCause,
        confidence,
        title_key: "perf.finding.gpuBound.title",
        summary_key: "perf.finding.gpuBound.summary",
        args: vec![arg("gpuPeakPercent", format!("{}", peak.0 / 100))],
        evidence: vec![evidence(
            "gpu.utilizationBp.peak",
            peak.0,
            u64::from(thresholds::GPU_SATURATION_BP),
            peak.1,
        )],
        depends_on: deps,
        applicable_actions: vec!["gameModeProfile"],
    })
}

fn working_set_bloat(window: &[PerfSnapshot]) -> Option<RuleOutput> {
    // Modified list growth across the window without matching hard-fault relief indicates
    // working sets are being held while dirty pages accumulate — a governance problem, not an
    // excuse for EmptyWorkingSet brute force.
    let first = window.first()?;
    let last = window.last()?;
    let growth = last
        .memory
        .modified_page_list_bytes
        .saturating_sub(first.memory.modified_page_list_bytes);
    if growth < thresholds::MODIFIED_LIST_GROWTH_BYTES {
        return None;
    }
    Some(RuleOutput {
        code: "WORKING_SET_BLOAT",
        tier: Tier::Resource,
        role: Role::ContributingCondition,
        confidence: Confidence::Medium,
        title_key: "perf.finding.workingSetBloat.title",
        summary_key: "perf.finding.workingSetBloat.summary",
        args: vec![arg("growthMb", format!("{}", growth / (1024 * 1024)))],
        evidence: vec![evidence(
            "memory.modifiedList.growthBytes",
            growth,
            thresholds::MODIFIED_LIST_GROWTH_BYTES,
            last.captured_unix_ms,
        )],
        depends_on: vec![],
        applicable_actions: vec!["cooperativeTrimRequest"],
    })
}

fn arg(key: &str, value: String) -> MessageArg {
    MessageArg {
        key: key.to_string(),
        value,
    }
}
