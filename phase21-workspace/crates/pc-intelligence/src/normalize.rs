use aethercore_cleaner::{CleanupScanState, CleanupSnapshot};
use aethercore_diagnostic_engine::{DiagnosticsSnapshot, ScanState as DiagnosticScanState};
use aethercore_driver_hub::{DriverHubSnapshot, ScanState as DriverScanState};
use aethercore_persistence::InstallItemRecord;
use aethercore_startup_manager::{StartupScanState, StartupSnapshot};
use aethercore_system_repair::{RepairAssessment, RepairAssessmentState};
use aethercore_update_engine::{UpdateSnapshot, UpdateState};
use serde::Deserialize;

use crate::model::*;

pub fn driver(snapshot: &DriverHubSnapshot, now: i64) -> Vec<SystemFact> {
    let mut out = vec![SystemFact::new(
        Domain::Hardware,
        "driver-hub",
        ResourceRef::global("machine", "hardware-inventory", "Hardware inventory"),
        now,
        Freshness::Current,
        Confidence::Confirmed,
        FactPayload::InventorySummary {
            device_count: snapshot.summary.device_count,
        },
        EvidenceKind::DeviceState,
        format!(
            "devices={};authorityCoverage={}",
            snapshot.summary.device_count, snapshot.authority_coverage
        ),
    )];
    if snapshot.state != DriverScanState::Ready {
        return out;
    }
    for d in &snapshot.devices {
        let resource = ResourceRef::private(
            "pnp-device",
            &d.instance_id,
            display(&d.display_name, "Device"),
        );
        let driver_version = d
            .driver
            .as_ref()
            .map(|v| v.version.clone())
            .unwrap_or_default();
        let gpu_vendor = d
            .gpu
            .as_ref()
            .map(|g| format!("{:?}", g.vendor))
            .unwrap_or_default();
        out.push(SystemFact::new(
            Domain::Hardware,
            "driver-hub",
            resource.clone(),
            snapshot.completed_unix_ms,
            Freshness::Current,
            Confidence::Confirmed,
            FactPayload::HardwareDevice {
                class_name: d.class_name.clone(),
                manufacturer: d.manufacturer.clone(),
                description: d.description.clone(),
                installed_driver_version: driver_version.clone(),
                gpu_vendor,
            },
            EvidenceKind::DeviceState,
            format!(
                "class={};manufacturer={};driverVersion={}",
                d.class_name, d.manufacturer, driver_version
            ),
        ));
        out.push(SystemFact::new(
            Domain::Drivers, "driver-hub", resource.clone(), snapshot.completed_unix_ms, Freshness::Current, Confidence::Confirmed,
            FactPayload::DeviceHealth { missing_driver:d.missing_driver, has_problem:d.has_problem, problem_code:d.problem_code, device_state:d.device_state.clone(), update_status:d.update_status.clone(), authority_coverage:d.authority_coverage.clone(), management_authorities:d.management_authorities.iter().map(|a| format!("{}|{}|{}",a.provider_id,a.display_name,a.update_availability)).collect() },
            EvidenceKind::DeviceState,
            format!("problemCode={};state={};updateStatus={};authorityCoverage={};managementAuthorities={}", d.problem_code, d.device_state, d.update_status, d.authority_coverage, d.management_authorities.len()),
        ));
        for c in &d.candidates {
            let finding_worthy = c.candidate_id == d.recommended_candidate_id
                || matches!(
                    c.recommendation_state.as_str(),
                    "Recommended" | "Optional" | "FirmwareProtected"
                );
            if !finding_worthy {
                continue;
            }
            out.push(SystemFact::new(
                Domain::Drivers, "driver-hub", resource.clone(), snapshot.completed_unix_ms, Freshness::Current, Confidence::High,
                FactPayload::DriverUpdate {
                    candidate_id:c.candidate_id.clone(), current_version:driver_version.clone(), target_version:c.target_version.clone(),
                    vendor_managed:c.vendor_managed || c.firmware_managed, selectable:c.selectable, authority_type:c.authority_type.clone(),
                    authority_name:c.authority_name.clone(), recommendation_state:c.recommendation_state.clone(), installation_mode:c.installation_mode.clone(),
                    trust_state:c.trust_state.clone(), authority_coverage:d.authority_coverage.clone(),
                },
                EvidenceKind::UpdateOffer,
                format!("authority={};provider={};targetVersion={};recommendation={};installMode={};trust={}", c.authority_type, c.authority_name, c.target_version, c.recommendation_state, c.installation_mode, c.trust_state),
            ));
        }
    }
    out
}

