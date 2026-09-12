use crate::{
    Diagnosis, DiagnosisConfidence, DiagnosisRole, FactState, RepairDomain, RepairObservationSet,
};

pub struct DiagnosisRuleVersion;
impl DiagnosisRuleVersion {
    pub const V1: &'static str = "p19.windows-repair-rules.v1";
}

pub fn diagnose(input: &RepairObservationSet) -> Vec<Diagnosis> {
    let mut out = Vec::new();
    // A free function instead of a closure: a closure would hold a mutable borrow of `out` until
    // its last use, which collides with the evidence-role scans that read `out` between adds.
    fn push_diagnosis(
        out: &mut Vec<Diagnosis>,
        code: &str,
        role: DiagnosisRole,
        domain: RepairDomain,
        confidence: DiagnosisConfidence,
        evidence: Vec<String>,
        uncertainty: &str,
    ) {
        out.push(Diagnosis {
            id: format!("diag:{}", code.to_ascii_lowercase()),
            code: code.into(),
            role,
            domain,
            confidence,
            scope: "Windows".into(),
            evidence_ids: evidence,
            uncertainty: uncertainty.into(),
            rule_version: DiagnosisRuleVersion::V1.into(),
        });
    }
    let facts = &input.facts;
    let ids = |domain: RepairDomain, states: &[FactState]| -> Vec<String> {
        facts
            .iter()
            .filter(|f| f.domain == domain && states.contains(&f.state))
            .map(|f| f.id.clone())
            .collect()
    };

    let component_bad = ids(
        RepairDomain::ComponentStore,
        &[
            FactState::Repairable,
            FactState::CorruptionDetected,
            FactState::RepairFailed,
            FactState::SourceRequired,
        ],
    );
    if !component_bad.is_empty() {
        let confidence = if facts.iter().any(|f| {
            f.domain == RepairDomain::ComponentStore
                && f.state == FactState::CorruptionDetected
                && f.confidence >= DiagnosisConfidence::High
        }) {
            DiagnosisConfidence::Confirmed
        } else {
            DiagnosisConfidence::High
        };
        push_diagnosis(
            &mut out,
            "COMPONENT_STORE_CORRUPTION",
            DiagnosisRole::RootCause,
            RepairDomain::ComponentStore,
            confidence,
            component_bad,
            "Confirmed only by supported servicing evidence; a tool execution failure alone is not corruption.",
        );
    }

    let system_bad = ids(
        RepairDomain::SystemFiles,
        &[
            FactState::CorruptionDetected,
            FactState::Repairable,
            FactState::RepairFailed,
        ],
    );
    if !system_bad.is_empty() {
        let role = if out.iter().any(|d| d.code == "COMPONENT_STORE_CORRUPTION") {
            DiagnosisRole::Symptom
        } else {
            DiagnosisRole::RootCause
        };
        push_diagnosis(
            &mut out,
            "SYSTEM_FILE_INTEGRITY_FAILURE",
            role,
            RepairDomain::SystemFiles,
            DiagnosisConfidence::High,
            system_bad,
            "Protected-file integrity evidence does not by itself identify why the files became damaged.",
        );
    }

    let reboot = ids(
        RepairDomain::Reboot,
        &[FactState::RebootRequired, FactState::Active],
    );
    if !reboot.is_empty() {
        push_diagnosis(
            &mut out,
            "PENDING_REBOOT",
            DiagnosisRole::ContributingCondition,
            RepairDomain::Reboot,
            DiagnosisConfidence::Confirmed,
            reboot,
            "A pending reboot is a servicing condition, not corruption.",
        );
    }

    let servicing = ids(
        RepairDomain::Servicing,
        &[FactState::Failure, FactState::Degraded, FactState::Active],
    );
    if !servicing.is_empty() {
        push_diagnosis(
            &mut out,
            "SERVICING_STATE_BLOCKED",
            DiagnosisRole::RootCause,
            RepairDomain::Servicing,
            DiagnosisConfidence::High,
            servicing,
            "Active servicing may be legitimate; mutation is blocked until contention clears.",
        );
    }

    let update_fail = ids(
        RepairDomain::WindowsUpdate,
        &[FactState::Failure, FactState::Degraded],
    );
    let update_offline = ids(RepairDomain::WindowsUpdate, &[FactState::Offline]);
    if !update_fail.is_empty() {
        let role = if out.iter().any(|d| {
            matches!(
                d.code.as_str(),
                "COMPONENT_STORE_CORRUPTION" | "SERVICING_STATE_BLOCKED" | "PENDING_REBOOT"
            )
        }) {
            DiagnosisRole::Symptom
        } else {
            DiagnosisRole::RootCause
        };
        push_diagnosis(
            &mut out,
            "WINDOWS_UPDATE_FAILURE",
            role,
            RepairDomain::WindowsUpdate,
            DiagnosisConfidence::Medium,
            update_fail,
            "Update HRESULT/category evidence may narrow cause; unknown codes remain unknown.",
        );
    } else if !update_offline.is_empty() {
        push_diagnosis(
            &mut out,
            "WINDOWS_UPDATE_OFFLINE",
            DiagnosisRole::Symptom,
            RepairDomain::WindowsUpdate,
            DiagnosisConfidence::Confirmed,
            update_offline,
            "Offline discovery is not evidence of update-store corruption.",
        );
    }

    let service_stopped = ids(RepairDomain::Services, &[FactState::Stopped]);
    if !service_stopped.is_empty() {
        push_diagnosis(
            &mut out,
            "REQUIRED_SERVICE_STOPPED",
            DiagnosisRole::RootCause,
            RepairDomain::Services,
            DiagnosisConfidence::High,
            service_stopped,
            "Only services linked to the active diagnosis are eligible for targeted repair.",
        );
    }
    let service_unexpected = ids(
        RepairDomain::Services,
        &[FactState::Disabled, FactState::UnexpectedConfiguration],
    );
    if !service_unexpected.is_empty() {
        push_diagnosis(
            &mut out,
            "SERVICE_CONFIGURATION_UNEXPECTED",
            DiagnosisRole::ContributingCondition,
            RepairDomain::Services,
            DiagnosisConfidence::Medium,
            service_unexpected,
            "Policy or enterprise management may intentionally configure the service; no blind template reset is permitted.",
        );
    }

    let dns = ids(RepairDomain::Dns, &[FactState::Failure]);
    if !dns.is_empty() {
        push_diagnosis(
            &mut out,
            "DNS_RESOLUTION_FAILURE",
            DiagnosisRole::RootCause,
            RepairDomain::Dns,
            DiagnosisConfidence::High,
            dns,
            "DNS is diagnosed only when DNS-specific evidence fails while lower-layer connectivity is otherwise available.",
        );
    }
    let network = ids(
        RepairDomain::Network,
        &[FactState::Failure, FactState::Offline, FactState::Degraded],
    );
    if !network.is_empty() {
        push_diagnosis(
            &mut out,
            "NETWORK_CONNECTIVITY_FAILURE",
            DiagnosisRole::RootCause,
            RepairDomain::Network,
            DiagnosisConfidence::Medium,
            network,
            "Generic connectivity failure does not imply DNS or Winsock corruption.",
        );
    }
    let proxy = ids(
        RepairDomain::Proxy,
        &[FactState::Failure, FactState::UnexpectedConfiguration],
    );
    if !proxy.is_empty() {
        push_diagnosis(
            &mut out,
            "PROXY_CONFIGURATION_PROBLEM",
            DiagnosisRole::ContributingCondition,
            RepairDomain::Proxy,
            DiagnosisConfidence::Medium,
            proxy,
            "Managed proxy policy must not be removed automatically.",
        );
    }

    let fs = ids(
        RepairDomain::Filesystem,
        &[
            FactState::Failure,
            FactState::Repairable,
            FactState::CorruptionDetected,
        ],
    );
    if !fs.is_empty() {
        push_diagnosis(
            &mut out,
            "FILESYSTEM_ERROR",
            DiagnosisRole::RootCause,
            RepairDomain::Filesystem,
            DiagnosisConfidence::High,
            fs,
            "Filesystem corruption is distinct from physical storage-health evidence.",
        );
    }
    let storage = ids(
        RepairDomain::StorageHardware,
        &[FactState::Failure, FactState::Degraded],
    );
    if !storage.is_empty() {
        push_diagnosis(
            &mut out,
            "STORAGE_HARDWARE_WARNING",
            DiagnosisRole::RootCause,
            RepairDomain::StorageHardware,
            DiagnosisConfidence::High,
            storage,
            "Hardware reliability is not repaired by filesystem mutation.",
        );
    }

    let winre = ids(RepairDomain::Recovery, &[FactState::Unavailable]);
    if !winre.is_empty() {
        push_diagnosis(
            &mut out,
            "RECOVERY_ENVIRONMENT_UNAVAILABLE",
            DiagnosisRole::ContributingCondition,
            RepairDomain::Recovery,
            DiagnosisConfidence::High,
            winre,
            "Reduced recovery readiness changes risk semantics for sensitive actions.",
        );
    }

    out.sort_by(|a, b| a.code.cmp(&b.code));
    out
}
