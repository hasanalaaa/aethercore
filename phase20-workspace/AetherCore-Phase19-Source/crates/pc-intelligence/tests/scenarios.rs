use aethercore_pc_intelligence::{
    evaluate_rules, remediation_candidates, Confidence, CorrelationStrength, Domain, EvidenceKind, FactPayload,
    Freshness, RemediationPlan, RemediationSafety, ResourceRef, Severity, SystemFact,
};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;
const NOW: i64 = 1_800_000_000_000;

fn fact(domain: Domain, resource: &str, observed: i64, payload: FactPayload) -> SystemFact {
    SystemFact::new(
        domain,
        "phase17-test",
        ResourceRef::private("test-resource", resource, resource),
        observed,
        Freshness::Current,
        Confidence::Confirmed,
        payload,
        EvidenceKind::DeviceState,
        "synthetic-evidence",
    )
}

fn codes(facts: &[SystemFact]) -> Vec<String> {
    evaluate_rules(facts, NOW)
        .into_iter()
        .map(|finding| finding.code)
        .collect()
}

#[test]
fn healthy_pc_does_not_invent_findings() {
    let facts = vec![
        fact(
            Domain::Storage,
            "disk0",
            NOW,
            FactPayload::StorageHealth {
                health_status: "Healthy".into(),
                source_severity: "Healthy".into(),
                uncorrected_read_errors: 0,
                uncorrected_write_errors: 0,
                nvme_critical_warning: 0,
                nvme_media_errors_nonzero: false,
                wear_percent: Some(12),
                temperature_c: Some(38),
                temperature_max_c: Some(80),
            },
        ),
        fact(
            Domain::Memory,
            "memory",
            NOW,
            FactPayload::MemoryPressure {
                memory_load_percent: 44,
                pressure_label: "Normal".into(),
            },
        ),
        fact(
            Domain::Startup,
            "startup",
            NOW,
            FactPayload::StartupFootprint {
                high_impact_count: 0,
                manageable_count: 3,
                total_count: 5,
            },
        ),
    ];
    assert!(evaluate_rules(&facts, NOW).is_empty());
}

#[test]
fn missing_driver_is_explicit_and_confirmed() {
    let findings = evaluate_rules(
        &[fact(
            Domain::Drivers,
            "pci-device",
            NOW,
            FactPayload::DeviceHealth {
                missing_driver: true,
                has_problem: true,
                problem_code: 28,
                device_state: "Problem".into(),
                update_status: "MissingDriverCandidateAvailable".into(),
                authority_coverage: "CompleteForRequiredAuthorities".into(),
                management_authorities: Vec::new(),
            },
        )],
        NOW,
    );
    let finding = findings.iter().find(|f| f.code == "DRIVER_MISSING").unwrap();
    assert_eq!(finding.severity, Severity::High);
    assert_eq!(finding.confidence, Confidence::Confirmed);
    assert_eq!(finding.remediation_safety, Some(RemediationSafety::Manual));
}

#[test]
fn driver_update_without_authority_is_informational_and_not_executable() {
    let facts = [fact(
        Domain::Drivers,
        "pci-device",
        NOW,
        FactPayload::DriverUpdate {
            candidate_id: "candidate-unknown".into(),
            current_version: "1.0".into(),
            target_version: "2.0".into(),
            vendor_managed: false,
            selectable: false,
            authority_type: "WindowsUpdate".into(),
            authority_name: "Windows Update".into(),
            recommendation_state: "Recommended".into(),
            installation_mode: "WindowsManaged".into(),
            trust_state: "WindowsManaged".into(),
            authority_coverage: "CompleteForRequiredAuthorities".into(),
        },
    )];
    let findings = evaluate_rules(&facts, NOW);
    let finding = findings.iter().find(|finding| finding.code == "DRIVER_UPDATE_AVAILABLE").unwrap();
    assert_eq!(finding.severity, Severity::Informational);
    assert!(!finding.remediation_available);
    assert!(aethercore_pc_intelligence::remediation_candidates(&findings).is_empty());
}

#[test]
fn vendor_managed_driver_update_stays_manual() {
    let facts = [fact(
        Domain::Drivers,
        "gpu",
        NOW,
        FactPayload::DriverUpdate {
            candidate_id: "candidate-vendor".into(),
            current_version: "1.0".into(),
            target_version: "2.0".into(),
            vendor_managed: true,
            selectable: false,
            authority_type: "VendorUtility".into(),
            authority_name: "Vendor application".into(),
            recommendation_state: "ManualOfficial".into(),
            installation_mode: "OfficialUtility".into(),
            trust_state: "TrustedSignature".into(),
            authority_coverage: "ManualAuthorityRequired".into(),
        },
    )];
    let findings = evaluate_rules(&facts, NOW);
    let update_finding = findings.iter().find(|f| f.code == "DRIVER_UPDATE_AVAILABLE").unwrap();
    assert_eq!(update_finding.severity, Severity::Informational);
    assert_ne!(update_finding.remediation_safety, Some(RemediationSafety::SafeReview));
    assert_ne!(update_finding.remediation_safety, Some(RemediationSafety::SafeAuto));
    let candidates = aethercore_pc_intelligence::remediation_candidates(&findings);
    for candidate in &candidates {
        assert_ne!(candidate.safety, RemediationSafety::SafeAuto);
        assert_ne!(candidate.action_type, aethercore_pc_intelligence::ActionType::InstallDriver);
    }
}