pub fn repair(snapshot: &RepairAssessment) -> Vec<SystemFact> {
    if snapshot.state != RepairAssessmentState::Ready {
        return Vec::new();
    }
    snapshot
        .checks
        .iter()
        .map(|c| {
            let confidence = if c.stage.eq_ignore_ascii_case("Unknown")
                || matches!(
                    c.result_code.as_str(),
                    "ProbeUnavailable"
                        | "SystemFilesUnknown"
                        | "WinReStateUnverified"
                        | "WinReStateUnknown"
                        | "RestoreStateUnverified"
                        | "RestoreStateUnknown"
                ) {
                Confidence::Unknown
            } else {
                Confidence::Confirmed
            };
            SystemFact::new(
                Domain::Windows,
                "system-repair",
                ResourceRef::global(
                    "windows-check",
                    &c.id,
                    display(&c.title, "Windows integrity check"),
                ),
                snapshot.completed_unix_ms,
                Freshness::Current,
                confidence,
                FactPayload::WindowsIntegrity {
                    check_id: c.id.clone(),
                    result_code: c.result_code.clone(),
                    exit_code: c.exit_code,
                    detail: c.detail.clone(),
                },
                EvidenceKind::ServicingResult,
                format!("{};exitCode={}", c.result_code, c.exit_code),
            )
        })
        .collect()
}

