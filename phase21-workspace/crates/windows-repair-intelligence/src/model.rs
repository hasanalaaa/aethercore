use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "camelCase")]
pub enum RepairDomain {
    ComponentStore,
    SystemFiles,
    Servicing,
    WindowsUpdate,
    Reboot,
    Services,
    Network,
    Dns,
    Proxy,
    Filesystem,
    StorageHardware,
    Recovery,
    Boot,
    WindowsSecurity,
    ApplicationPlatform,
    Driver,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum FactState {
    Healthy,
    Repairable,
    CorruptionDetected,
    RepairFailed,
    SourceRequired,
    RebootRequired,
    Active,
    Stopped,
    Disabled,
    UnexpectedConfiguration,
    Unavailable,
    Offline,
    Failure,
    Degraded,
    Available,
    Unknown,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosisRole {
    RootCause,
    ContributingCondition,
    Symptom,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosisConfidence {
    Unknown,
    Low,
    Medium,
    High,
    Confirmed,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "camelCase")]
pub enum RepairSafetyTier {
    Level0Diagnostic,
    Level1SafeAuto,
    Level2SensitiveRepair,
    Level3RebootOrOffline,
    Level4RecoveryEscalation,
    Level5DestructiveRecovery,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum Reversibility {
    FullyReversible,
    PartiallyReversible,
    RebootRollback,
    RestorePointDependent,
    IrreversibleManualRecovery,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum VerificationKind {
    None,
    RecheckComponentStore,
    ReverifySystemFiles,
    RetryWindowsUpdateDiscovery,
    QueryServiceState,
    RerunNetworkDiagnostic,
    RerunDnsResolution,
    RecheckProxy,
    RerunFilesystemScan,
    VerifyWinRe,
    VerifyDefenderHealth,
    ManualOfficialRecovery,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum RepairActionKind {
    CheckComponentStore,
    ScanComponentStore,
    RepairComponentStore,
    VerifyComponentStore,
    VerifySystemFiles,
    RepairSystemFiles,
    CheckFilesystem,
    RepairFilesystemOffline,
    StartRequiredService,
    RestartRequiredService,
    RetryWindowsUpdate,
    RecoverWindowsUpdateCache,
    FlushDnsCache,
    RenewDhcpLease,
    ResetWinsock,
    ReviewProxyConfiguration,
    RestoreProxyConfiguration,
    VerifyWinRe,
    EnableWinRe,
    CreateRecoveryPoint,
    UpdateDefenderMetadata,
    RunDefenderScan,
    GuidedRepairReinstall,
    GuidedWinReRecovery,
    GuidedResetPreservingFiles,
    GuidedCleanReinstall,
    GuidedHardwareService,
    Reboot,
}

impl RepairActionKind {
    pub fn canonical_id(self) -> &'static str {
        match self {
            Self::CheckComponentStore => "check-component-store",
            Self::ScanComponentStore => "scan-component-store",
            Self::RepairComponentStore => "repair-component-store",
            Self::VerifyComponentStore => "verify-component-store",
            Self::VerifySystemFiles => "verify-system-files",
            Self::RepairSystemFiles => "repair-system-files",
            Self::CheckFilesystem => "check-filesystem",
            Self::RepairFilesystemOffline => "repair-filesystem-offline",
            Self::StartRequiredService => "start-required-service",
            Self::RestartRequiredService => "restart-required-service",
            Self::RetryWindowsUpdate => "retry-windows-update",
            Self::RecoverWindowsUpdateCache => "recover-windows-update-cache",
            Self::FlushDnsCache => "flush-dns-cache",
            Self::RenewDhcpLease => "renew-dhcp-lease",
            Self::ResetWinsock => "reset-winsock",
            Self::ReviewProxyConfiguration => "review-proxy-configuration",
            Self::RestoreProxyConfiguration => "restore-proxy-configuration",
            Self::VerifyWinRe => "verify-winre",
            Self::EnableWinRe => "enable-winre",
            Self::CreateRecoveryPoint => "create-recovery-point",
            Self::UpdateDefenderMetadata => "update-defender-metadata",
            Self::RunDefenderScan => "run-defender-scan",
            Self::GuidedRepairReinstall => "guided-repair-reinstall",
            Self::GuidedWinReRecovery => "guided-winre-recovery",
            Self::GuidedResetPreservingFiles => "guided-reset-preserving-files",
            Self::GuidedCleanReinstall => "guided-clean-reinstall",
            Self::GuidedHardwareService => "guided-hardware-service",
            Self::Reboot => "reboot",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepairFact {
    pub id: String,
    pub domain: RepairDomain,
    pub state: FactState,
    pub resource: String,
    pub evidence_code: String,
    pub technical_code: String,
    pub detail: String,
    pub observed_unix_ms: i64,
    pub confidence: DiagnosisConfidence,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Diagnosis {
    pub id: String,
    pub code: String,
    pub role: DiagnosisRole,
    pub domain: RepairDomain,
    pub confidence: DiagnosisConfidence,
    pub scope: String,
    pub evidence_ids: Vec<String>,
    pub uncertainty: String,
    pub rule_version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryReadiness {
    pub system_restore: FactState,
    pub restore_point_creation: FactState,
    pub win_re: FactState,
    pub journal_recovery: FactState,
    pub driver_rollback: FactState,
}

impl RecoveryReadiness {
    pub fn has_mandatory_protection(&self) -> bool {
        matches!(self.restore_point_creation, FactState::Available)
            || matches!(self.win_re, FactState::Available)
            || matches!(self.journal_recovery, FactState::Available)
    }
}

impl Default for RecoveryReadiness {
    fn default() -> Self {
        Self {
            system_restore: FactState::Unknown,
            restore_point_creation: FactState::Unknown,
            win_re: FactState::Unknown,
            journal_recovery: FactState::Available,
            driver_rollback: FactState::Unknown,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepairObservationSet {
    pub observation_id: String,
    pub machine_state_fingerprint: String,
    pub facts: Vec<RepairFact>,
    pub recovery: RecoveryReadiness,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepairNode {
    pub id: String,
    pub action: RepairActionKind,
    pub safety: RepairSafetyTier,
    pub dependencies: Vec<String>,
    pub target_resource: String,
    pub diagnosis_ids: Vec<String>,
    pub verification: VerificationKind,
    pub reversibility: Reversibility,
    pub requires_explicit_consent: bool,
    pub requires_recovery_protection: bool,
    pub reboot_boundary_after: bool,
    pub executable_automatically: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepairGraph {
    pub schema: String,
    pub valid: bool,
    pub invalid_reason: String,
    pub nodes: Vec<RepairNode>,
    pub deterministic_order: Vec<String>,
    pub digest_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepairIntelligenceSnapshot {
    pub schema: String,
    pub observation_id: String,
    pub machine_state_fingerprint: String,
    pub facts: Vec<RepairFact>,
    pub diagnoses: Vec<Diagnosis>,
    pub recovery: RecoveryReadiness,
    pub graph: RepairGraph,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RepairOutcome {
    SucceededVerified,
    SucceededVerificationPendingReboot,
    MutationSucceededVerificationFailed,
    FailedBeforeMutation,
    FailedAfterMutation,
    RolledBack,
    RollbackFailed,
    CancelledBeforeMutation,
    CancellationDeferred,
    ManualInterventionRequired,
    RecoveryEscalationRequired,
}
