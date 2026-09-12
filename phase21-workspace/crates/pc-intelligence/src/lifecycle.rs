use std::collections::{BTreeMap, BTreeSet};

use aethercore_persistence::{Database, IntelligenceOverrideRecord};

use crate::{
    model::{
        CollectorState, Finding, FindingLifecycle, FindingVerificationStatus, ResolutionEvidence,
        SystemFact, private_id,
    },
    rules,
};

pub(crate) struct LifecycleOutcome {
    pub visible_findings: Vec<Finding>,
    pub persisted_findings: Vec<Finding>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResolutionPolicy {
    AuthoritativeAbsence,
    MatchingHealthyState,
}

pub(crate) fn default_resolution_authority(code: &str) -> Vec<String> {
    let scopes: &[&str] = match code {
        "DRIVER_MISSING"
        | "DEVICE_PROBLEM"
        | "DRIVER_UPDATE_AVAILABLE"
        | "NO_TRUSTED_CANDIDATE"
        | "VENDOR_UTILITY_REQUIRED"
        | "FIRMWARE_REVIEW_REQUIRED"
        | "DRIVER_MANAGEMENT_AUTHORITY_AVAILABLE"
        | "DRIVER_AUTHORITY_COVERAGE_INCOMPLETE"
        | "DRIVER_UPDATE_STATUS_UNKNOWN" => &["drivers"],
        "WINDOWS_INTEGRITY_ATTENTION" => &["windows"],
        "STORAGE_RELIABILITY_CONCERN"
        | "STORAGE_ATTENTION"
        | "HIGH_MEMORY_PRESSURE"
        | "HARDWARE_ERROR_EVIDENCE"
        | "RECENT_CRASH_EVIDENCE"
        | "CRASH_WITH_HARDWARE_EVIDENCE" => &["diagnostics"],
        "HIGH_STARTUP_FOOTPRINT" => &["startup"],
        "CLEANUP_OPPORTUNITY" => &["cleanup"],
        "APP_UPDATE_AVAILABLE" => &["updates"],
        "POSSIBLE_DRIVER_REGRESSION" => &["drivers", "diagnostics", "driver-history"],
        _ => &[],
    };
    scopes.iter().map(|scope| (*scope).into()).collect()
}

pub(crate) fn reconcile(
    db: &Database,
    owner: &str,
    scan_id: &str,
    now_ms: i64,
    facts: &[SystemFact],
    collectors: &[crate::model::CollectorStatus],
    supplemental_scopes: &BTreeMap<String, CollectorState>,
    mut current_findings: Vec<Finding>,
) -> LifecycleOutcome {
    let mut warnings = Vec::new();
    let previous = match db.intelligence_findings_for_owner(owner) {
        Ok(value) => value,
        Err(error) => {
            warnings.push(format!("persistence lifecycle read unavailable: {error}"));
            for finding in &mut current_findings {
                initialize_current_finding(finding, None, &[], now_ms);
            }
            return LifecycleOutcome {
                visible_findings: current_findings.clone(),
                persisted_findings: current_findings,
                warnings,
            };
        }
    };
    let overrides = match db.intelligence_overrides_for_owner(owner, now_ms) {
        Ok(value) => value,
        Err(error) => {
            warnings.push(format!("persistence override read unavailable: {error}"));
            Vec::new()
        }
    };

    let mut visible = Vec::new();
    let mut persisted = Vec::new();
    let current_ids = current_findings
        .iter()
        .map(|finding| finding.id.clone())
        .collect::<BTreeSet<_>>();

    for mut finding in current_findings.drain(..) {
        let old = previous
            .iter()
            .find(|record| record.finding_id == finding.id);
        initialize_current_finding(&mut finding, old, &overrides, now_ms);
        visible.push(finding.clone());
        persisted.push(finding);
    }

    for record in previous
        .iter()
        .filter(|record| !current_ids.contains(&record.finding_id))
    {
        let Some(mut old_finding) = serde_json::from_str::<Finding>(&record.finding_json).ok()
        else {
            warnings.push(format!(
                "persistence lifecycle evidence unavailable for finding {}",
                record.finding_id
            ));
            continue;
        };
        if record.lifecycle == "Resolved" {
            continue;
        }
        if old_finding.resolution_authority.is_empty() {
            old_finding.resolution_authority = default_resolution_authority(&old_finding.code);
        }
        old_finding.ignored =
            is_ignored(&overrides, &old_finding.id) || record.lifecycle == "Ignored";

        let authority = resolution_authority_state(
            &old_finding.resolution_authority,
            collectors,
            supplemental_scopes,
        );
        let matching_healthy_fact_ids = facts
            .iter()
            .filter(|fact| fact.resource.stable_id == old_finding.affected_resource.stable_id)
            .filter(|fact| rules::explicitly_healthy(fact))
            .map(|fact| fact.id.clone())
            .collect::<Vec<_>>();
        let policy = resolution_policy(&old_finding.code);
        let proof_sufficient = authority.all_completed
            && match policy {
                ResolutionPolicy::AuthoritativeAbsence => true,
                ResolutionPolicy::MatchingHealthyState => !matching_healthy_fact_ids.is_empty(),
            };

        if proof_sufficient {
            old_finding.lifecycle = FindingLifecycle::Resolved;
            old_finding.verification_status = FindingVerificationStatus::ResolutionConfirmed;
            old_finding.resolved_at_unix_ms = Some(now_ms);
            old_finding.resolution_scan_id = scan_id.into();
            old_finding.resolution_reason_key = match policy {
                ResolutionPolicy::AuthoritativeAbsence => {
                    "finding.resolution.authoritativeAbsence".into()
                }
                ResolutionPolicy::MatchingHealthyState => {
                    "finding.resolution.healthyStateConfirmed".into()
                }
            };
            old_finding.resolution_evidence = build_resolution_evidence(
                &old_finding.resolution_authority,
                collectors,
                supplemental_scopes,
                now_ms,
                &matching_healthy_fact_ids,
            );
            old_finding.remediation_available = false;
            old_finding.automatic_eligible = false;
            persisted.push(old_finding);
            continue;
        }

        old_finding.verification_status = if authority.has_unavailable_or_partial {
            FindingVerificationStatus::VerificationUnavailable
        } else {
            FindingVerificationStatus::NotRechecked
        };
        old_finding.resolved_at_unix_ms = None;
        old_finding.resolution_scan_id.clear();
        old_finding.resolution_reason_key.clear();
        old_finding.resolution_evidence.clear();
        old_finding.remediation_available = false;
        old_finding.automatic_eligible = false;
        if old_finding.ignored {
            old_finding.lifecycle = FindingLifecycle::Ignored;
        } else if matches!(
            old_finding.lifecycle,
            FindingLifecycle::New | FindingLifecycle::Improved
        ) {
            old_finding.lifecycle = FindingLifecycle::Active;
        }
        visible.push(old_finding.clone());
        persisted.push(old_finding);
    }

    visible.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| a.code.cmp(&b.code))
            .then_with(|| a.id.cmp(&b.id))
    });
    persisted.sort_by(|a, b| a.id.cmp(&b.id));
    LifecycleOutcome {
        visible_findings: visible,
        persisted_findings: persisted,
        warnings,
    }
}