pub fn diagnostics(snapshot: &DiagnosticsSnapshot) -> Vec<SystemFact> {
    let mut out = Vec::new();
    if !matches!(
        snapshot.state,
        DiagnosticScanState::Ready | DiagnosticScanState::Partial
    ) {
        return out;
    }
    for d in &snapshot.storage {
        let resource = ResourceRef::private(
            "storage-device",
            &d.device_id,
            display(&d.friendly_name, "Storage device"),
        );
        let r = &d.reliability;
        out.push(SystemFact::new(
            Domain::Storage,
            "diagnostic-engine",
            resource,
            snapshot.completed_unix_ms,
            Freshness::Current,
            Confidence::Confirmed,
            FactPayload::StorageHealth {
                health_status: d.windows_health_status.clone(),
                source_severity: d.severity.clone(),
                // DBT-P46-B3: carry the collector's own distinction through
                // instead of re-defaulting it away one layer downstream of B2.
                uncorrected_read_errors: r.read_errors_uncorrected,
                uncorrected_write_errors: r.write_errors_uncorrected,
                nvme_critical_warning: r.nvme_critical_warning,
                nvme_media_errors_nonzero: nonzero(r.nvme_media_errors.as_deref()),
                wear_percent: r
                    .wear_percent_used
                    .map(u64::from)
                    .or(r.nvme_percentage_used.map(u64::from)),
                temperature_c: r.temperature_c.map(i64::from),
                temperature_max_c: r.temperature_max_c.map(i64::from),
            },
            EvidenceKind::StorageHealth,
            format!("health={};severity={}", d.windows_health_status, d.severity),
        ));
    }
    if let Some(m) = &snapshot.memory {
        out.push(SystemFact::new(
            Domain::Memory,
            "diagnostic-engine",
            ResourceRef::global("memory", "system-memory", "System memory"),
            snapshot.completed_unix_ms,
            Freshness::Current,
            Confidence::Confirmed,
            FactPayload::MemoryPressure {
                memory_load_percent: m.memory_load_percent,
                pressure_label: m.pressure_label.clone(),
            },
            EvidenceKind::DeviceState,
            format!(
                "load={}%;availableBytes={};totalBytes={};pressure={}",
                m.memory_load_percent,
                m.available_physical_bytes,
                m.total_physical_bytes,
                m.pressure_label
            ),
        ));
    }
    for e in &snapshot.events {
        out.push(SystemFact::new(
            Domain::Hardware,
            "crash-diagnostics",
            ResourceRef::global(
                "hardware-event",
                format!("{}-{}-{}", e.provider, e.event_id, e.recorded_unix_ms),
                "Hardware error evidence",
            ),
            e.recorded_unix_ms,
            Freshness::Recent,
            Confidence::High,
            FactPayload::HardwareEvent {
                category: e.category.clone(),
                provider: e.provider.clone(),
                event_id: e.event_id,
            },
            EvidenceKind::HardwareEvent,
            format!(
                "provider={};eventId={};category={}",
                e.provider, e.event_id, e.category
            ),
        ));
    }
    for c in &snapshot.crashes {
        out.push(SystemFact::new(
            Domain::Diagnostics,
            "crash-diagnostics",
            ResourceRef::global("crash", &c.crash_id, "System crash"),
            // DBT-P46-B6: the dump proves a crash happened; its mtime proves
            // WHEN. If the mtime could not be read, observe the fact at scan
            // time and mark it Historical rather than asserting a crash time
            // (epoch 0) the collector never established.
            c.recorded_unix_ms.unwrap_or(snapshot.completed_unix_ms),
            if c.recorded_unix_ms.is_some() {
                Freshness::Recent
            } else {
                Freshness::Historical
            },
            Confidence::Confirmed,
            FactPayload::Crash {
                crash_id: c.crash_id.clone(),
                bugcheck_hex: c.bugcheck_hex.clone(),
            },
            EvidenceKind::CrashRecord,
            format!("bugcheck={};source={}", c.bugcheck_hex, c.source),
        ));
    }
    for fault in &snapshot.provider_faults {
        // ProviderFaultRecord.kind is the Debug string of a collector FaultKind; match on that
        // textual contract so normalization never misreads an unknown kind as success.
        let state = match fault.kind.as_str() {
            "Timeout" => CollectorState::TimedOut,
            "Cancelled" => CollectorState::Cancelled,
            "Unavailable" => CollectorState::Unavailable,
            "PermissionDenied" => CollectorState::PermissionDenied,
            _ => CollectorState::Failed,
        };
        out.push(limitation(
            &fault.provider,
            state,
            &fault.detail,
            snapshot.completed_unix_ms,
        ));
    }
    out
}

pub fn startup(snapshot: &StartupSnapshot) -> Vec<SystemFact> {
    if snapshot.state != StartupScanState::Ready {
        return Vec::new();
    }
    vec![SystemFact::new(
        Domain::Startup,
        "startup-manager",
        ResourceRef::global("startup", "startup-footprint", "Startup footprint"),
        snapshot.completed_unix_ms,
        Freshness::Current,
        Confidence::Confirmed,
        FactPayload::StartupFootprint {
            high_impact_count: snapshot.summary.high_impact,
            manageable_count: snapshot.summary.manageable,
            total_count: snapshot.summary.total,
        },
        EvidenceKind::StartupRegistration,
        format!(
            "total={};highImpact={};manageable={}",
            snapshot.summary.total, snapshot.summary.high_impact, snapshot.summary.manageable
        ),
    )]
}

pub fn cleanup(snapshot: &CleanupSnapshot) -> Vec<SystemFact> {
    if snapshot.state != CleanupScanState::Ready {
        return Vec::new();
    }
    snapshot
        .candidates
        .iter()
        .map(|c| {
            SystemFact::new(
                Domain::Cleanup,
                "cleaner",
                ResourceRef::global(
                    "cleanup-candidate",
                    &c.candidate_id,
                    display(&c.title, "Cleanup opportunity"),
                ),
                snapshot.completed_unix_ms,
                Freshness::Current,
                Confidence::Confirmed,
                FactPayload::CleanupOpportunity {
                    candidate_id: c.candidate_id.clone(),
                    reclaimable_bytes: c.reclaimable_bytes,
                    file_count: c.file_count,
                    requires_confirmation: c.requires_explicit_confirmation,
                },
                EvidenceKind::CleanupEstimate,
                format!("bytes={};files={}", c.reclaimable_bytes, c.file_count),
            )
        })
        .collect()
}

