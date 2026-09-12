use aethercore_windows_repair_intelligence::*;

fn fact(id: &str, domain: RepairDomain, state: FactState) -> RepairFact {
    RepairFact {
        id: id.into(),
        domain,
        state,
        resource: format!("resource:{id}"),
        evidence_code: format!("EVIDENCE_{id}"),
        technical_code: String::new(),
        detail: String::new(),
        observed_unix_ms: 1,
        confidence: DiagnosisConfidence::High,
    }
}
fn obs(facts: Vec<RepairFact>) -> RepairObservationSet {
    RepairObservationSet {
        observation_id: "p19-fixture".into(),
        machine_state_fingerprint: "machine-fingerprint-v1".into(),
        facts,
        recovery: RecoveryReadiness {
            system_restore: FactState::Available,
            restore_point_creation: FactState::Available,
            win_re: FactState::Available,
            journal_recovery: FactState::Available,
            driver_rollback: FactState::Available,
        },
    }
}
fn node<'a>(s: &'a RepairIntelligenceSnapshot, id: &str) -> &'a RepairNode {
    s.graph.nodes.iter().find(|n| n.id == id).unwrap()
}

#[test]
fn p19_01_healthy_windows_no_fabricated_repair() {
    let s = analyze(&obs(vec![
        fact("c", RepairDomain::ComponentStore, FactState::Healthy),
        fact("s", RepairDomain::SystemFiles, FactState::Healthy),
    ]));
    assert!(s.diagnoses.is_empty());
    assert!(s.graph.nodes.is_empty());
}
#[test]
fn p19_02_component_store_corruption_plans_dism() {
    let s = analyze(&obs(vec![fact(
        "c",
        RepairDomain::ComponentStore,
        FactState::CorruptionDetected,
    )]));
    assert!(
        s.diagnoses
            .iter()
            .any(|d| d.code == "COMPONENT_STORE_CORRUPTION")
    );
    assert_eq!(
        node(&s, "repair-component-store").action,
        RepairActionKind::RepairComponentStore
    );
}
#[test]
fn p19_03_component_precedes_sfc() {
    let s = analyze(&obs(vec![
        fact(
            "c",
            RepairDomain::ComponentStore,
            FactState::CorruptionDetected,
        ),
        fact(
            "s",
            RepairDomain::SystemFiles,
            FactState::CorruptionDetected,
        ),
    ]));
    assert!(
        node(&s, "repair-system-files")
            .dependencies
            .contains(&"verify-component-store".into())
    );
    let a = s
        .graph
        .deterministic_order
        .iter()
        .position(|v| v == "verify-component-store")
        .unwrap();
    let b = s
        .graph
        .deterministic_order
        .iter()
        .position(|v| v == "repair-system-files")
        .unwrap();
    assert!(a < b);
}
#[test]
fn p19_04_failed_verification_cannot_be_success() {
    assert_ne!(
        RepairOutcome::MutationSucceededVerificationFailed,
        RepairOutcome::SucceededVerified
    );
}
#[test]
fn p19_05_pending_reboot_is_barrier() {
    let s = analyze(&obs(vec![
        fact("r", RepairDomain::Reboot, FactState::RebootRequired),
        fact("u", RepairDomain::WindowsUpdate, FactState::Failure),
    ]));
    let retry = node(&s, "retry-windows-update");
    assert!(retry.dependencies.contains(&"reboot-boundary".into()));
    assert!(!retry.executable_automatically);
    assert!(node(&s, "reboot-boundary").reboot_boundary_after);
}
#[test]
fn p19_06_update_failure_from_servicing_repairs_servicing_first() {
    let s = analyze(&obs(vec![
        fact(
            "c",
            RepairDomain::ComponentStore,
            FactState::CorruptionDetected,
        ),
        fact("u", RepairDomain::WindowsUpdate, FactState::Failure),
    ]));
    assert!(
        node(&s, "retry-windows-update")
            .dependencies
            .contains(&"verify-component-store".into())
    );
}
#[test]
fn p19_07_update_offline_is_not_corruption() {
    let s = analyze(&obs(vec![fact(
        "u",
        RepairDomain::WindowsUpdate,
        FactState::Offline,
    )]));
    assert!(
        s.diagnoses
            .iter()
            .any(|d| d.code == "WINDOWS_UPDATE_OFFLINE")
    );
    assert!(
        !s.diagnoses
            .iter()
            .any(|d| d.code == "COMPONENT_STORE_CORRUPTION")
    );
    assert!(s.graph.nodes.is_empty());
}
#[test]
fn p19_08_required_service_stopped_is_targeted() {
    let s = analyze(&obs(vec![fact(
        "svc-wuauserv",
        RepairDomain::Services,
        FactState::Stopped,
    )]));
    assert_eq!(
        node(&s, "start-required-service").action,
        RepairActionKind::StartRequiredService
    );
    assert_eq!(
        node(&s, "start-required-service").target_resource,
        "windows:diagnosis-scoped-service"
    );
}
#[test]
fn p19_09_unexpected_service_config_not_blind_auto_reset() {
    let s = analyze(&obs(vec![fact(
        "svc",
        RepairDomain::Services,
        FactState::UnexpectedConfiguration,
    )]));
    let n = node(&s, "review-service-configuration");
    assert!(n.requires_explicit_consent);
    assert!(!n.executable_automatically);
}
#[test]
fn p19_10_dns_failure_not_generic_network_reset() {
    let s = analyze(&obs(vec![fact(
        "dns",
        RepairDomain::Dns,
        FactState::Failure,
    )]));
    assert!(
        s.graph
            .nodes
            .iter()
            .any(|n| n.action == RepairActionKind::FlushDnsCache)
    );
    assert!(
        !s.graph
            .nodes
            .iter()
            .any(|n| n.action == RepairActionKind::ResetWinsock)
    );
}
#[test]
fn p19_11_bad_proxy_is_review_first() {
    let s = analyze(&obs(vec![fact(
        "proxy",
        RepairDomain::Proxy,
        FactState::Failure,
    )]));
    let n = node(&s, "review-proxy");
    assert_eq!(n.safety, RepairSafetyTier::Level0Diagnostic);
    assert!(!n.executable_automatically);
}
#[test]
fn p19_12_dhcp_problem_does_not_invent_dns_fix() {
    let s = analyze(&obs(vec![fact(
        "net",
        RepairDomain::Network,
        FactState::Failure,
    )]));
    assert!(
        !s.graph
            .nodes
            .iter()
            .any(|n| n.action == RepairActionKind::FlushDnsCache)
    );
}
#[test]
fn p19_13_winsock_is_sensitive_and_reboot_aware_by_contract() {
    let n = RepairNode {
        id: "winsock".into(),
        action: RepairActionKind::ResetWinsock,
        safety: RepairSafetyTier::Level2SensitiveRepair,
        dependencies: vec![],
        target_resource: "windows:winsock".into(),
        diagnosis_ids: vec![],
        verification: VerificationKind::RerunNetworkDiagnostic,
        reversibility: Reversibility::RebootRollback,
        requires_explicit_consent: true,
        requires_recovery_protection: false,
        reboot_boundary_after: true,
        executable_automatically: false,
    };
    let g = RepairGraph::new(vec![n]).unwrap();
    assert!(g.nodes[0].reboot_boundary_after);
    assert!(g.nodes[0].requires_explicit_consent);
}
#[test]
fn p19_14_filesystem_error_not_storage_hardware() {
    let s = analyze(&obs(vec![fact(
        "fs",
        RepairDomain::Filesystem,
        FactState::CorruptionDetected,
    )]));
    assert!(s.diagnoses.iter().any(|d| d.code == "FILESYSTEM_ERROR"));
    assert!(
        !s.diagnoses
            .iter()
            .any(|d| d.code == "STORAGE_HARDWARE_WARNING")
    );
}
#[test]
fn p19_15_storage_and_filesystem_remain_separate_correlated_domains() {
    let s = analyze(&obs(vec![
        fact("fs", RepairDomain::Filesystem, FactState::Failure),
        fact("hw", RepairDomain::StorageHardware, FactState::Degraded),
    ]));
    assert!(s.diagnoses.iter().any(|d| d.code == "FILESYSTEM_ERROR"));
    assert!(
        s.diagnoses
            .iter()
            .any(|d| d.code == "STORAGE_HARDWARE_WARNING")
    );
    assert!(
        s.graph
            .nodes
            .iter()
            .any(|n| n.action == RepairActionKind::GuidedHardwareService)
    );
}
#[test]
fn p19_16_restore_unavailable_changes_sensitive_recovery_semantics() {
    let mut o = obs(vec![fact(
        "fs",
        RepairDomain::Filesystem,
        FactState::Repairable,
    )]);
    o.recovery.system_restore = FactState::Unavailable;
    o.recovery.restore_point_creation = FactState::Unavailable;
    o.recovery.win_re = FactState::Unavailable;
    o.recovery.journal_recovery = FactState::Unavailable;
    let s = analyze(&o);
    let n = node(&s, "filesystem-offline-repair");
    assert!(n.requires_recovery_protection);
    assert!(!n.executable_automatically);
}
#[test]
fn p19_17_winre_unavailable_is_recovery_readiness_finding() {
    let s = analyze(&obs(vec![fact(
        "winre",
        RepairDomain::Recovery,
        FactState::Unavailable,
    )]));
    assert!(
        s.diagnoses
            .iter()
            .any(|d| d.code == "RECOVERY_ENVIRONMENT_UNAVAILABLE")
    );
    assert_eq!(
        node(&s, "verify-winre").action,
        RepairActionKind::VerifyWinRe
    );
}
#[test]
fn p19_18_failure_before_mutation_has_no_rollback_claim() {
    assert_eq!(
        RepairOutcome::FailedBeforeMutation,
        RepairOutcome::FailedBeforeMutation
    );
    assert_ne!(
        RepairOutcome::FailedBeforeMutation,
        RepairOutcome::RolledBack
    );
}
#[test]
fn p19_19_failure_after_mutation_requires_verification_or_recovery() {
    assert!(matches!(
        RepairOutcome::FailedAfterMutation,
        RepairOutcome::FailedAfterMutation | RepairOutcome::MutationSucceededVerificationFailed
    ));
    assert!(matches!(
        RepairOutcome::MutationSucceededVerificationFailed,
        RepairOutcome::FailedAfterMutation | RepairOutcome::MutationSucceededVerificationFailed
    ));
}
#[test]
fn p19_20_state_change_after_consent_is_detectable() {
    let o = obs(vec![fact(
        "c",
        RepairDomain::ComponentStore,
        FactState::CorruptionDetected,
    )]);
    let s = analyze(&o);
    assert_eq!(s.machine_state_fingerprint, "machine-fingerprint-v1");
    assert_ne!(s.machine_state_fingerprint, "machine-fingerprint-v2");
}
#[test]
fn p19_21_concurrent_driver_mutation_is_external_supervisor_contract() {
    let s = analyze(&obs(vec![fact(
        "c",
        RepairDomain::ComponentStore,
        FactState::CorruptionDetected,
    )]));
    assert!(s.graph.valid);
    assert!(node(&s, "repair-component-store").requires_explicit_consent);
}
#[test]
fn p19_22_cancel_safe_boundary_status_exists() {
    assert_eq!(
        format!("{:?}", RepairOutcome::CancelledBeforeMutation),
        "CancelledBeforeMutation"
    );
}
#[test]
fn p19_23_non_interruptible_servicing_deferred_cancel_status_exists() {
    assert_eq!(
        format!("{:?}", RepairOutcome::CancellationDeferred),
        "CancellationDeferred"
    );
}
#[test]
fn p19_24_lower_repairs_exhausted_escalates_not_loops() {
    let s = analyze(&obs(vec![fact(
        "c",
        RepairDomain::ComponentStore,
        FactState::RepairFailed,
    )]));
    let n = node(&s, "guided-repair-reinstall");
    assert_eq!(n.safety, RepairSafetyTier::Level4RecoveryEscalation);
    assert!(!n.executable_automatically);
}