fn initialize_current_finding(
    finding: &mut Finding,
    previous: Option<&aethercore_persistence::IntelligenceFindingRecord>,
    overrides: &[IntelligenceOverrideRecord],
    now_ms: i64,
) {
    finding.verification_status = FindingVerificationStatus::ConfirmedCurrent;
    if finding.resolution_authority.is_empty() {
        finding.resolution_authority = default_resolution_authority(&finding.code);
    }
    finding.resolved_at_unix_ms = None;
    finding.resolution_scan_id.clear();
    finding.resolution_reason_key.clear();
    finding.resolution_evidence.clear();
    finding.last_observed_unix_ms = now_ms;

    if let Some(old) = previous {
        finding.first_observed_unix_ms = old.first_observed_unix_ms;
        finding.lifecycle = if old.lifecycle == "Resolved" {
            FindingLifecycle::Recurred
        } else if old.lifecycle == "Ignored" {
            FindingLifecycle::Ignored
        } else if serde_json::from_str::<Finding>(&old.finding_json)
            .ok()
            .is_some_and(|previous| finding.severity < previous.severity)
        {
            FindingLifecycle::Improved
        } else {
            FindingLifecycle::Active
        };
    }

    if is_ignored(overrides, &finding.id) || finding.lifecycle == FindingLifecycle::Ignored {
        finding.ignored = true;
        finding.lifecycle = FindingLifecycle::Ignored;
        finding.remediation_available = false;
        finding.automatic_eligible = false;
    }
}

