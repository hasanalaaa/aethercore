use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const RULE_ENGINE_VERSION: &str = "phase17.1-rules-v2";

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "camelCase")]
pub enum Domain {
    System,
    Hardware,
    Drivers,
    Windows,
    Storage,
    Memory,
    Diagnostics,
    Performance,
    Startup,
    Cleanup,
    Updates,
    Recovery,
}

impl Domain {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Hardware => "hardware",
            Self::Drivers => "drivers",
            Self::Windows => "windows",
            Self::Storage => "storage",
            Self::Memory => "memory",
            Self::Diagnostics => "diagnostics",
            Self::Performance => "performance",
            Self::Startup => "startup",
            Self::Cleanup => "cleanup",
            Self::Updates => "updates",
            Self::Recovery => "recovery",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum Confidence {
    Unknown,
    Low,
    Medium,
    High,
    Confirmed,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Informational,
    Low,
    Moderate,
    High,
    Critical,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Freshness {
    Current,
    Recent,
    Historical,
    Stale,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum SystemStatus {
    Healthy,
    AttentionRecommended,
    ActionRequired,
    Critical,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ScanState {
    Idle,
    Scanning,
    Completed,
    Partial,
    Cancelled,
    Failed,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CollectorState {
    Pending,
    Running,
    Completed,
    CompletedWithWarnings,
    Unavailable,
    PermissionDenied,
    TimedOut,
    Cancelled,
    Failed,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "camelCase")]
pub enum ScanStage {
    SystemIdentity,
    HardwareInventory,
    DriverInventory,
    WindowsIntegrity,
    StorageHealth,
    MemoryPressure,
    HardwareErrors,
    CrashDiagnostics,
    StartupFootprint,
    CleanupOpportunities,
    UpdateState,
    RecoveryReadiness,
    Correlation,
    RecommendationSynthesis,
}

impl ScanStage {
    pub const fn weight(self) -> u32 {
        match self {
            Self::SystemIdentity => 4,
            Self::HardwareInventory => 8,
            Self::DriverInventory => 13,
            Self::WindowsIntegrity => 11,
            Self::StorageHealth => 9,
            Self::MemoryPressure => 5,
            Self::HardwareErrors => 8,
            Self::CrashDiagnostics => 8,
            Self::StartupFootprint => 7,
            Self::CleanupOpportunities => 7,
            Self::UpdateState => 5,
            Self::RecoveryReadiness => 4,
            Self::Correlation => 6,
            Self::RecommendationSynthesis => 5,
        }
    }

    pub const fn message_key(self) -> &'static str {
        match self {
            Self::SystemIdentity => "deepScan.stage.systemIdentity",
            Self::HardwareInventory => "deepScan.stage.hardwareInventory",
            Self::DriverInventory => "deepScan.stage.driverInventory",
            Self::WindowsIntegrity => "deepScan.stage.windowsIntegrity",
            Self::StorageHealth => "deepScan.stage.storageHealth",
            Self::MemoryPressure => "deepScan.stage.memoryPressure",
            Self::HardwareErrors => "deepScan.stage.hardwareErrors",
            Self::CrashDiagnostics => "deepScan.stage.crashDiagnostics",
            Self::StartupFootprint => "deepScan.stage.startupFootprint",
            Self::CleanupOpportunities => "deepScan.stage.cleanup",
            Self::UpdateState => "deepScan.stage.updateState",
            Self::RecoveryReadiness => "deepScan.stage.recoveryReadiness",
            Self::Correlation => "deepScan.stage.correlation",
            Self::RecommendationSynthesis => "deepScan.stage.recommendations",
        }
    }
}

pub const ALL_STAGES: [ScanStage; 14] = [
    ScanStage::SystemIdentity,
    ScanStage::HardwareInventory,
    ScanStage::DriverInventory,
    ScanStage::WindowsIntegrity,
    ScanStage::StorageHealth,
    ScanStage::MemoryPressure,
    ScanStage::HardwareErrors,
    ScanStage::CrashDiagnostics,
    ScanStage::StartupFootprint,
    ScanStage::CleanupOpportunities,
    ScanStage::UpdateState,
    ScanStage::RecoveryReadiness,
    ScanStage::Correlation,
    ScanStage::RecommendationSynthesis,
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResourceRef {
    pub kind: String,
    pub stable_id: String,
    pub display_name: String,
}

impl ResourceRef {
    pub fn private(
        kind: impl Into<String>,
        raw_identity: &str,
        display_name: impl Into<String>,
    ) -> Self {
        Self {
            kind: kind.into(),
            stable_id: private_id(raw_identity),
            display_name: display_name.into(),
        }
    }

    pub fn global(
        kind: impl Into<String>,
        stable_id: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Self {
        Self {
            kind: kind.into(),
            stable_id: stable_id.into(),
            display_name: display_name.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EvidenceKind {
    DeviceState,
    DriverVersion,
    UpdateOffer,
    ServicingResult,
    StorageHealth,
    SmartMetric,
    HardwareEvent,
    CrashRecord,
    StartupRegistration,
    CleanupEstimate,
    UpdateState,
    CollectorLimitation,
    JournalEvent,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceRef {
    pub fact_id: String,
    pub kind: EvidenceKind,
    pub source: String,
    pub observed_unix_ms: i64,
    pub technical_value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FactPayload {
    InventorySummary {
        device_count: u32,
    },
    HardwareDevice {
        class_name: String,
        manufacturer: String,
        description: String,
        installed_driver_version: String,
        gpu_vendor: String,
    },
    DeviceHealth {
        missing_driver: bool,
        has_problem: bool,
        problem_code: u32,
        device_state: String,
        update_status: String,
        authority_coverage: String,
        management_authorities: Vec<String>,
    },
    DriverUpdate {
        candidate_id: String,
        current_version: String,
        target_version: String,
        vendor_managed: bool,
        selectable: bool,
        authority_type: String,
        authority_name: String,
        recommendation_state: String,
        installation_mode: String,
        trust_state: String,
        authority_coverage: String,
    },
    DriverChange {
        installed_unix_ms: i64,
        previous_version: String,
        version: String,
        authority_type: String,
        authority_provider_id: String,
        provider: String,
        reboot_required: bool,
        verified: bool,
        rollback_available: bool,
    },
    WindowsIntegrity {
        check_id: String,
        result_code: String,
        exit_code: i32,
        detail: String,
    },
    StorageHealth {
        health_status: String,
        source_severity: String,
        /// `None` when the SMART attribute was not readable on this pass.
        /// A counter that did not answer is not a counter that read zero —
        /// see `rules::explicitly_healthy`, which authorises resolution.
        uncorrected_read_errors: Option<u64>,
        uncorrected_write_errors: Option<u64>,
        nvme_critical_warning: Option<u8>,
        nvme_media_errors_nonzero: bool,
        wear_percent: Option<u64>,
        temperature_c: Option<i64>,
        temperature_max_c: Option<i64>,
    },
    MemoryPressure {
        memory_load_percent: u32,
        pressure_label: String,
    },
    HardwareEvent {
        category: String,
        provider: String,
        event_id: u32,
    },
    Crash {
        crash_id: String,
        bugcheck_hex: String,
    },
    StartupFootprint {
        high_impact_count: u32,
        manageable_count: u32,
        total_count: u32,
    },
    CleanupOpportunity {
        candidate_id: String,
        reclaimable_bytes: u64,
        file_count: u32,
        requires_confirmation: bool,
    },
    UpdateState {
        state: String,
        current_version: String,
        available_release_id: String,
        update_available: bool,
        failed: bool,
    },
    RecoveryReadiness {
        active_recovery_records: u32,
    },
    DiagnosticLimitation {
        collector: String,
        state: CollectorState,
        detail: String,
    },
}

impl FactPayload {
    pub const fn kind_name(&self) -> &'static str {
        match self {
            Self::InventorySummary { .. } => "inventorySummary",
            Self::HardwareDevice { .. } => "hardwareDevice",
            Self::DeviceHealth { .. } => "deviceHealth",
            Self::DriverUpdate { .. } => "driverUpdate",
            Self::DriverChange { .. } => "driverChange",
            Self::WindowsIntegrity { .. } => "windowsIntegrity",
            Self::StorageHealth { .. } => "storageHealth",
            Self::MemoryPressure { .. } => "memoryPressure",
            Self::HardwareEvent { .. } => "hardwareEvent",
            Self::Crash { .. } => "crash",
            Self::StartupFootprint { .. } => "startupFootprint",
            Self::CleanupOpportunity { .. } => "cleanupOpportunity",
            Self::UpdateState { .. } => "updateState",
            Self::RecoveryReadiness { .. } => "recoveryReadiness",
            Self::DiagnosticLimitation { .. } => "diagnosticLimitation",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SystemFact {
    pub id: String,
    pub domain: Domain,
    pub source: String,
    pub resource: ResourceRef,
    pub observed_unix_ms: i64,
    pub freshness: Freshness,
    pub confidence: Confidence,
    pub evidence: Vec<EvidenceRef>,
    pub payload: FactPayload,
}

impl SystemFact {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        domain: Domain,
        source: impl Into<String>,
        resource: ResourceRef,
        observed_unix_ms: i64,
        freshness: Freshness,
        confidence: Confidence,
        payload: FactPayload,
        evidence_kind: EvidenceKind,
        technical_value: impl Into<String>,
    ) -> Self {
        let source = source.into();
        let seed = format!(
            "{}|{}|{}|{}",
            domain.as_str(),
            source,
            resource.stable_id,
            payload.kind_name()
        );
        let id = stable_id("fact", &seed);
        let evidence = vec![EvidenceRef {
            fact_id: id.clone(),
            kind: evidence_kind,
            source: source.clone(),
            observed_unix_ms,
            technical_value: bounded_text(technical_value.into(), 2048),
        }];
        Self {
            id,
            domain,
            source,
            resource,
            observed_unix_ms,
            freshness,
            confidence,
            evidence,
            payload,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FindingLifecycle {
    New,
    Active,
    Improved,
    Resolved,
    #[serde(alias = "returned")]
    Recurred,
    Ignored,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum FindingVerificationStatus {
    ConfirmedCurrent,
    #[default]
    NotRechecked,
    VerificationUnavailable,
    ResolutionConfirmed,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CorrelationStrength {
    Weak,
    Moderate,
    Strong,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionEvidence {
    pub scope: String,
    pub collector_state: CollectorState,
    pub observed_unix_ms: i64,
    pub evidence_fact_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CorrelationExplanation {
    pub strength: CorrelationStrength,
    pub time_distance_ms: u64,
    pub shared_scope: String,
    pub rationale_key: String,
    pub contributing_fact_ids: Vec<String>,
    pub conflicting_evidence_keys: Vec<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RemediationSafety {
    SafeAuto,
    SafeReview,
    Sensitive,
    Manual,
    HardwareService,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PrivilegeRequirement {
    None,
    User,
    ElevatedService,
    VendorTool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RebootRequirement {
    None,
    Possible,
    Required,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Reversibility {
    Reversible,
    Checkpointed,
    Limited,
    NotSoftwareReversible,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ImpactEstimate {
    Minimal,
    Low,
    Moderate,
    High,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ActionType {
    InstallDriver,
    RepairWindows,
    ReviewStorage,
    ReviewHardwareError,
    ReviewCrashEvidence,
    DisableStartupItem,
    CleanupData,
    InstallAppUpdate,
    ManualVendorAction,
    HardwareService,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub id: String,
    pub code: String,
    pub domain: Domain,
    pub severity: Severity,
    pub confidence: Confidence,
    pub title_key: String,
    pub summary_key: String,
    pub technical_key: String,
    pub message_args: BTreeMap<String, String>,
    pub evidence: Vec<EvidenceRef>,
    pub affected_resource: ResourceRef,
    pub first_observed_unix_ms: i64,
    pub last_observed_unix_ms: i64,
    pub lifecycle: FindingLifecycle,
    pub remediation_available: bool,
    pub remediation_safety: Option<RemediationSafety>,
    pub reboot_requirement: RebootRequirement,
    pub privilege_requirement: PrivilegeRequirement,
    pub automatic_eligible: bool,
    pub reversibility: Reversibility,
    pub estimated_impact: ImpactEstimate,
    pub uncertainty_key: String,
    pub ignored: bool,
    pub rule_id: String,
    pub rule_version: u32,
    #[serde(default)]
    pub verification_status: FindingVerificationStatus,
    #[serde(default)]
    pub resolution_authority: Vec<String>,
    #[serde(default)]
    pub resolved_at_unix_ms: Option<i64>,
    #[serde(default)]
    pub resolution_scan_id: String,
    #[serde(default)]
    pub resolution_reason_key: String,
    #[serde(default)]
    pub resolution_evidence: Vec<ResolutionEvidence>,
    #[serde(default)]
    pub correlation: Option<CorrelationExplanation>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemediationCandidate {
    pub action_id: String,
    pub finding_id: String,
    pub action_type: ActionType,
    pub description_key: String,
    pub authority: String,
    pub privilege: PrivilegeRequirement,
    pub safety: RemediationSafety,
    pub reversibility: Reversibility,
    pub reboot_requirement: RebootRequirement,
    pub expected_effect_key: String,
    pub preconditions: Vec<String>,
    pub verification_method_key: String,
    pub conflicts: Vec<String>,
    pub duration_category: String,
    pub automatic_eligible: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemediationPlan {
    pub plan_id: String,
    pub scan_id: String,
    pub digest: String,
    pub created_unix_ms: i64,
    pub immutable: bool,
    pub actions: Vec<RemediationCandidate>,
}

impl RemediationPlan {
    pub fn seal(
        scan_id: &str,
        created_unix_ms: i64,
        mut actions: Vec<RemediationCandidate>,
    ) -> Self {
        actions.sort_by(|a, b| a.action_id.cmp(&b.action_id));
        let canonical = serde_json::to_vec(&(scan_id, created_unix_ms, &actions)).unwrap_or_default();
        let digest = hex_sha256(&canonical);
        Self {
            plan_id: stable_id("remediation-plan", &format!("{scan_id}|{digest}")),
            scan_id: scan_id.to_owned(),
            digest,
            created_unix_ms,
            immutable: true,
            actions,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CollectorStatus {
    pub id: String,
    pub state: CollectorState,
    pub stage_keys: Vec<String>,
    pub started_unix_ms: i64,
    pub completed_unix_ms: i64,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub total_weight: u32,
    pub completed_weight: u32,
    pub completed_tasks: u32,
    pub total_tasks: u32,
    pub active_tasks: u32,
    pub skipped_tasks: u32,
    pub failed_tasks: u32,
    pub unavailable_tasks: u32,
    pub current_stage_key: String,
}

impl ScanProgress {
    pub fn percent(&self) -> u32 {
        if self.total_weight == 0 {
            0
        } else {
            self.completed_weight
                .saturating_mul(100)
                .checked_div(self.total_weight)
                .unwrap_or(0)
                .min(100)
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct FindingSummary {
    pub critical: u32,
    pub high: u32,
    pub moderate: u32,
    pub low: u32,
    pub informational: u32,
    pub recommended_actions: u32,
    pub optional_optimizations: u32,
    pub healthy_checks: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScanMetrics {
    pub duration_ms: i64,
    pub collector_duration_ms: i64,
    pub peak_active_tasks: u32,
    pub streamed_event_count: u32,
    pub persistence_write_count: u32,
    pub normalized_payload_bytes_estimate: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeepScanSnapshot {
    pub scan_id: String,
    pub state: ScanState,
    pub status: SystemStatus,
    pub started_unix_ms: i64,
    pub completed_unix_ms: i64,
    pub progress: ScanProgress,
    pub facts_count: u32,
    pub findings: Vec<Finding>,
    pub remediation_candidates: Vec<RemediationCandidate>,
    pub collectors: Vec<CollectorStatus>,
    pub warnings: Vec<String>,
    pub summary: FindingSummary,
    pub metrics: ScanMetrics,
    pub machine_state_fingerprint: String,
    pub rule_engine_version: String,
    pub app_version: String,
}

impl Default for DeepScanSnapshot {
    fn default() -> Self {
        Self {
            scan_id: String::new(),
            state: ScanState::Idle,
            status: SystemStatus::Healthy,
            started_unix_ms: 0,
            completed_unix_ms: 0,
            progress: ScanProgress {
                total_weight: ALL_STAGES.iter().map(|value| value.weight()).sum(),
                completed_weight: 0,
                completed_tasks: 0,
                total_tasks: 7,
                active_tasks: 0,
                skipped_tasks: 0,
                failed_tasks: 0,
                unavailable_tasks: 0,
                current_stage_key: String::new(),
            },
            facts_count: 0,
            findings: Vec::new(),
            remediation_candidates: Vec::new(),
            collectors: Vec::new(),
            warnings: Vec::new(),
            summary: FindingSummary::default(),
            metrics: ScanMetrics::default(),
            machine_state_fingerprint: String::new(),
            rule_engine_version: RULE_ENGINE_VERSION.into(),
            app_version: String::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeepScanHistoryEntry {
    pub scan_id: String,
    pub state: ScanState,
    pub status: SystemStatus,
    pub completed_unix_ms: i64,
    pub duration_ms: i64,
    pub finding_count: u32,
    pub unavailable_collector_count: u32,
    pub machine_state_fingerprint: String,
}

pub fn stable_id(prefix: &str, seed: &str) -> String {
    format!("{prefix}-{}", &hex_sha256(seed.as_bytes())[..24])
}

pub fn private_id(raw: &str) -> String {
    stable_id("private", raw)
}

pub fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn bounded_text(mut value: String, limit: usize) -> String {
    if value.len() <= limit {
        return value;
    }
    let mut end = limit;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    value.truncate(end);
    value.push('…');
    value
}