#[test]
fn storage_explicit_errors_are_critical_without_fake_percentage() {
    let findings = evaluate_rules(
        &[fact(
            Domain::Storage,
            "nvme0",
            NOW,
            FactPayload::StorageHealth {
                health_status: "Healthy".into(),
                source_severity: "Healthy".into(),
                uncorrected_read_errors: 1,
                uncorrected_write_errors: 0,
                nvme_critical_warning: 0,
                nvme_media_errors_nonzero: false,
                wear_percent: Some(33),
                temperature_c: Some(42),
                temperature_max_c: Some(80),
            },
        )],
        NOW,
    );
    let finding = findings
        .iter()
        .find(|f| f.code == "STORAGE_RELIABILITY_CONCERN")
        .unwrap();
    assert_eq!(finding.severity, Severity::Critical);
    assert_eq!(finding.confidence, Confidence::Confirmed);
    assert_eq!(finding.remediation_safety, Some(RemediationSafety::HardwareService));
}

#[test]
fn recent_driver_change_plus_later_crash_correlates_but_does_not_claim_root_cause() {
    let facts = vec![
        fact(
            Domain::Drivers,
            "gpu",
            NOW - DAY_MS,
            FactPayload::DriverChange {
                installed_unix_ms: NOW - DAY_MS,
                previous_version: String::new(),
                version: "31.0.1".into(),
                authority_type: "WindowsUpdate".into(),
                authority_provider_id: "microsoft.windows-update".into(),
                provider: "Microsoft".into(),
                reboot_required: false,
                verified: true,
                rollback_available: false,
            },
        ),
        fact(
            Domain::Diagnostics,
            "crash-a",
            NOW - DAY_MS + 60 * 60 * 1000,
            FactPayload::Crash {
                crash_id: "crash-a".into(),
                bugcheck_hex: "0x00000116".into(),
            },
        ),
    ];
    let findings = evaluate_rules(&facts, NOW);
    let finding = findings
        .iter()
        .find(|f| f.code == "POSSIBLE_DRIVER_REGRESSION")
        .unwrap();
    assert_eq!(finding.severity, Severity::Moderate);
    assert_eq!(finding.confidence, Confidence::Medium);
    assert_eq!(finding.rule_id, "P17-CORR-001");
    assert_eq!(finding.rule_version, 2);
    let correlation = finding.correlation.as_ref().expect("correlation explanation");
    assert_eq!(correlation.strength, CorrelationStrength::Moderate);
    assert_eq!(correlation.time_distance_ms, 60 * 60 * 1000);
    assert!(finding.evidence.len() >= 2);
}

#[test]
fn crash_before_driver_change_does_not_correlate() {
    let facts = vec![
        fact(
            Domain::Diagnostics,
            "crash-a",
            NOW - 2 * DAY_MS,
            FactPayload::Crash {
                crash_id: "crash-a".into(),
                bugcheck_hex: "0x1".into(),
            },
        ),
        fact(
            Domain::Drivers,
            "gpu",
            NOW - DAY_MS,
            FactPayload::DriverChange {
                installed_unix_ms: NOW - DAY_MS,
                previous_version: String::new(),
                version: "31.0.1".into(),
                authority_type: "WindowsUpdate".into(),
                authority_provider_id: "microsoft.windows-update".into(),
                provider: "Microsoft".into(),
                reboot_required: false,
                verified: true,
                rollback_available: false,
            },
        ),
    ];
    assert!(!codes(&facts).contains(&"POSSIBLE_DRIVER_REGRESSION".into()));
}

#[test]
fn windows_corruption_creates_repair_recommendation_without_unrelated_update_inference() {
    let facts = vec![
        fact(
            Domain::Windows,
            "component-store",
            NOW,
            FactPayload::WindowsIntegrity {
                check_id: "dism".into(),
                result_code: "ComponentStoreCorruptionDetected".into(),
                exit_code: 2,
                detail: "synthetic".into(),
            },
        ),
        fact(
            Domain::Updates,
            "aethercore-app",
            NOW,
            FactPayload::UpdateState {
                state: "Failed".into(),
                current_version: "0.1.0".into(),
                available_release_id: String::new(),
                update_available: false,
                failed: true,
            },
        ),
    ];
    let findings = evaluate_rules(&facts, NOW);
    let integrity = findings
        .iter()
        .find(|f| f.code == "WINDOWS_INTEGRITY_ATTENTION")
        .unwrap();
    assert_eq!(integrity.confidence, Confidence::High);
    assert!(!findings.iter().any(|f| f.rule_id == "P17-CORR-002"));
}