fn is_ignored(overrides: &[IntelligenceOverrideRecord], finding_id: &str) -> bool {
    overrides.iter().any(|entry| {
        entry.scope_kind == "finding"
            && entry.behavior == "Ignore"
            && entry.scope_value_hash == private_id(finding_id)
    })
}

fn resolution_policy(code: &str) -> ResolutionPolicy {
    match code {
        "WINDOWS_INTEGRITY_ATTENTION"
        | "STORAGE_RELIABILITY_CONCERN"
        | "STORAGE_ATTENTION"
        | "HIGH_MEMORY_PRESSURE"
        | "HIGH_STARTUP_FOOTPRINT"
        | "APP_UPDATE_AVAILABLE" => ResolutionPolicy::MatchingHealthyState,
        _ => ResolutionPolicy::AuthoritativeAbsence,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct AuthorityState {
    all_completed: bool,
    has_unavailable_or_partial: bool,
}

fn resolution_authority_state(
    scopes: &[String],
    collectors: &[crate::model::CollectorStatus],
    supplemental_scopes: &BTreeMap<String, CollectorState>,
) -> AuthorityState {
    if scopes.is_empty() {
        return AuthorityState {
            all_completed: false,
            has_unavailable_or_partial: false,
        };
    }
    let states = scopes
        .iter()
        .map(|scope| scope_state(scope, collectors, supplemental_scopes))
        .collect::<Vec<_>>();
    AuthorityState {
        all_completed: states
            .iter()
            .all(|state| matches!(state, Some(CollectorState::Completed))),
        has_unavailable_or_partial: states.iter().any(|state| {
            matches!(
                state,
                Some(
                    CollectorState::CompletedWithWarnings
                        | CollectorState::Unavailable
                        | CollectorState::PermissionDenied
                        | CollectorState::TimedOut
                        | CollectorState::Failed
                )
            )
        }),
    }
}

fn scope_state(
    scope: &str,
    collectors: &[crate::model::CollectorStatus],
    supplemental_scopes: &BTreeMap<String, CollectorState>,
) -> Option<CollectorState> {
    collectors
        .iter()
        .find(|collector| collector.id == scope)
        .map(|collector| collector.state)
        .or_else(|| supplemental_scopes.get(scope).copied())
}

fn build_resolution_evidence(
    scopes: &[String],
    collectors: &[crate::model::CollectorStatus],
    supplemental_scopes: &BTreeMap<String, CollectorState>,
    now_ms: i64,
    matching_healthy_fact_ids: &[String],
) -> Vec<ResolutionEvidence> {
    scopes
        .iter()
        .map(|scope| {
            let collector = collectors.iter().find(|collector| collector.id == *scope);
            ResolutionEvidence {
                scope: scope.clone(),
                collector_state: collector
                    .map(|collector| collector.state)
                    .or_else(|| supplemental_scopes.get(scope).copied())
                    .unwrap_or(CollectorState::Pending),
                observed_unix_ms: collector
                    .map(|collector| collector.completed_unix_ms)
                    .unwrap_or(now_ms),
                evidence_fact_ids: matching_healthy_fact_ids.to_vec(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        Confidence, Domain, EvidenceKind, FactPayload, Freshness, ResourceRef, Severity, SystemFact,
    };

    fn collector(id: &str, state: CollectorState) -> crate::model::CollectorStatus {
        crate::model::CollectorStatus {
            id: id.into(),
            state,
            stage_keys: Vec::new(),
            started_unix_ms: 1,
            completed_unix_ms: 2,
            detail: String::new(),
        }
    }

    fn healthy_driver_fact(resource: ResourceRef) -> SystemFact {
        SystemFact::new(
            Domain::Drivers,
            "test",
            resource,
            2,
            Freshness::Current,
            Confidence::Confirmed,
            FactPayload::DeviceHealth {
                missing_driver: false,
                has_problem: false,
                problem_code: 0,
                device_state: "Started".into(),
                update_status: "UpToDate".into(),
                authority_coverage: "CompleteForRequiredAuthorities".into(),
                management_authorities: vec![],
            },
            EvidenceKind::DeviceState,
            "healthy",
        )
    }

    fn test_db() -> (Database, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "aethercore-p17-1-lifecycle-{}.db",
            uuid::Uuid::new_v4()
        ));
        (Database::open(&path).expect("database"), path)
    }

    fn persist_finding(db: &Database, owner: &str, finding: &Finding, lifecycle: &str) {
        db.upsert_intelligence_finding(&aethercore_persistence::IntelligenceFindingRecord {
            owner_principal_key: owner.into(),
            finding_id: finding.id.clone(),
            finding_code: finding.code.clone(),
            first_observed_unix_ms: finding.first_observed_unix_ms,
            last_observed_unix_ms: finding.last_observed_unix_ms,
            lifecycle: lifecycle.into(),
            severity: format!("{:?}", finding.severity),
            confidence: format!("{:?}", finding.confidence),
            verification_status: format!("{:?}", finding.verification_status),
            resolved_at_unix_ms: None,
            resolution_scan_id: String::new(),
            resolution_reason_key: String::new(),
            finding_json: serde_json::to_string(finding).expect("serialize finding"),
        })
        .expect("persist finding");
    }

    fn cleanup(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(format!("{}-wal", path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    }

    fn old_missing_driver() -> Finding {
        crate::rules::evaluate(
            &[SystemFact::new(
                Domain::Drivers,
                "test",
                ResourceRef::private("device", "pci-a", "Device"),
                1,
                Freshness::Current,
                Confidence::Confirmed,
                FactPayload::DeviceHealth {
                    missing_driver: true,
                    has_problem: true,
                    problem_code: 28,
                    device_state: "Problem".into(),
                    update_status: "NoTrustedCandidate".into(),
                    authority_coverage: "CompleteForRequiredAuthorities".into(),
                    management_authorities: vec![],
                },
                EvidenceKind::DeviceState,
                "problem=28",
            )],
            1,
        )
        .remove(0)
    }

    fn storage_fact(
        read_errors: Option<u64>,
        write_errors: Option<u64>,
        nvme_warning: Option<u8>,
    ) -> SystemFact {
        SystemFact::new(
            Domain::Storage,
            "test",
            ResourceRef::private("storage-device", "disk0", "Disk"),
            2,
            Freshness::Current,
            Confidence::Confirmed,
            FactPayload::StorageHealth {
                health_status: "Healthy".into(),
                source_severity: "Healthy".into(),
                uncorrected_read_errors: read_errors,
                uncorrected_write_errors: write_errors,
                nvme_critical_warning: nvme_warning,
                nvme_media_errors_nonzero: false,
                wear_percent: Some(10),
                temperature_c: Some(40),
                temperature_max_c: Some(80),
            },
            EvidenceKind::StorageHealth,
            "state",
        )
    }

    /// DBT-P46-B3. `ResolutionPolicy::MatchingHealthyState` closes an open
    /// finding the moment a matching healthy fact appears, so this predicate is
    /// what stands between a real storage warning and it being auto-resolved as
    /// `healthyStateConfirmed`. A SMART counter that stopped answering must not
    /// be able to supply that proof.
    #[test]
    fn unreadable_smart_counters_do_not_prove_storage_health() {
        assert!(rules::explicitly_healthy(&storage_fact(
            Some(0),
            Some(0),
            Some(0)
        )));
        assert!(!rules::explicitly_healthy(&storage_fact(
            None,
            Some(0),
            Some(0)
        )));
        assert!(!rules::explicitly_healthy(&storage_fact(
            Some(0),
            None,
            Some(0)
        )));
        assert!(!rules::explicitly_healthy(&storage_fact(
            Some(0),
            Some(0),
            None
        )));
    }

    /// The same distinction in the other direction: an unread counter is not
    /// evidence of failure either, so it must not fabricate a Critical finding.
    #[test]
    fn unreadable_smart_counters_do_not_fabricate_a_storage_concern() {
        let findings = rules::evaluate(&[storage_fact(None, None, None)], 3);
        assert!(
            findings
                .iter()
                .all(|finding| finding.code != "STORAGE_RELIABILITY_CONCERN"),
            "unread counters fabricated {:?}",
            findings.iter().map(|f| &f.code).collect::<Vec<_>>()
        );
    }

    #[test]
    fn completed_owner_scope_can_authorize_resolution() {
        let finding = old_missing_driver();
        let state = resolution_authority_state(
            &["drivers".into()],
            &[collector("drivers", CollectorState::Completed)],
            &BTreeMap::new(),
        );
        assert!(state.all_completed);
        assert!(rules::explicitly_healthy(&healthy_driver_fact(
            finding.affected_resource
        )));
    }

    #[test]
    fn failed_timed_out_unavailable_and_partial_scopes_never_authorize_resolution() {
        for state in [
            CollectorState::Failed,
            CollectorState::TimedOut,
            CollectorState::Unavailable,
            CollectorState::PermissionDenied,
            CollectorState::CompletedWithWarnings,
        ] {
            let authority = resolution_authority_state(
                &["drivers".into()],
                &[collector("drivers", state)],
                &BTreeMap::new(),
            );
            assert!(!authority.all_completed);
            assert!(authority.has_unavailable_or_partial);
        }
    }

    #[test]
    fn cancelled_or_not_started_scope_is_not_rechecked_not_resolved() {
        for state in [CollectorState::Cancelled, CollectorState::Pending] {
            let authority = resolution_authority_state(
                &["drivers".into()],
                &[collector("drivers", state)],
                &BTreeMap::new(),
            );
            assert!(!authority.all_completed);
            assert!(!authority.has_unavailable_or_partial);
        }
    }

    #[test]
    fn later_unrelated_cancellation_does_not_revoke_completed_owner_authority() {
        let authority = resolution_authority_state(
            &["drivers".into()],
            &[
                collector("drivers", CollectorState::Completed),
                collector("cleanup", CollectorState::Cancelled),
            ],
            &BTreeMap::new(),
        );
        assert!(authority.all_completed);
    }

    #[test]
    fn authoritative_completed_scope_resolves_but_unrelated_cancel_does_not_block_it() {
        let (db, path) = test_db();
        let owner = "owner";
        let old = old_missing_driver();
        let healthy = healthy_driver_fact(old.affected_resource.clone());
        persist_finding(&db, owner, &old, "Active");
        let result = reconcile(
            &db,
            owner,
            "scan-2",
            20,
            &[healthy],
            &[
                collector("drivers", CollectorState::Completed),
                collector("cleanup", CollectorState::Cancelled),
            ],
            &BTreeMap::new(),
            Vec::new(),
        );
        assert!(result.visible_findings.is_empty());
        let resolved = &result.persisted_findings[0];
        assert_eq!(resolved.lifecycle, FindingLifecycle::Resolved);
        assert_eq!(
            resolved.verification_status,
            FindingVerificationStatus::ResolutionConfirmed
        );
        assert_eq!(resolved.resolution_scan_id, "scan-2");
        assert!(!resolved.resolution_evidence.is_empty());
        drop(db);
        cleanup(&path);
    }

    #[test]
    fn failed_or_partial_owner_scope_carries_finding_without_remediation() {
        for state in [
            CollectorState::Failed,
            CollectorState::TimedOut,
            CollectorState::Unavailable,
            CollectorState::PermissionDenied,
            CollectorState::CompletedWithWarnings,
        ] {
            let (db, path) = test_db();
            let owner = "owner";
            let old = old_missing_driver();
            persist_finding(&db, owner, &old, "Active");
            let result = reconcile(
                &db,
                owner,
                "scan-2",
                20,
                &[],
                &[collector("drivers", state)],
                &BTreeMap::new(),
                Vec::new(),
            );
            assert_eq!(result.visible_findings.len(), 1);
            let carried = &result.visible_findings[0];
            assert_ne!(carried.lifecycle, FindingLifecycle::Resolved);
            assert_eq!(
                carried.verification_status,
                FindingVerificationStatus::VerificationUnavailable
            );
            assert!(!carried.remediation_available);
            drop(db);
            cleanup(&path);
        }
    }

    #[test]
    fn cancellation_before_owner_scope_keeps_finding_not_rechecked() {
        let (db, path) = test_db();
        let owner = "owner";
        let old = old_missing_driver();
        persist_finding(&db, owner, &old, "Active");
        let result = reconcile(
            &db,
            owner,
            "scan-2",
            20,
            &[],
            &[collector("drivers", CollectorState::Cancelled)],
            &BTreeMap::new(),
            Vec::new(),
        );
        let carried = &result.visible_findings[0];
        assert_ne!(carried.lifecycle, FindingLifecycle::Resolved);
        assert_eq!(
            carried.verification_status,
            FindingVerificationStatus::NotRechecked
        );
        drop(db);
        cleanup(&path);
    }

    #[test]
    fn resolved_issue_returning_is_recurred_and_ignored_state_is_durable() {
        let (db, path) = test_db();
        let owner = "owner";
        let old = old_missing_driver();
        persist_finding(&db, owner, &old, "Resolved");
        let current = old_missing_driver();
        let result = reconcile(
            &db,
            owner,
            "scan-3",
            30,
            &[],
            &[collector("drivers", CollectorState::Completed)],
            &BTreeMap::new(),
            vec![current],
        );
        assert_eq!(
            result.visible_findings[0].lifecycle,
            FindingLifecycle::Recurred
        );

        let ignored = result.visible_findings[0].clone();
        persist_finding(&db, owner, &ignored, "Ignored");
        let current = old_missing_driver();
        let result = reconcile(
            &db,
            owner,
            "scan-4",
            40,
            &[],
            &[collector("drivers", CollectorState::Completed)],
            &BTreeMap::new(),
            vec![current],
        );
        assert_eq!(
            result.visible_findings[0].lifecycle,
            FindingLifecycle::Ignored
        );
        assert!(result.visible_findings[0].ignored);
        assert!(!result.visible_findings[0].remediation_available);
        drop(db);
        cleanup(&path);
    }

    #[test]
    fn recurrence_name_replaces_phase17_returned_semantics() {
        let mut finding = old_missing_driver();
        finding.lifecycle = FindingLifecycle::Recurred;
        assert_eq!(finding.lifecycle, FindingLifecycle::Recurred);
        assert_eq!(finding.severity, Severity::High);
    }
}