pub fn update(snapshot: &UpdateSnapshot, now: i64) -> SystemFact {
    SystemFact::new(
        Domain::Updates,
        "update-engine",
        ResourceRef::global(
            "application",
            "aethercore",
            aethercore_product_identity::PRODUCT_NAME,
        ),
        if snapshot.updated_unix_ms > 0 {
            snapshot.updated_unix_ms
        } else {
            now
        },
        Freshness::Current,
        Confidence::Confirmed,
        FactPayload::UpdateState {
            state: format!("{:?}", snapshot.state),
            current_version: snapshot.current_version.clone(),
            available_release_id: snapshot
                .latest_release
                .as_ref()
                .map(|release| release.release_id.clone())
                .unwrap_or_default(),
            update_available: matches!(
                snapshot.state,
                UpdateState::Available | UpdateState::Staged | UpdateState::AwaitingConsent
            ),
            failed: snapshot.state == UpdateState::Failed,
        },
        EvidenceKind::UpdateState,
        format!("state={:?};channel={:?}", snapshot.state, snapshot.channel),
    )
}

pub fn limitation(id: &str, state: CollectorState, detail: &str, now: i64) -> SystemFact {
    SystemFact::new(
        Domain::Diagnostics,
        "deep-scan",
        ResourceRef::global("collector", id, id),
        now,
        Freshness::Current,
        Confidence::Confirmed,
        FactPayload::DiagnosticLimitation {
            collector: id.into(),
            state,
            detail: detail.into(),
        },
        EvidenceKind::CollectorLimitation,
        detail,
    )
}
fn display(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.into()
    } else {
        value.trim().to_owned()
    }
}
fn nonzero(value: Option<&str>) -> bool {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .is_some_and(|v| {
            v.parse::<u128>().map(|n| n > 0).unwrap_or_else(|_| {
                v.trim_start_matches('0')
                    .trim_start_matches('x')
                    .chars()
                    .any(|c| c != '0')
            })
        })
}

#[derive(Deserialize)]
struct InstalledDriverEvidence {
    #[serde(default)]
    version: String,
    #[serde(default)]
    provider: String,
}

pub fn driver_change(item: &InstallItemRecord) -> SystemFact {
    let raw_identity = format!(
        "{}|{}|{}",
        item.instance_id, item.candidate_id, item.updated_unix_ms
    );
    let before = serde_json::from_str::<InstalledDriverEvidence>(&item.before_driver_json)
        .unwrap_or(InstalledDriverEvidence {
            version: String::new(),
            provider: String::new(),
        });
    let after = serde_json::from_str::<InstalledDriverEvidence>(&item.after_driver_json).unwrap_or(
        InstalledDriverEvidence {
            version: String::new(),
            provider: String::new(),
        },
    );
    let version = after.version.clone();
    SystemFact::new(
        Domain::Drivers,
        "driver-install-journal",
        ResourceRef::private(
            "driver-change",
            &raw_identity,
            display(&item.title, "Driver change"),
        ),
        item.updated_unix_ms,
        Freshness::Recent,
        Confidence::Confirmed,
        FactPayload::DriverChange {
            installed_unix_ms: item.updated_unix_ms,
            previous_version: before.version.clone(),
            version: version.clone(),
            authority_type: String::new(),
            authority_provider_id: String::new(),
            provider: after.provider.clone(),
            reboot_required: item.reboot_required,
            verified: item.verified,
            rollback_available: !item.backup_path.is_empty(),
        },
        EvidenceKind::DriverVersion,
        format!(
            "verified={};previousVersion={};version={};provider={};reboot={};rollbackAvailable={}",
            item.verified,
            before.version,
            version,
            after.provider,
            item.reboot_required,
            !item.backup_path.is_empty()
        ),
    )
}