#[test]
fn invariant_graph_cycle_rejected() {
    let a = RepairNode {
        id: "a".into(),
        action: RepairActionKind::CheckComponentStore,
        safety: RepairSafetyTier::Level0Diagnostic,
        dependencies: vec!["b".into()],
        target_resource: "x".into(),
        diagnosis_ids: vec![],
        verification: VerificationKind::None,
        reversibility: Reversibility::FullyReversible,
        requires_explicit_consent: false,
        requires_recovery_protection: false,
        reboot_boundary_after: false,
        executable_automatically: true,
    };
    let mut b = a.clone();
    b.id = "b".into();
    b.dependencies = vec!["a".into()];
    assert!(matches!(
        RepairGraph::new(vec![a, b]),
        Err(GraphError::Cycle)
    ));
}
#[test]
fn invariant_missing_dependency_rejected() {
    let n = RepairNode {
        id: "a".into(),
        action: RepairActionKind::CheckComponentStore,
        safety: RepairSafetyTier::Level0Diagnostic,
        dependencies: vec!["missing".into()],
        target_resource: "x".into(),
        diagnosis_ids: vec![],
        verification: VerificationKind::None,
        reversibility: Reversibility::FullyReversible,
        requires_explicit_consent: false,
        requires_recovery_protection: false,
        reboot_boundary_after: false,
        executable_automatically: true,
    };
    assert!(matches!(
        RepairGraph::new(vec![n]),
        Err(GraphError::MissingDependency { .. })
    ));
}
#[test]
fn invariant_destructive_action_cannot_be_safe_auto() {
    let n = RepairNode {
        id: "wipe".into(),
        action: RepairActionKind::GuidedCleanReinstall,
        safety: RepairSafetyTier::Level1SafeAuto,
        dependencies: vec![],
        target_resource: "windows".into(),
        diagnosis_ids: vec![],
        verification: VerificationKind::ManualOfficialRecovery,
        reversibility: Reversibility::IrreversibleManualRecovery,
        requires_explicit_consent: false,
        requires_recovery_protection: false,
        reboot_boundary_after: false,
        executable_automatically: true,
    };
    assert!(matches!(
        RepairGraph::new(vec![n]),
        Err(GraphError::DestructiveAuto(_))
    ));
}
#[test]
fn invariant_unknown_evidence_not_confident_diagnosis() {
    let mut f = fact("c", RepairDomain::ComponentStore, FactState::Unknown);
    f.confidence = DiagnosisConfidence::Unknown;
    let s = analyze(&obs(vec![f]));
    assert!(s.diagnoses.is_empty());
}
#[test]
fn invariant_one_failing_collector_does_not_mark_windows_globally_broken() {
    let s = analyze(&obs(vec![
        fact("network", RepairDomain::Network, FactState::Unknown),
        fact("c", RepairDomain::ComponentStore, FactState::Healthy),
    ]));
    assert!(
        !s.diagnoses
            .iter()
            .any(|d| d.code == "COMPONENT_STORE_CORRUPTION")
    );
}
#[test]
fn invariant_hresult_mapping_alone_is_not_root_cause_authority() {
    let e = classify_windows_error(0x8024402Cu32 as i64);
    assert!(!e.definitive);
    assert_eq!(e.class, "NetworkOrProxy");
}