#[test]
fn future_dated_crash_evidence_is_not_treated_as_recent() {
    let future = fact(
        Domain::Diagnostics,
        "future-crash",
        NOW + DAY_MS,
        FactPayload::Crash {
            crash_id: "future-crash".into(),
            bugcheck_hex: "0x1".into(),
        },
    );
    assert!(!codes(&[future]).contains(&"RECENT_CRASH_EVIDENCE".into()));
}

#[test]
fn slow_startup_threshold_is_bounded() {
    let below = fact(
        Domain::Startup,
        "startup",
        NOW,
        FactPayload::StartupFootprint {
            high_impact_count: 2,
            manageable_count: 4,
            total_count: 7,
        },
    );
    let at = fact(
        Domain::Startup,
        "startup",
        NOW,
        FactPayload::StartupFootprint {
            high_impact_count: 3,
            manageable_count: 5,
            total_count: 8,
        },
    );
    assert!(!codes(&[below]).contains(&"HIGH_STARTUP_FOOTPRINT".into()));
    assert!(codes(&[at]).contains(&"HIGH_STARTUP_FOOTPRINT".into()));
}

#[test]
fn cleanup_and_memory_boundaries_prevent_eager_warnings() {
    let mib = 1024_u64 * 1024;
    let cleanup_below = fact(
        Domain::Cleanup,
        "cleanup",
        NOW,
        FactPayload::CleanupOpportunity {
            candidate_id: "c1".into(),
            reclaimable_bytes: 512 * mib - 1,
            file_count: 1,
            requires_confirmation: false,
        },
    );
    let cleanup_at = fact(
        Domain::Cleanup,
        "cleanup",
        NOW,
        FactPayload::CleanupOpportunity {
            candidate_id: "c2".into(),
            reclaimable_bytes: 512 * mib,
            file_count: 1,
            requires_confirmation: false,
        },
    );
    let memory_94 = fact(
        Domain::Memory,
        "memory",
        NOW,
        FactPayload::MemoryPressure {
            memory_load_percent: 94,
            pressure_label: "Elevated".into(),
        },
    );
    let memory_95 = fact(
        Domain::Memory,
        "memory",
        NOW,
        FactPayload::MemoryPressure {
            memory_load_percent: 95,
            pressure_label: "High".into(),
        },
    );
    assert!(!codes(&[cleanup_below]).contains(&"CLEANUP_OPPORTUNITY".into()));
    assert!(codes(&[cleanup_at]).contains(&"CLEANUP_OPPORTUNITY".into()));
    assert!(!codes(&[memory_94]).contains(&"HIGH_MEMORY_PRESSURE".into()));
    assert!(codes(&[memory_95]).contains(&"HIGH_MEMORY_PRESSURE".into()));
}

#[test]
fn stale_crash_is_not_reported_as_recent() {
    let stale = fact(
        Domain::Diagnostics,
        "old-crash",
        NOW - 31 * DAY_MS,
        FactPayload::Crash {
            crash_id: "old-crash".into(),
            bugcheck_hex: "0x1".into(),
        },
    );
    assert!(!codes(&[stale]).contains(&"RECENT_CRASH_EVIDENCE".into()));
}

#[test]
fn diagnostic_limitation_is_not_a_machine_health_finding() {
    let limitation = fact(
        Domain::Diagnostics,
        "wmi",
        NOW,
        FactPayload::DiagnosticLimitation {
            collector: "wmi".into(),
            state: aethercore_pc_intelligence::CollectorState::Unavailable,
            detail: "synthetic unavailable".into(),
        },
    );
    assert!(evaluate_rules(&[limitation], NOW).is_empty());
}

#[test]
fn remediation_plan_is_deterministic_for_same_consent_snapshot() {
    let findings = evaluate_rules(
        &[fact(
            Domain::Cleanup,
            "cleanup",
            NOW,
            FactPayload::CleanupOpportunity {
                candidate_id: "c1".into(),
                reclaimable_bytes: 1024 * 1024 * 1024,
                file_count: 100,
                requires_confirmation: false,
            },
        )],
        NOW,
    );
    let actions = remediation_candidates(&findings);
    let first = RemediationPlan::seal("scan-a", NOW, actions.clone());
    let second = RemediationPlan::seal("scan-a", NOW, actions.into_iter().rev().collect());
    assert!(first.immutable && second.immutable);
    assert_eq!(first.digest, second.digest);
    assert_eq!(first.plan_id, second.plan_id);
}
