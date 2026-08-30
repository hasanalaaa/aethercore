use serde::Serialize;

use crate::model::{hex_sha256, FactPayload, SystemFact};

#[derive(Clone, Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
struct CanonicalStateEntry {
    domain: String,
    source: String,
    resource_kind: String,
    resource_stable_id: String,
    payload_kind: String,
    state: String,
}

/// Produces a deterministic privacy-safe fingerprint of semantically relevant
/// machine state. Observation timestamps, display names, evidence text, scan
/// identifiers and collector timing are intentionally excluded.
pub fn machine_state_fingerprint(facts: &[SystemFact]) -> String {
    let mut entries = facts
        .iter()
        .filter_map(canonical_entry)
        .collect::<Vec<_>>();
    entries.sort();
    let bytes = serde_json::to_vec(&entries).unwrap_or_default();
    hex_sha256(&bytes)
}

fn canonical_entry(fact: &SystemFact) -> Option<CanonicalStateEntry> {
    let state = canonical_payload_state(&fact.payload)?;
    let resource_stable_id = match &fact.payload {
        // Event resource IDs intentionally contain collection/event timestamps in
        // Phase 17. Canonical machine state uses type/count + semantic payload,
        // never those volatile IDs.
        FactPayload::HardwareEvent { .. } => "hardware-event".into(),
        FactPayload::Crash { .. } => "crash-event".into(),
        _ => fact.resource.stable_id.clone(),
    };
    Some(CanonicalStateEntry {
        domain: fact.domain.as_str().into(),
        source: fact.source.clone(),
        resource_kind: fact.resource.kind.clone(),
        // Private resource identities are privacy-preserving hashes produced by
        // ResourceRef::private. Event IDs with embedded timestamps are excluded above.
        resource_stable_id,
        payload_kind: fact.payload.kind_name().into(),
        state,
    })
}