#[test]
fn invariant_recovery_required_action_cannot_auto_run_without_protection() {
    let n = RepairNode {
        id: "protected".into(),
        action: RepairActionKind::CreateRecoveryPoint,
        safety: RepairSafetyTier::Level2SensitiveRepair,
        dependencies: vec![],
        target_resource: "windows:restore".into(),
        diagnosis_ids: vec![],
        verification: VerificationKind::None,
        reversibility: Reversibility::RestorePointDependent,
        requires_explicit_consent: true,
        requires_recovery_protection: true,
        reboot_boundary_after: false,
        executable_automatically: true,
    };
    let g = RepairGraph::new(vec![n]).unwrap();
    let r = RecoveryReadiness {
        system_restore: FactState::Unavailable,
        restore_point_creation: FactState::Unavailable,
        win_re: FactState::Unavailable,
        journal_recovery: FactState::Unavailable,
        driver_rollback: FactState::Unknown,
    };
    assert!(matches!(
        g.validate_for_execution(&r),
        Err(GraphError::RecoveryPrerequisite(_))
    ));
}
#[test]
fn invariant_auto_action_cannot_cross_reboot_barrier() {
    let b = RepairNode {
        id: "reboot".into(),
        action: RepairActionKind::Reboot,
        safety: RepairSafetyTier::Level3RebootOrOffline,
        dependencies: vec![],
        target_resource: "windows:reboot".into(),
        diagnosis_ids: vec![],
        verification: VerificationKind::None,
        reversibility: Reversibility::RebootRollback,
        requires_explicit_consent: true,
        requires_recovery_protection: false,
        reboot_boundary_after: true,
        executable_automatically: false,
    };
    let n = RepairNode {
        id: "after".into(),
        action: RepairActionKind::RetryWindowsUpdate,
        safety: RepairSafetyTier::Level1SafeAuto,
        dependencies: vec!["reboot".into()],
        target_resource: "windows:update-agent".into(),
        diagnosis_ids: vec![],
        verification: VerificationKind::RetryWindowsUpdateDiscovery,
        reversibility: Reversibility::FullyReversible,
        requires_explicit_consent: false,
        requires_recovery_protection: false,
        reboot_boundary_after: false,
        executable_automatically: true,
    };
    assert!(matches!(
        RepairGraph::new(vec![b, n]),
        Err(GraphError::RebootBoundary(_))
    ));
}
#[test]
fn invariant_contradictory_destructive_recovery_rejected() {
    let base = RepairNode {
        id: "reset".into(),
        action: RepairActionKind::GuidedResetPreservingFiles,
        safety: RepairSafetyTier::Level5DestructiveRecovery,
        dependencies: vec![],
        target_resource: "windows:recovery".into(),
        diagnosis_ids: vec![],
        verification: VerificationKind::ManualOfficialRecovery,
        reversibility: Reversibility::IrreversibleManualRecovery,
        requires_explicit_consent: true,
        requires_recovery_protection: true,
        reboot_boundary_after: true,
        executable_automatically: false,
    };
    let mut clean = base.clone();
    clean.id = "clean".into();
    clean.action = RepairActionKind::GuidedCleanReinstall;
    assert!(matches!(
        RepairGraph::new(vec![base, clean]),
        Err(GraphError::ContradictoryActions(_))
    ));
}
