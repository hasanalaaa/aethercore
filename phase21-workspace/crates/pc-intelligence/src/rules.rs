use std::collections::BTreeMap;

use crate::{
    lifecycle::default_resolution_authority,
    model::*,
};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;
const DRIVER_CORRELATION_WINDOW_MS: i64 = 24 * 60 * 60 * 1000;
const HARDWARE_CORRELATION_WINDOW_MS: i64 = 30 * 60 * 1000;
const TIGHT_HARDWARE_CORRELATION_MS: i64 = 10 * 60 * 1000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleDescriptor {
    pub id: &'static str,
    pub version: u32,
    pub domain: Domain,
    pub description: &'static str,
}

pub const RULES: &[RuleDescriptor] = &[
    RuleDescriptor { id: "P17-DRV-001", version: 2, domain: Domain::Drivers, description: "Missing driver with authority-aware remediation semantics" },
    RuleDescriptor { id: "P17-DRV-002", version: 2, domain: Domain::Drivers, description: "PnP device problem with trusted-source coverage semantics" },
    RuleDescriptor { id: "P17-DRV-003", version: 2, domain: Domain::Drivers, description: "Authority-ranked official driver recommendation or guided official path" },
    RuleDescriptor { id: "P17-WIN-001", version: 1, domain: Domain::Windows, description: "Windows integrity assessment requires attention" },
    RuleDescriptor { id: "P17-STO-001", version: 1, domain: Domain::Storage, description: "Storage reports explicit reliability evidence" },
    RuleDescriptor { id: "P17-MEM-001", version: 1, domain: Domain::Memory, description: "Current memory pressure is sustained at a high threshold" },
    RuleDescriptor { id: "P17-HW-001", version: 2, domain: Domain::Hardware, description: "WHEA or hardware-error evidence exists with corrected/fatal semantics kept distinct" },
    RuleDescriptor { id: "P17-CRASH-001", version: 1, domain: Domain::Diagnostics, description: "Recent bugcheck/crash evidence exists" },
    RuleDescriptor { id: "P17-START-001", version: 1, domain: Domain::Startup, description: "Multiple high-impact startup entries are present" },
    RuleDescriptor { id: "P17-CLEAN-001", version: 1, domain: Domain::Cleanup, description: "Material reclaimable data is available" },
    RuleDescriptor { id: "P17-UPD-001", version: 1, domain: Domain::Updates, description: "AetherCore application update is available" },
    RuleDescriptor { id: "P17-CORR-001", version: 2, domain: Domain::Drivers, description: "Recent driver change is closely followed by crash evidence without claiming root cause" },
    RuleDescriptor { id: "P17-CORR-003", version: 2, domain: Domain::Hardware, description: "WHEA and crash evidence correlate only when proximity/recurrence supports a meaningful relationship" },
];