fn canonical_payload_state(payload: &FactPayload) -> Option<String> {
    let value = match payload {
        FactPayload::InventorySummary { device_count } => serde_json::to_string(&(device_count,)),
        FactPayload::HardwareDevice {
            class_name,
            manufacturer,
            installed_driver_version,
            gpu_vendor,
            ..
        } => serde_json::to_string(&(
            class_name,
            manufacturer,
            installed_driver_version,
            gpu_vendor,
        )),
        FactPayload::DeviceHealth {
            missing_driver,
            has_problem,
            problem_code,
            device_state,
            update_status,
            authority_coverage,
            management_authorities,
        } => serde_json::to_string(&(
            missing_driver,
            has_problem,
            problem_code,
            device_state,
            update_status,
            authority_coverage,
            management_authorities,
        )),
        FactPayload::DriverUpdate {
            candidate_id,
            current_version,
            target_version,
            vendor_managed,
            selectable,
            authority_type,
            authority_name,
            recommendation_state,
            installation_mode,
            trust_state,
            authority_coverage,
        } => serde_json::to_string(&(
            candidate_id,
            current_version,
            target_version,
            vendor_managed,
            selectable,
            authority_type,
            authority_name,
            recommendation_state,
            installation_mode,
            trust_state,
            authority_coverage,
        )),
        // DriverChange is historical correlation evidence rather than current
        // machine state. The installed driver version itself is represented by
        // HardwareDevice/DriverUpdate facts, so event time does not destabilize
        // the state fingerprint.
        FactPayload::DriverChange { .. } => return None,
        FactPayload::WindowsIntegrity {
            check_id,
            result_code,
            exit_code,
            ..
        } => serde_json::to_string(&(check_id, result_code, exit_code)),
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
            let thermal_attention = matches!(
                (temperature_c, temperature_max_c),
                (Some(current), Some(maximum)) if *maximum > 0 && *current >= *maximum
            );
            serde_json::to_string(&(
                health_status,
                source_severity,
                uncorrected_read_errors,
                uncorrected_write_errors,
                nvme_critical_warning,
                nvme_media_errors_nonzero,
                wear_percent,
                thermal_attention,
            ))
        },
        FactPayload::MemoryPressure {
            memory_load_percent,
            pressure_label,
        } => {
            let bucket = match memory_load_percent {
                0..=79 => "normal",
                80..=94 => "elevated",
                _ => "high",
            };
            serde_json::to_string(&(bucket, pressure_label))
        }
        FactPayload::HardwareEvent {
            category,
            provider,
            event_id,
        } => serde_json::to_string(&(category, provider, event_id)),
        FactPayload::Crash { bugcheck_hex, .. } => serde_json::to_string(&(bugcheck_hex,)),
        FactPayload::StartupFootprint {
            high_impact_count,
            manageable_count,
            total_count,
        } => serde_json::to_string(&(high_impact_count, manageable_count, total_count)),
        // Reclaimable cache/temp content is intentionally not machine-health state.
        // Candidate IDs are ephemeral and would destabilize timeline fingerprints.
        FactPayload::CleanupOpportunity { .. } => return None,
        FactPayload::UpdateState {
            state,
            current_version,
            available_release_id,
            update_available,
            failed,
        } => serde_json::to_string(&(state, current_version, available_release_id, update_available, failed)),
        FactPayload::RecoveryReadiness {
            active_recovery_records,
        } => serde_json::to_string(&(active_recovery_records,)),
        FactPayload::DiagnosticLimitation { .. } => return None,
    };
    value.ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        Confidence, Domain, EvidenceKind, FactPayload, Freshness, ResourceRef, SystemFact,
    };

    fn fact(at: i64, payload: FactPayload) -> SystemFact {
        SystemFact::new(
            Domain::Drivers,
            "test",
            ResourceRef::private("device", "PCI\\VEN_1234", "User-visible name"),
            at,
            Freshness::Current,
            Confidence::Confirmed,
            payload,
            EvidenceKind::DeviceState,
            format!("collected-at={at}"),
        )
    }

    #[test]
    fn same_state_ignores_timestamp_evidence_text_and_order() {
        let mut a = fact(
            100,
            FactPayload::DeviceHealth {
                missing_driver: false,
                has_problem: false,
                problem_code: 0,
                device_state: "Started".into(),
                update_status: "UpToDate".into(),
                authority_coverage: "CompleteForRequiredAuthorities".into(),
            management_authorities: vec![],
            },
        );
        let mut b = a.clone();
        b.observed_unix_ms = 9_999;
        b.evidence[0].observed_unix_ms = 9_999;
        b.evidence[0].technical_value = "different collection metadata".into();
        b.resource.display_name = "Different display name".into();
        assert_eq!(machine_state_fingerprint(&[a.clone(), b.clone()]), machine_state_fingerprint(&[b, a]));
    }

    #[test]
    fn driver_version_change_changes_fingerprint() {
        let a = fact(
            100,
            FactPayload::HardwareDevice {
                class_name: "Display".into(),
                manufacturer: "Vendor".into(),
                description: "Adapter".into(),
                installed_driver_version: "1.0".into(),
                gpu_vendor: "Vendor".into(),
            },
        );
        let mut b = a.clone();
        if let FactPayload::HardwareDevice { installed_driver_version, .. } = &mut b.payload {
            *installed_driver_version = "2.0".into();
        }
        assert_ne!(machine_state_fingerprint(&[a]), machine_state_fingerprint(&[b]));
    }

    #[test]
    fn storage_degradation_changes_fingerprint() {
        let make = |errors| {
            SystemFact::new(
                Domain::Storage,
                "test",
                ResourceRef::private("storage", "disk0", "Disk"),
                100,
                Freshness::Current,
                Confidence::Confirmed,
                FactPayload::StorageHealth {
                    health_status: "Healthy".into(),
                    source_severity: "Healthy".into(),
                    uncorrected_read_errors: errors,
                    uncorrected_write_errors: 0,
                    nvme_critical_warning: 0,
                    nvme_media_errors_nonzero: false,
                    wear_percent: Some(10),
                    temperature_c: Some(40),
                    temperature_max_c: Some(80),
                },
                EvidenceKind::StorageHealth,
                "state",
            )
        };
        assert_ne!(machine_state_fingerprint(&[make(0)]), machine_state_fingerprint(&[make(27)]));
    }

    #[test]
    fn event_resource_timestamp_identity_does_not_change_fingerprint() {
        let make = |id: &str, at: i64| {
            SystemFact::new(
                Domain::Hardware,
                "crash-diagnostics",
                ResourceRef::global("hardware-event", id, "Hardware error evidence"),
                at,
                Freshness::Recent,
                Confidence::High,
                FactPayload::HardwareEvent {
                    category: "ProcessorHardwareEvidence".into(),
                    provider: "Microsoft-Windows-WHEA-Logger".into(),
                    event_id: 17,
                },
                EvidenceKind::HardwareEvent,
                "event",
            )
        };
        assert_eq!(
            machine_state_fingerprint(&[make("whea-17-100", 100)]),
            machine_state_fingerprint(&[make("whea-17-9999", 9_999)])
        );
    }

    #[test]
    fn normal_temperature_fluctuation_and_cleanup_churn_do_not_change_machine_state() {
        let storage = |temperature| {
            SystemFact::new(
                Domain::Storage,
                "test",
                ResourceRef::private("storage", "disk0", "Disk"),
                100,
                Freshness::Current,
                Confidence::Confirmed,
                FactPayload::StorageHealth {
                    health_status: "Healthy".into(),
                    source_severity: "Healthy".into(),
                    uncorrected_read_errors: 0,
                    uncorrected_write_errors: 0,
                    nvme_critical_warning: 0,
                    nvme_media_errors_nonzero: false,
                    wear_percent: Some(10),
                    temperature_c: Some(temperature),
                    temperature_max_c: Some(80),
                },
                EvidenceKind::StorageHealth,
                "state",
            )
        };
        let cleanup = SystemFact::new(
            Domain::Cleanup,
            "cleaner",
            ResourceRef::global("cleanup-candidate", "random-uuid", "Cache"),
            100,
            Freshness::Current,
            Confidence::Confirmed,
            FactPayload::CleanupOpportunity {
                candidate_id: "random-uuid".into(),
                reclaimable_bytes: 800_000_000,
                file_count: 100,
                requires_confirmation: false,
            },
            EvidenceKind::CleanupEstimate,
            "cache",
        );
        assert_eq!(
            machine_state_fingerprint(&[storage(40), cleanup.clone()]),
            machine_state_fingerprint(&[storage(45), cleanup])
        );
    }

    #[test]
    fn diagnostic_limitations_do_not_change_machine_state_fingerprint() {
        let state = fact(
            100,
            FactPayload::DeviceHealth {
                missing_driver: false,
                has_problem: false,
                problem_code: 0,
                device_state: "Started".into(),
                update_status: "UpToDate".into(),
                authority_coverage: "CompleteForRequiredAuthorities".into(),
            management_authorities: vec![],
            },
        );
        let limitation = SystemFact::new(
            Domain::Diagnostics,
            "deep-scan",
            ResourceRef::global("collector", "wmi", "WMI"),
            200,
            Freshness::Current,
            Confidence::Confirmed,
            FactPayload::DiagnosticLimitation {
                collector: "wmi".into(),
                state: crate::model::CollectorState::TimedOut,
                detail: "C:\\Users\\Alice\\private".into(),
            },
            EvidenceKind::CollectorLimitation,
            "C:\\Users\\Alice\\private",
        );
        assert_eq!(machine_state_fingerprint(&[state.clone()]), machine_state_fingerprint(&[state, limitation]));
    }
}
