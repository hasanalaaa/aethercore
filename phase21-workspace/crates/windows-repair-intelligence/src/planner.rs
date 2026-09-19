use crate::{
    Diagnosis, DiagnosisConfidence, FactState, GraphError, RepairActionKind, RepairGraph,
    RepairNode, RepairObservationSet, RepairSafetyTier, Reversibility, VerificationKind,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlannerError {
    #[error(transparent)]
    Graph(#[from] GraphError),
}

pub fn build_repair_graph(
    input: &RepairObservationSet,
    diagnoses: &[Diagnosis],
) -> Result<RepairGraph, PlannerError> {
    let mut nodes = Vec::new();
    let has = |code: &str| {
        diagnoses
            .iter()
            .any(|d| d.code == code && d.confidence >= DiagnosisConfidence::Medium)
    };
    let ids = |code: &str| {
        diagnoses
            .iter()
            .filter(|d| d.code == code)
            .map(|d| d.id.clone())
            .collect::<Vec<_>>()
    };

    let mut component_verify = None;
    if has("COMPONENT_STORE_CORRUPTION") {
        let repair = node(
            "repair-component-store",
            RepairActionKind::RepairComponentStore,
            RepairSafetyTier::Level2SensitiveRepair,
            &[],
            "windows:component-store",
            ids("COMPONENT_STORE_CORRUPTION"),
            VerificationKind::RecheckComponentStore,
            Reversibility::PartiallyReversible,
            true,
            false,
            false,
            true,
        );
        nodes.push(repair);
        let verify = node(
            "verify-component-store",
            RepairActionKind::VerifyComponentStore,
            RepairSafetyTier::Level0Diagnostic,
            &["repair-component-store"],
            "windows:component-store",
            ids("COMPONENT_STORE_CORRUPTION"),
            VerificationKind::RecheckComponentStore,
            Reversibility::FullyReversible,
            false,
            false,
            false,
            true,
        );
        nodes.push(verify);
        component_verify = Some("verify-component-store".to_string());
    }

    if has("SYSTEM_FILE_INTEGRITY_FAILURE") {
        let deps = component_verify
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        nodes.push(node(
            "repair-system-files",
            RepairActionKind::RepairSystemFiles,
            RepairSafetyTier::Level2SensitiveRepair,
            &deps,
            "windows:protected-system-files",
            ids("SYSTEM_FILE_INTEGRITY_FAILURE"),
            VerificationKind::ReverifySystemFiles,
            Reversibility::PartiallyReversible,
            true,
            false,
            false,
            true,
        ));
        nodes.push(node(
            "verify-system-files",
            RepairActionKind::VerifySystemFiles,
            RepairSafetyTier::Level0Diagnostic,
            &["repair-system-files"],
            "windows:protected-system-files",
            ids("SYSTEM_FILE_INTEGRITY_FAILURE"),
            VerificationKind::ReverifySystemFiles,
            Reversibility::FullyReversible,
            false,
            false,
            false,
            true,
        ));
    }

    if has("PENDING_REBOOT") {
        nodes.push(node(
            "reboot-boundary",
            RepairActionKind::Reboot,
            RepairSafetyTier::Level3RebootOrOffline,
            &[],
            "windows:reboot",
            ids("PENDING_REBOOT"),
            VerificationKind::None,
            Reversibility::RebootRollback,
            true,
            false,
            true,
            false,
        ));
    }

    if has("REQUIRED_SERVICE_STOPPED") {
        nodes.push(node(
            "start-required-service",
            RepairActionKind::StartRequiredService,
            RepairSafetyTier::Level1SafeAuto,
            &[],
            "windows:diagnosis-scoped-service",
            ids("REQUIRED_SERVICE_STOPPED"),
            VerificationKind::QueryServiceState,
            Reversibility::FullyReversible,
            false,
            false,
            false,
            true,
        ));
    }
    if has("SERVICE_CONFIGURATION_UNEXPECTED") {
        nodes.push(node(
            "review-service-configuration",
            RepairActionKind::RestartRequiredService,
            RepairSafetyTier::Level2SensitiveRepair,
            &[],
            "windows:diagnosis-scoped-service",
            ids("SERVICE_CONFIGURATION_UNEXPECTED"),
            VerificationKind::QueryServiceState,
            Reversibility::PartiallyReversible,
            true,
            false,
            false,
            false,
        ));
    }

    if has("DNS_RESOLUTION_FAILURE") && !has("NETWORK_CONNECTIVITY_FAILURE") {
        nodes.push(node(
            "flush-dns-cache",
            RepairActionKind::FlushDnsCache,
            RepairSafetyTier::Level1SafeAuto,
            &[],
            "windows:dns-cache",
            ids("DNS_RESOLUTION_FAILURE"),
            VerificationKind::RerunDnsResolution,
            Reversibility::FullyReversible,
            false,
            false,
            false,
            true,
        ));
    }
    if has("PROXY_CONFIGURATION_PROBLEM") {
        nodes.push(node(
            "review-proxy",
            RepairActionKind::ReviewProxyConfiguration,
            RepairSafetyTier::Level0Diagnostic,
            &[],
            "windows:proxy",
            ids("PROXY_CONFIGURATION_PROBLEM"),
            VerificationKind::RecheckProxy,
            Reversibility::FullyReversible,
            false,
            false,
            false,
            false,
        ));
    }

    if has("FILESYSTEM_ERROR") {
        nodes.push(node(
            "filesystem-offline-repair",
            RepairActionKind::RepairFilesystemOffline,
            RepairSafetyTier::Level3RebootOrOffline,
            &[],
            "windows:system-volume",
            ids("FILESYSTEM_ERROR"),
            VerificationKind::RerunFilesystemScan,
            Reversibility::IrreversibleManualRecovery,
            true,
            true,
            true,
            false,
        ));
    }
    if has("STORAGE_HARDWARE_WARNING") {
        nodes.push(node(
            "hardware-service",
            RepairActionKind::GuidedHardwareService,
            RepairSafetyTier::Level4RecoveryEscalation,
            &[],
            "hardware:storage",
            ids("STORAGE_HARDWARE_WARNING"),
            VerificationKind::ManualOfficialRecovery,
            Reversibility::IrreversibleManualRecovery,
            true,
            false,
            false,
            false,
        ));
    }

    if has("WINDOWS_UPDATE_FAILURE") {
        let mut deps = Vec::new();
        if component_verify.is_some() {
            deps.push("verify-component-store");
        }
        if nodes.iter().any(|n| n.id == "verify-system-files") {
            deps.push("verify-system-files");
        }
        if nodes.iter().any(|n| n.id == "start-required-service") {
            deps.push("start-required-service");
        }
        if nodes.iter().any(|n| n.id == "reboot-boundary") {
            // A required reboot is represented as a hard resume barrier; update retry is not auto-executable in the pre-reboot session.
            deps.push("reboot-boundary");
        }
        nodes.push(node(
            "retry-windows-update",
            RepairActionKind::RetryWindowsUpdate,
            RepairSafetyTier::Level1SafeAuto,
            &deps,
            "windows:update-agent",
            ids("WINDOWS_UPDATE_FAILURE"),
            VerificationKind::RetryWindowsUpdateDiscovery,
            Reversibility::FullyReversible,
            false,
            false,
            false,
            !deps.contains(&"reboot-boundary"),
        ));
    }

    if has("RECOVERY_ENVIRONMENT_UNAVAILABLE") {
        nodes.push(node(
            "verify-winre",
            RepairActionKind::VerifyWinRe,
            RepairSafetyTier::Level0Diagnostic,
            &[],
            "windows:winre",
            ids("RECOVERY_ENVIRONMENT_UNAVAILABLE"),
            VerificationKind::VerifyWinRe,
            Reversibility::FullyReversible,
            false,
            false,
            false,
            true,
        ));
    }

    // Exhausted lower tiers: model, never silently execute, official recovery escalation.
    let persistent = input
        .facts
        .iter()
        .any(|f| matches!(f.state, FactState::RepairFailed | FactState::SourceRequired));
    if persistent {
        let deps = nodes
            .iter()
            .filter(|n| {
                n.action == RepairActionKind::VerifyComponentStore
                    || n.action == RepairActionKind::VerifySystemFiles
            })
            .map(|n| n.id.as_str())
            .collect::<Vec<_>>();
        nodes.push(node(
            "guided-repair-reinstall",
            RepairActionKind::GuidedRepairReinstall,
            RepairSafetyTier::Level4RecoveryEscalation,
            &deps,
            "windows:official-recovery",
            diagnoses.iter().map(|d| d.id.clone()).collect(),
            VerificationKind::ManualOfficialRecovery,
            Reversibility::PartiallyReversible,
            true,
            true,
            false,
            false,
        ));
    }

    // Healthy/offline/unknown-only observations intentionally produce no mutation nodes.
    RepairGraph::new(nodes).map_err(Into::into)
}

#[allow(clippy::too_many_arguments)]
fn node(
    id: &str,
    action: RepairActionKind,
    safety: RepairSafetyTier,
    deps: &[&str],
    target: &str,
    diagnosis_ids: Vec<String>,
    verification: VerificationKind,
    reversibility: Reversibility,
    consent: bool,
    requires_recovery: bool,
    reboot: bool,
    auto: bool,
) -> RepairNode {
    RepairNode {
        id: id.into(),
        action,
        safety,
        dependencies: deps.iter().map(|v| (*v).into()).collect(),
        target_resource: target.into(),
        diagnosis_ids,
        verification,
        reversibility,
        requires_explicit_consent: consent,
        requires_recovery_protection: requires_recovery,
        reboot_boundary_after: reboot,
        executable_automatically: auto,
    }
}