pub fn evaluate(facts: &[SystemFact], now_ms: i64) -> Vec<Finding> {
    let mut findings = Vec::new();
    for fact in facts {
        match &fact.payload {
            FactPayload::DeviceHealth { missing_driver: true, has_problem, problem_code, update_status, .. } => {
                let trusted_candidate = matches!(update_status.as_str(), "MissingDriverCandidateAvailable" | "RecommendedUpdateAvailable");
                let (code, title, summary, technical, safety, action) = if trusted_candidate {
                    ("DRIVER_MISSING", "finding.driverMissing.title", "finding.driverMissing.summary", "finding.driverMissing.technical", Some(RemediationSafety::Manual), ActionType::ManualVendorAction)
                } else {
                    ("NO_TRUSTED_CANDIDATE", "finding.noTrustedDriver.title", "finding.noTrustedDriver.summary", "finding.noTrustedDriver.technical", Some(RemediationSafety::Manual), ActionType::ManualVendorAction)
                };
                let severity = if *has_problem { Severity::High } else { Severity::Moderate };
                findings.push(finding(fact, code, Domain::Drivers, severity, Confidence::Confirmed, title, summary, technical, "P17-DRV-001", 2, safety, action));
                let _ = problem_code;
            }
            FactPayload::DeviceHealth { missing_driver: false, has_problem: true, update_status, .. } => {
                let no_trusted = update_status.contains("NoTrustedCandidate");
                findings.push(finding(
                    fact,
                    if no_trusted { "NO_TRUSTED_CANDIDATE" } else { "DEVICE_PROBLEM" },
                    Domain::Drivers,
                    Severity::Moderate,
                    Confidence::Confirmed,
                    if no_trusted { "finding.noTrustedDriver.title" } else { "finding.deviceProblem.title" },
                    if no_trusted { "finding.noTrustedDriver.summary" } else { "finding.deviceProblem.summary" },
                    if no_trusted { "finding.noTrustedDriver.technical" } else { "finding.deviceProblem.technical" },
                    "P17-DRV-002", 2, Some(RemediationSafety::Manual), ActionType::ManualVendorAction,
                ));
            }
            FactPayload::DeviceHealth { missing_driver: false, has_problem: false, update_status, authority_coverage, management_authorities, .. } => {
                if !management_authorities.is_empty() {
                    findings.push(finding(fact, "DRIVER_MANAGEMENT_AUTHORITY_AVAILABLE", Domain::Drivers, Severity::Informational, Confidence::Confirmed, "finding.driverManagement.title", "finding.driverManagement.summary", "finding.driverManagement.technical", "P18.1-DRV-001", 1, Some(RemediationSafety::Manual), ActionType::ManualVendorAction));
                }
                if authority_coverage != "CompleteForRequiredAuthorities" {
                    findings.push(finding(fact, "DRIVER_AUTHORITY_COVERAGE_INCOMPLETE", Domain::Drivers, Severity::Informational, Confidence::Confirmed, "finding.driverCoverage.title", "finding.driverCoverage.summary", "finding.driverCoverage.technical", "P18.1-DRV-002", 1, None, ActionType::ManualVendorAction));
                }
                if matches!(update_status.as_str(), "UpdateStatusUnknown" | "UpdateStatusUnknownOffline" | "ProviderUnavailable") {
                    findings.push(finding(fact, "DRIVER_UPDATE_STATUS_UNKNOWN", Domain::Drivers, Severity::Informational, Confidence::Confirmed, "finding.driverStatusUnknown.title", "finding.driverStatusUnknown.summary", "finding.driverStatusUnknown.technical", "P18.1-DRV-003", 1, None, ActionType::ManualVendorAction));
                }
            }
            FactPayload::DriverUpdate { recommendation_state, installation_mode, trust_state, selectable, .. } => {
                let executable = *selectable
                    && matches!(recommendation_state.as_str(), "Recommended" | "Optional")
                    && matches!(installation_mode.as_str(), "WindowsManaged" | "DirectTrusted")
                    && matches!(trust_state.as_str(), "WindowsManaged" | "TrustedSignature" | "TrustedSignatureAndDigest");
                let firmware = recommendation_state == "FirmwareProtected";
                let (code, severity, safety, action, title, summary, technical) = if firmware {
                    ("FIRMWARE_REVIEW_REQUIRED", Severity::Informational, Some(RemediationSafety::Manual), ActionType::ManualVendorAction, "finding.firmwareDriver.title", "finding.firmwareDriver.summary", "finding.firmwareDriver.technical")
                } else if executable {
                    ("DRIVER_UPDATE_AVAILABLE", Severity::Low, Some(RemediationSafety::SafeReview), ActionType::InstallDriver, "finding.driverUpdate.title", "finding.driverUpdate.summary", "finding.driverUpdate.technical")
                } else {
                    ("DRIVER_UPDATE_AVAILABLE", Severity::Informational, None, ActionType::ManualVendorAction, "finding.driverUpdate.title", "finding.driverUpdate.summary", "finding.driverUpdate.technical")
                };
                findings.push(finding(fact, code, Domain::Drivers, severity, Confidence::High, title, summary, technical, "P18.1-DRV-004", 1, safety, action));
            }
            FactPayload::WindowsIntegrity { result_code, .. }
                if is_actionable_integrity_attention(result_code) => findings.push(finding(
                    fact,
                    "WINDOWS_INTEGRITY_ATTENTION",
                    Domain::Windows,
                    Severity::High,
                    Confidence::High,
                    "finding.windowsIntegrity.title",
                    "finding.windowsIntegrity.summary",
                    "finding.windowsIntegrity.technical",
                    "P17-WIN-001",
                    1,
                    Some(RemediationSafety::Sensitive),
                    ActionType::RepairWindows,
                )),
            FactPayload::StorageHealth {
                health_status,
                source_severity,
                uncorrected_read_errors,
                uncorrected_write_errors,
                nvme_critical_warning,
                nvme_media_errors_nonzero,
                wear_percent,
                temperature_c,
                temperature_max_c,
            } => {
                let explicit = health_status.eq_ignore_ascii_case("Unhealthy")
                    || source_severity.eq_ignore_ascii_case("ActionRequired")
                    || uncorrected_read_errors.is_some_and(|value| value > 0)
                    || uncorrected_write_errors.is_some_and(|value| value > 0)
                    || nvme_critical_warning.is_some_and(|value| value != 0)
                    || *nvme_media_errors_nonzero;
                let attention = health_status.eq_ignore_ascii_case("Warning")
                    || wear_percent.is_some_and(|value| value >= 100)
                    || matches!((temperature_c, temperature_max_c), (Some(current), Some(maximum)) if *maximum > 0 && *current >= *maximum);
                if explicit {
                    findings.push(finding(
                        fact,
                        "STORAGE_RELIABILITY_CONCERN",
                        Domain::Storage,
                        Severity::Critical,
                        Confidence::Confirmed,
                        "finding.storageConcern.title",
                        "finding.storageConcern.summary",
                        "finding.storageConcern.technical",
                        "P17-STO-001",
                        1,
                        Some(RemediationSafety::HardwareService),
                        ActionType::ReviewStorage,
                    ));
                } else if attention {
                    findings.push(finding(
                        fact,
                        "STORAGE_ATTENTION",
                        Domain::Storage,
                        Severity::Moderate,
                        Confidence::High,
                        "finding.storageAttention.title",
                        "finding.storageAttention.summary",
                        "finding.storageAttention.technical",
                        "P17-STO-001",
                        1,
                        Some(RemediationSafety::SafeReview),
                        ActionType::ReviewStorage,
                    ));
                }
            }
            FactPayload::MemoryPressure { memory_load_percent, .. } if *memory_load_percent >= 95 => findings.push(finding(
                fact,
                "HIGH_MEMORY_PRESSURE",
                Domain::Memory,
                Severity::Moderate,
                Confidence::Confirmed,
                "finding.memoryPressure.title",
                "finding.memoryPressure.summary",
                "finding.memoryPressure.technical",
                "P17-MEM-001",
                1,
                None,
                ActionType::ManualVendorAction,
            )),
            FactPayload::HardwareEvent { provider, event_id, .. } => {
                let (severity, confidence) = match whea_disposition(provider, *event_id) {
                    WheaDisposition::Fatal => (Severity::High, Confidence::Confirmed),
                    WheaDisposition::Corrected => (Severity::Moderate, Confidence::High),
                    WheaDisposition::Unknown => (Severity::Moderate, Confidence::Medium),
                    WheaDisposition::NotWhea => (Severity::Moderate, Confidence::Medium),
                };
                findings.push(finding(
                    fact,
                    "HARDWARE_ERROR_EVIDENCE",
                    Domain::Hardware,
                    severity,
                    confidence,
                    "finding.hardwareEvidence.title",
                    "finding.hardwareEvidence.summary",
                    "finding.hardwareEvidence.technical",
                    "P17-HW-001",
                    2,
                    Some(RemediationSafety::HardwareService),
                    ActionType::ReviewHardwareError,
                ));
            }
            FactPayload::Crash { .. } if observed_within(fact.observed_unix_ms, now_ms, 30 * DAY_MS) => findings.push(finding(
                fact,
                "RECENT_CRASH_EVIDENCE",
                Domain::Diagnostics,
                Severity::Moderate,
                Confidence::Confirmed,
                "finding.recentCrash.title",
                "finding.recentCrash.summary",
                "finding.recentCrash.technical",
                "P17-CRASH-001",
                1,
                Some(RemediationSafety::SafeReview),
                ActionType::ReviewCrashEvidence,
            )),
            FactPayload::StartupFootprint { high_impact_count, .. } if *high_impact_count >= 3 => findings.push(finding(
                fact,
                "HIGH_STARTUP_FOOTPRINT",
                Domain::Startup,
                Severity::Low,
                Confidence::High,
                "finding.startupFootprint.title",
                "finding.startupFootprint.summary",
                "finding.startupFootprint.technical",
                "P17-START-001",
                1,
                Some(RemediationSafety::SafeReview),
                ActionType::DisableStartupItem,
            )),
            FactPayload::CleanupOpportunity { reclaimable_bytes, .. } if *reclaimable_bytes >= 512 * 1024 * 1024 => findings.push(finding(
                fact,
                "CLEANUP_OPPORTUNITY",
                Domain::Cleanup,
                Severity::Informational,
                Confidence::Confirmed,
                "finding.cleanupOpportunity.title",
                "finding.cleanupOpportunity.summary",
                "finding.cleanupOpportunity.technical",
                "P17-CLEAN-001",
                1,
                Some(RemediationSafety::SafeReview),
                ActionType::CleanupData,
            )),
            FactPayload::UpdateState { update_available: true, .. } => findings.push(finding(
                fact,
                "APP_UPDATE_AVAILABLE",
                Domain::Updates,
                Severity::Informational,
                Confidence::Confirmed,
                "finding.appUpdate.title",
                "finding.appUpdate.summary",
                "finding.appUpdate.technical",
                "P17-UPD-001",
                1,
                Some(RemediationSafety::Sensitive),
                ActionType::InstallAppUpdate,
            )),
            _ => {}
        }
    }
    correlate_driver_change(facts, now_ms, &mut findings);
    correlate_hardware_crash(facts, now_ms, &mut findings);
    findings.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| a.code.cmp(&b.code))
            .then_with(|| a.id.cmp(&b.id))
    });
    findings.dedup_by(|a, b| a.id == b.id);
    findings
}

fn correlate_driver_change(facts: &[SystemFact], now_ms: i64, findings: &mut Vec<Finding>) {
    let changes = facts
        .iter()
        .filter(|fact| matches!(fact.payload, FactPayload::DriverChange { .. }))
        .collect::<Vec<_>>();
    let crashes = facts
        .iter()
        .filter(|fact| matches!(fact.payload, FactPayload::Crash { .. }))
        .collect::<Vec<_>>();

    for change in changes {
        let Some(crash) = crashes
            .iter()
            .copied()
            .filter(|crash| {
                observed_within(change.observed_unix_ms, now_ms, 7 * DAY_MS)
                    && observed_within(crash.observed_unix_ms, now_ms, 7 * DAY_MS)
                    && crash.observed_unix_ms >= change.observed_unix_ms
            })
            .filter(|crash| crash.observed_unix_ms - change.observed_unix_ms <= DRIVER_CORRELATION_WINDOW_MS)
            .min_by_key(|crash| crash.observed_unix_ms - change.observed_unix_ms)
        else {
            continue;
        };
        let distance_ms = (crash.observed_unix_ms - change.observed_unix_ms) as u64;
        let mut correlated = finding(
            change,
            "POSSIBLE_DRIVER_REGRESSION",
            Domain::Drivers,
            Severity::Moderate,
            Confidence::Medium,
            "finding.driverRegression.title",
            "finding.driverRegression.summary",
            "finding.driverRegression.technical",
            "P17-CORR-001",
            2,
            Some(RemediationSafety::Sensitive),
            ActionType::InstallDriver,
        );
        correlated.evidence.extend(crash.evidence.clone());
        correlated.message_args.insert("timeDistanceSeconds".into(), (distance_ms / 1000).to_string());
        correlated.correlation = Some(CorrelationExplanation {
            strength: CorrelationStrength::Moderate,
            time_distance_ms: distance_ms,
            shared_scope: "driver-change-before-crash".into(),
            rationale_key: "finding.correlation.driverChangeCloseToCrash".into(),
            contributing_fact_ids: vec![change.id.clone(), crash.id.clone()],
            conflicting_evidence_keys: vec!["finding.correlation.noCrashModuleAttribution".into()],
        });
        findings.push(correlated);
    }
}

fn correlate_hardware_crash(facts: &[SystemFact], now_ms: i64, findings: &mut Vec<Finding>) {
    let hardware = facts
        .iter()
        .filter(|fact| matches!(fact.payload, FactPayload::HardwareEvent { .. }))
        .filter(|fact| observed_within(fact.observed_unix_ms, now_ms, 30 * DAY_MS))
        .collect::<Vec<_>>();
    let crashes = facts
        .iter()
        .filter(|fact| matches!(fact.payload, FactPayload::Crash { .. }))
        .filter(|fact| observed_within(fact.observed_unix_ms, now_ms, 30 * DAY_MS))
        .collect::<Vec<_>>();

    let mut pairs = Vec::new();
    for hw in &hardware {
        let FactPayload::HardwareEvent { provider, event_id, category } = &hw.payload else {
            continue;
        };
        if !provider.eq_ignore_ascii_case("Microsoft-Windows-WHEA-Logger") {
            continue;
        }
        for crash in &crashes {
            let signed_delta = crash.observed_unix_ms.saturating_sub(hw.observed_unix_ms);
            let distance_ms = signed_delta.unsigned_abs();
            if distance_ms <= HARDWARE_CORRELATION_WINDOW_MS as u64 {
                pairs.push((
                    *hw,
                    *crash,
                    distance_ms,
                    signed_delta,
                    whea_disposition(provider, *event_id),
                    category.clone(),
                ));
            }
        }
    }
    if pairs.is_empty() {
        return;
    }
    pairs.sort_by_key(|(_, _, distance_ms, _, _, _)| *distance_ms);

    let causal_tight_pairs = pairs
        .iter()
        .filter(|(_, _, distance_ms, signed_delta, _, _)| {
            *signed_delta >= 0 && *distance_ms <= TIGHT_HARDWARE_CORRELATION_MS as u64
        })
        .collect::<Vec<_>>();
    let distinct_hw = causal_tight_pairs
        .iter()
        .map(|(hw, _, _, _, _, _)| hw.id.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let distinct_crashes = causal_tight_pairs
        .iter()
        .map(|(_, crash, _, _, _, _)| crash.id.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();

    let (hw, crash, distance_ms, signed_delta, disposition, category) = &pairs[0];
    let repeated_tight_relationship = causal_tight_pairs.len() >= 2 && distinct_hw >= 2 && distinct_crashes >= 2;
    let strength = if repeated_tight_relationship
        || (*disposition == WheaDisposition::Fatal
            && *signed_delta >= 0
            && *distance_ms <= TIGHT_HARDWARE_CORRELATION_MS as u64)
    {
        CorrelationStrength::Strong
    } else if *distance_ms <= TIGHT_HARDWARE_CORRELATION_MS as u64 {
        CorrelationStrength::Moderate
    } else {
        CorrelationStrength::Weak
    };

    // Weak broad coincidence is intentionally not promoted to a combined Finding.
    if strength == CorrelationStrength::Weak {
        return;
    }

    let (severity, confidence) = match strength {
        CorrelationStrength::Strong => (Severity::High, Confidence::High),
        CorrelationStrength::Moderate => (Severity::Moderate, Confidence::Medium),
        CorrelationStrength::Weak => (Severity::Informational, Confidence::Low),
    };
    let mut correlated = finding(
        hw,
        "CRASH_WITH_HARDWARE_EVIDENCE",
        Domain::Hardware,
        severity,
        confidence,
        "finding.crashHardware.title",
        "finding.crashHardware.summary",
        "finding.crashHardware.technical",
        "P17-CORR-003",
        2,
        Some(RemediationSafety::HardwareService),
        ActionType::ReviewHardwareError,
    );
    correlated.evidence.extend(crash.evidence.clone());
    if repeated_tight_relationship {
        for (extra_hw, extra_crash, _, _, _, _) in pairs.iter().skip(1).take(3) {
            correlated.evidence.extend(extra_hw.evidence.clone());
            correlated.evidence.extend(extra_crash.evidence.clone());
        }
    }
    correlated.message_args.insert("timeDistanceSeconds".into(), (distance_ms / 1000).to_string());
    correlated.message_args.insert("hardwareCategory".into(), category.clone());
    let mut conflicts = Vec::new();
    if *disposition == WheaDisposition::Corrected {
        conflicts.push("finding.correlation.correctedHardwareEvidence".into());
    }
    if *signed_delta < -2 * 60 * 1000 {
        conflicts.push("finding.correlation.hardwareEvidenceAfterCrash".into());
    }
    correlated.correlation = Some(CorrelationExplanation {
        strength,
        time_distance_ms: *distance_ms,
        shared_scope: category.clone(),
        rationale_key: if repeated_tight_relationship {
            "finding.correlation.repeatedHardwareCrashCluster".into()
        } else if *disposition == WheaDisposition::Fatal {
            "finding.correlation.fatalHardwareNearCrash".into()
        } else {
            "finding.correlation.hardwareNearCrash".into()
        },
        contributing_fact_ids: vec![hw.id.clone(), crash.id.clone()],
        conflicting_evidence_keys: conflicts,
    });
    findings.push(correlated);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WheaDisposition {
    Fatal,
    Corrected,
    Unknown,
    NotWhea,
}

fn whea_disposition(provider: &str, event_id: u32) -> WheaDisposition {
    if !provider.eq_ignore_ascii_case("Microsoft-Windows-WHEA-Logger") {
        return WheaDisposition::NotWhea;
    }
    match event_id {
        // WHEA-Logger 18 is the fatal hardware-error event. 17 and 19 are
        // corrected-error families and must never be treated as fatal evidence.
        18 => WheaDisposition::Fatal,
        17 | 19 => WheaDisposition::Corrected,
        _ => WheaDisposition::Unknown,
    }
}

fn observed_within(observed: i64, now: i64, window: i64) -> bool {
    observed >= 0 && observed <= now && now - observed <= window
}

fn is_healthy_integrity(result_code: &str, exit_code: i32) -> bool {
    exit_code == 0 && matches!(result_code,
        "ExitCode0" | "NoErrors" | "ComponentStoreHealthy" | "SystemFilesHealthy" | "ServicingAvailable" | "UpdateHealthy" | "NetworkHealthy" | "DnsHealthy" | "ProxyHealthy" | "WinReAvailable" | "RestoreAvailable"
    )
}

fn is_actionable_integrity_attention(result_code: &str) -> bool {
    matches!(result_code,
        "ComponentStoreRepairable" | "ComponentStoreCorruptionDetected" | "ComponentStoreNonRepairable" |
        "SystemFilesCorrupt" | "SystemFilesRepairable" | "SystemFilesRepairFailed" |
        "ServicingBusy" | "RebootPending" | "UpdateFailure" | "ServiceStopped" | "ServiceDisabled" |
        "DnsFailure" | "ProxyUnexpected" | "WinReUnavailable" | "RestoreUnavailable"
    ) || result_code.starts_with("ChkdskExit")
}

pub fn explicitly_healthy(fact: &SystemFact) -> bool {
    match &fact.payload {
        FactPayload::DeviceHealth { missing_driver, has_problem, .. } => !*missing_driver && !*has_problem,
        FactPayload::WindowsIntegrity { result_code, exit_code, .. } => is_healthy_integrity(result_code, *exit_code),
        FactPayload::StorageHealth {
            health_status,
            source_severity,
            uncorrected_read_errors,
            uncorrected_write_errors,
            nvme_critical_warning,
            nvme_media_errors_nonzero,
            wear_percent,
            temperature_c,
            temperature_max_c,
        } => {
            health_status.eq_ignore_ascii_case("Healthy")
                && !source_severity.eq_ignore_ascii_case("ActionRequired")
                && !source_severity.eq_ignore_ascii_case("Warning")
                // DBT-P46-B3: `Some(0)`, not `0`. An unread counter cannot
                // supply the proof that resolves an open storage finding.
                && *uncorrected_read_errors == Some(0)
                && *uncorrected_write_errors == Some(0)
                && *nvme_critical_warning == Some(0)
                && !*nvme_media_errors_nonzero
                && !wear_percent.is_some_and(|value| value >= 100)
                && !matches!((temperature_c, temperature_max_c), (Some(current), Some(maximum)) if *maximum > 0 && *current >= *maximum)
        }
        FactPayload::MemoryPressure { memory_load_percent, pressure_label } => {
            *memory_load_percent < 95 && !pressure_label.eq_ignore_ascii_case("Critical")
        }
        FactPayload::StartupFootprint { high_impact_count, .. } => *high_impact_count < 3,
        FactPayload::UpdateState { update_available, failed, .. } => !*update_available && !*failed,
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn finding(
    fact: &SystemFact,
    code: &str,
    domain: Domain,
    severity: Severity,
    confidence: Confidence,
    title_key: &str,
    summary_key: &str,
    technical_key: &str,
    rule_id: &str,
    rule_version: u32,
    safety: Option<RemediationSafety>,
    action_type: ActionType,
) -> Finding {
    let id = stable_id("finding", &format!("{code}|{}", fact.resource.stable_id));
    let mut message_args = BTreeMap::new();
    message_args.insert("resource".into(), fact.resource.display_name.clone());
    let (privilege, reboot_requirement, reversibility, automatic_eligible) =
        remediation_traits(action_type, safety);
    Finding {
        id,
        code: code.into(),
        domain,
        severity,
        confidence,
        title_key: title_key.into(),
        summary_key: summary_key.into(),
        technical_key: technical_key.into(),
        message_args,
        evidence: fact.evidence.clone(),
        affected_resource: fact.resource.clone(),
        first_observed_unix_ms: fact.observed_unix_ms,
        last_observed_unix_ms: fact.observed_unix_ms,
        lifecycle: FindingLifecycle::New,
        remediation_available: safety.is_some(),
        remediation_safety: safety,
        reboot_requirement,
        privilege_requirement: privilege,
        automatic_eligible,
        reversibility,
        estimated_impact: match severity {
            Severity::Critical | Severity::High => ImpactEstimate::High,
            Severity::Moderate => ImpactEstimate::Moderate,
            Severity::Low => ImpactEstimate::Low,
            Severity::Informational => ImpactEstimate::Minimal,
        },
        uncertainty_key: if confidence >= Confidence::High {
            "finding.uncertainty.low".into()
        } else {
            "finding.uncertainty.review".into()
        },
        ignored: false,
        rule_id: rule_id.into(),
        rule_version,
        verification_status: FindingVerificationStatus::ConfirmedCurrent,
        resolution_authority: default_resolution_authority(code),
        resolved_at_unix_ms: None,
        resolution_scan_id: String::new(),
        resolution_reason_key: String::new(),
        resolution_evidence: Vec::new(),
        correlation: None,
    }
}

fn remediation_traits(
    action: ActionType,
    safety: Option<RemediationSafety>,
) -> (PrivilegeRequirement, RebootRequirement, Reversibility, bool) {
    match action {
        ActionType::InstallDriver => (
            PrivilegeRequirement::ElevatedService,
            RebootRequirement::Possible,
            Reversibility::Checkpointed,
            false,
        ),
        ActionType::RepairWindows => (
            PrivilegeRequirement::ElevatedService,
            RebootRequirement::Possible,
            Reversibility::Limited,
            false,
        ),
        ActionType::ReviewStorage | ActionType::ReviewHardwareError | ActionType::ReviewCrashEvidence => (
            PrivilegeRequirement::None,
            RebootRequirement::None,
            Reversibility::NotSoftwareReversible,
            false,
        ),
        ActionType::DisableStartupItem => (
            PrivilegeRequirement::ElevatedService,
            RebootRequirement::None,
            Reversibility::Reversible,
            false,
        ),
        ActionType::CleanupData => (
            PrivilegeRequirement::ElevatedService,
            RebootRequirement::None,
            Reversibility::Limited,
            matches!(safety, Some(RemediationSafety::SafeAuto)),
        ),
        ActionType::InstallAppUpdate => (
            PrivilegeRequirement::ElevatedService,
            RebootRequirement::Possible,
            Reversibility::Limited,
            false,
        ),
        ActionType::ManualVendorAction => (
            PrivilegeRequirement::VendorTool,
            RebootRequirement::Possible,
            Reversibility::Limited,
            false,
        ),
        ActionType::HardwareService => (
            PrivilegeRequirement::VendorTool,
            RebootRequirement::None,
            Reversibility::NotSoftwareReversible,
            false,
        ),
    }
}

pub fn remediation_candidates(findings: &[Finding]) -> Vec<RemediationCandidate> {
    let mut out = Vec::new();
    for finding in findings {
        if finding.verification_status != FindingVerificationStatus::ConfirmedCurrent
            || finding.ignored
            || finding.lifecycle == FindingLifecycle::Resolved
        {
            continue;
        }
        let Some(safety) = finding.remediation_safety else {
            continue;
        };
        let action_type = match finding.code.as_str() {
            "POSSIBLE_DRIVER_REGRESSION" => ActionType::InstallDriver,
            "DRIVER_UPDATE_AVAILABLE" if safety == RemediationSafety::Manual => ActionType::ManualVendorAction,
            "DRIVER_UPDATE_AVAILABLE" => ActionType::InstallDriver,
            "DRIVER_MISSING" | "DEVICE_PROBLEM" | "NO_TRUSTED_CANDIDATE" | "VENDOR_UTILITY_REQUIRED" | "FIRMWARE_REVIEW_REQUIRED" | "DRIVER_MANAGEMENT_AUTHORITY_AVAILABLE" => ActionType::ManualVendorAction,
            "WINDOWS_INTEGRITY_ATTENTION" => ActionType::RepairWindows,
            "STORAGE_RELIABILITY_CONCERN" | "STORAGE_ATTENTION" => ActionType::ReviewStorage,
            "HARDWARE_ERROR_EVIDENCE" | "CRASH_WITH_HARDWARE_EVIDENCE" => ActionType::ReviewHardwareError,
            "RECENT_CRASH_EVIDENCE" => ActionType::ReviewCrashEvidence,
            "HIGH_STARTUP_FOOTPRINT" => ActionType::DisableStartupItem,
            "CLEANUP_OPPORTUNITY" => ActionType::CleanupData,
            "APP_UPDATE_AVAILABLE" => ActionType::InstallAppUpdate,
            _ => continue,
        };
        let (privilege, reboot_requirement, reversibility, automatic_eligible) =
            remediation_traits(action_type, Some(safety));
        out.push(RemediationCandidate {
            action_id: stable_id("action", &format!("{}|{:?}", finding.id, action_type)),
            finding_id: finding.id.clone(),
            action_type,
            description_key: format!("remediation.{}", finding.code.to_ascii_lowercase()),
            authority: match action_type {
                ActionType::InstallDriver => "driver-hub",
                ActionType::RepairWindows => "system-repair",
                ActionType::DisableStartupItem => "startup-manager",
                ActionType::CleanupData => "cleaner",
                ActionType::InstallAppUpdate => "update-engine",
                _ => "human-review",
            }
            .into(),
            privilege,
            safety,
            reversibility,
            reboot_requirement,
            expected_effect_key: "remediation.expectedEffect".into(),
            preconditions: vec!["findingStillActive".into(), "evidenceNotStale".into()],
            verification_method_key: "remediation.verifyAfterExecution".into(),
            conflicts: Vec::new(),
            duration_category: "bounded".into(),
            automatic_eligible,
        });
    }
    out.sort_by(|a, b| a.action_id.cmp(&b.action_id));
    out
}

#[cfg(test)]
mod correlation_tests {
    use super::*;

    const NOW: i64 = 1_900_000_000_000;

    fn fact(domain: Domain, id: &str, at: i64, payload: FactPayload) -> SystemFact {
        SystemFact::new(
            domain,
            "test",
            ResourceRef::global("test", id, id),
            at,
            Freshness::Recent,
            Confidence::Confirmed,
            payload,
            EvidenceKind::HardwareEvent,
            id,
        )
    }

    fn whea(id: &str, at: i64, event_id: u32, category: &str) -> SystemFact {
        fact(
            Domain::Hardware,
            id,
            at,
            FactPayload::HardwareEvent {
                category: category.into(),
                provider: "Microsoft-Windows-WHEA-Logger".into(),
                event_id,
            },
        )
    }

    fn crash(id: &str, at: i64) -> SystemFact {
        fact(
            Domain::Diagnostics,
            id,
            at,
            FactPayload::Crash {
                crash_id: id.into(),
                bugcheck_hex: "0x124".into(),
            },
        )
    }

    #[test]
    fn fatal_whea_immediately_before_crash_is_strong_not_causal_claim() {
        let findings = evaluate(&[
            whea("whea", NOW - 42_000, 18, "ProcessorHardwareEvidence"),
            crash("crash", NOW),
        ], NOW);
        let correlated = findings.iter().find(|finding| finding.code == "CRASH_WITH_HARDWARE_EVIDENCE").unwrap();
        assert_eq!(correlated.correlation.as_ref().unwrap().strength, CorrelationStrength::Strong);
        assert_eq!(correlated.severity, Severity::High);
        assert_ne!(correlated.confidence, Confidence::Confirmed);
    }

    #[test]
    fn corrected_whea_days_before_unrelated_crash_never_creates_critical_correlation() {
        let findings = evaluate(&[
            whea("whea", NOW - 4 * DAY_MS, 17, "PcieHardwareEvidence"),
            crash("crash", NOW),
        ], NOW);
        assert!(!findings.iter().any(|finding| finding.code == "CRASH_WITH_HARDWARE_EVIDENCE"));
    }

    #[test]
    fn repeated_tight_whea_and_crashes_raise_strength() {
        let findings = evaluate(&[
            whea("whea-1", NOW - 120_000, 17, "ProcessorHardwareEvidence"),
            crash("crash-1", NOW - 100_000),
            whea("whea-2", NOW - 60_000, 17, "ProcessorHardwareEvidence"),
            crash("crash-2", NOW - 30_000),
        ], NOW);
        let correlated = findings.iter().find(|finding| finding.code == "CRASH_WITH_HARDWARE_EVIDENCE").unwrap();
        assert_eq!(correlated.correlation.as_ref().unwrap().strength, CorrelationStrength::Strong);
    }

    #[test]
    fn crash_without_whea_has_no_hardware_correlation() {
        let findings = evaluate(&[crash("crash", NOW)], NOW);
        assert!(!findings.iter().any(|finding| finding.code == "CRASH_WITH_HARDWARE_EVIDENCE"));
    }

    #[test]
    fn whea_without_crash_remains_independent_hardware_evidence() {
        let findings = evaluate(&[whea("whea", NOW, 17, "PcieHardwareEvidence")], NOW);
        assert!(findings.iter().any(|finding| finding.code == "HARDWARE_ERROR_EVIDENCE"));
        assert!(!findings.iter().any(|finding| finding.code == "CRASH_WITH_HARDWARE_EVIDENCE"));
    }

    #[test]
    fn corrected_or_after_crash_evidence_exposes_conflict() {
        let findings = evaluate(&[
            crash("crash", NOW - 5 * 60 * 1000),
            whea("whea", NOW, 17, "ProcessorHardwareEvidence"),
        ], NOW);
        let correlated = findings.iter().find(|finding| finding.code == "CRASH_WITH_HARDWARE_EVIDENCE").unwrap();
        let explanation = correlated.correlation.as_ref().unwrap();
        assert!(explanation.conflicting_evidence_keys.iter().any(|key| key.contains("corrected")));
        assert!(explanation.conflicting_evidence_keys.iter().any(|key| key.contains("AfterCrash")));
        assert_eq!(explanation.strength, CorrelationStrength::Moderate);
    }

    #[test]
    fn stale_evidence_does_not_dominate_current_scan() {
        let findings = evaluate(&[
            whea("whea", NOW - 31 * DAY_MS, 18, "ProcessorHardwareEvidence"),
            crash("crash", NOW - 31 * DAY_MS + 1_000),
        ], NOW);
        assert!(!findings.iter().any(|finding| finding.code == "CRASH_WITH_HARDWARE_EVIDENCE"));
    }
}
