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

/// How long a storage-health reading stays a statement about the disk as it is now.
const STORAGE_WINDOW_MS: i64 = 24 * 60 * 60_000;
/// A memory-load reading is a live one; minutes later it describes a moment that has passed.
const MEMORY_WINDOW_MS: i64 = 5 * 60_000;

/// A reading is current inside its window and stale after it. The age is never negative: a clock
/// that moved back after the scan makes a reading "now", not "from the future".
fn measured_freshness(observed_unix_ms: i64, now: i64, window_ms: i64) -> Freshness {
    if now.saturating_sub(observed_unix_ms) <= window_ms {
        Freshness::Current
    } else {
        Freshness::Stale
    }
}

pub fn diagnostics(snapshot: &DiagnosticsSnapshot, now: i64) -> Vec<SystemFact> {
    let mut out = Vec::new();
    if !matches!(
        snapshot.state,
        DiagnosticScanState::Ready | DiagnosticScanState::Partial
    ) {
        return out;
    }
    for d in &snapshot.storage {
        // The device id is a slot (PHYSICALDRIVE0); the serial number is what makes it this disk.
        let resource = ResourceRef::private(
            "storage-device",
            &d.device_id,
            display(&d.friendly_name, "Storage device"),
        )
        .with_identity(&d.serial_number);
        let r = &d.reliability;
        out.push(SystemFact::new(
            Domain::Storage,
            "diagnostic-engine",
            resource,
            snapshot.completed_unix_ms,
            measured_freshness(snapshot.completed_unix_ms, now, STORAGE_WINDOW_MS),
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
            measured_freshness(snapshot.completed_unix_ms, now, MEMORY_WINDOW_MS),
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
    // A zone is a fact only when it gives both a reading and its own rated critical trip point:
    // without the threshold nothing can be called too hot.
    for zone in &snapshot.thermal_zones {
        let (Some(temperature_c), Some(critical_c)) = (zone.temperature_c, zone.critical_c) else {
            continue;
        };
        let observed = zone
            .coverage
            .observed_unix_ms
            .unwrap_or(snapshot.completed_unix_ms);
        out.push(SystemFact::new(
            Domain::Hardware,
            "hardware-telemetry",
            ResourceRef::private(
                "thermal-zone",
                &zone.stable_id,
                display(&zone.display_name, "Thermal zone"),
            ),
            observed,
            measured_freshness(observed, now, MEMORY_WINDOW_MS),
            Confidence::High,
            FactPayload::ThermalZone {
                temperature_c: i64::from(temperature_c),
                critical_c: i64::from(critical_c),
            },
            EvidenceKind::DeviceState,
            format!("temperatureC={temperature_c};ratedCriticalC={critical_c}"),
        ));
    }
    for battery in &snapshot.batteries {
        use aethercore_diagnostic_engine::measurements::Availability;
        let (Some(design), Some(full), Some(observed)) = (
            battery.design_capacity_mwh,
            battery.full_charge_capacity_mwh,
            battery.coverage.observed_unix_ms,
        ) else {
            continue;
        };
        if design == 0
            || full == 0
            || battery.design_capacity_relative.is_some()
            || battery.full_charge_capacity_relative.is_some()
            || battery.coverage.availability != Availability::Measured
            || observed <= 0
            || observed > now
            || battery.stable_id.trim().is_empty()
            || battery.coverage.source.trim().is_empty()
        {
            continue;
        }
        out.push(SystemFact::new(
            Domain::Hardware,
            &battery.coverage.source,
            ResourceRef::private(
                "battery",
                &battery.stable_id,
                display(&battery.display_name, "Battery"),
            ),
            observed,
            measured_freshness(observed, now, MEMORY_WINDOW_MS),
            Confidence::Confirmed,
            FactPayload::BatteryCapacity {
                design_capacity_mwh: design,
                full_charge_capacity_mwh: full,
            },
            EvidenceKind::DeviceState,
            format!("designCapacityMwh={design};fullChargeCapacityMwh={full}"),
        ));
    }
    // An accumulated lifetime counter or unavailable/reset window is not a current fault or
    // resolution proof. Default routes, cable/media and virtual status do not diagnose internet.
    for adapter in &snapshot.network_adapters {
        use aethercore_diagnostic_engine::measurements::Availability;
        let (Some(delta), Some(observed)) =
            (adapter.counter_delta, adapter.coverage.observed_unix_ms)
        else {
            continue;
        };
        if adapter.stable_id.trim().is_empty()
            || adapter.coverage.source.trim().is_empty()
            || adapter.coverage.availability != Availability::Measured
            || adapter.counter_availability != Availability::Measured
            || adapter.admin_enabled != Some(true)
            || adapter.operational_status != Some(1)
            || delta.elapsed_ms == 0
            || observed <= 0
            || observed > now
        {
            continue;
        }
        out.push(SystemFact::new(
            Domain::Hardware,
            &adapter.coverage.source,
            ResourceRef::private(
                "network-adapter",
                &adapter.stable_id,
                display(&adapter.display_name, "Network adapter"),
            ),
            observed,
            measured_freshness(observed, now, MEMORY_WINDOW_MS),
            Confidence::Confirmed,
            FactPayload::NetworkCounterWindow {
                elapsed_ms: delta.elapsed_ms,
                in_errors: delta.counts.in_errors,
                out_errors: delta.counts.out_errors,
                in_discards: delta.counts.in_discards,
                out_discards: delta.counts.out_discards,
            },
            EvidenceKind::DeviceState,
            format!(
                "windowMs={};inErrors={};outErrors={};inDiscards={};outDiscards={}",
                delta.elapsed_ms,
                delta.counts.in_errors,
                delta.counts.out_errors,
                delta.counts.in_discards,
                delta.counts.out_discards
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

#[cfg(test)]
mod measured_network_tests {
    use super::*;
    use aethercore_diagnostic_engine::measurements::{
        Availability, Coverage, NetworkAdapter, NetworkCounterDelta, NetworkCounters,
    };
    fn adapter() -> NetworkAdapter {
        NetworkAdapter {
            stable_id: "guid-1".into(),
            display_name: "VPN".into(),
            admin_enabled: Some(true),
            operational_status: Some(1),
            is_virtual: Some(true),
            default_route_v4: Some(false),
            default_route_v6: Some(true),
            counter_availability: Availability::Measured,
            counters: Some(NetworkCounters {
                in_errors: u64::MAX,
                ..Default::default()
            }),
            counter_delta: Some(NetworkCounterDelta {
                elapsed_ms: 1000,
                counts: NetworkCounters {
                    in_errors: 1,
                    ..Default::default()
                },
            }),
            coverage: Coverage {
                source: "GetIfEntry2".into(),
                observed_unix_ms: Some(1000),
                availability: Availability::Measured,
                ..Default::default()
            },
            ..Default::default()
        }
    }
    fn findings(value: NetworkAdapter, now: i64) -> Vec<Finding> {
        crate::rules::evaluate(
            &diagnostics(
                &DiagnosticsSnapshot {
                    state: DiagnosticScanState::Ready,
                    completed_unix_ms: 1000,
                    network_adapters: vec![value],
                    ..Default::default()
                },
                now,
            ),
            now,
        )
    }
    #[test]
    fn actual_network_delta_yields_only_an_informational_window_observation() {
        let found = findings(adapter(), 1000);
        let finding = found
            .iter()
            .find(|f| f.code == "NETWORK_COUNTER_ERRORS_OBSERVED")
            .expect("measured error window");
        assert_eq!(finding.severity, Severity::Informational);
        assert!(!finding.remediation_available && !finding.automatic_eligible);
        assert_eq!(finding.evidence[0].source, "GetIfEntry2");
        assert_eq!(finding.evidence[0].observed_unix_ms, 1000);
        assert!(
            finding.evidence[0]
                .technical_value
                .contains("windowMs=1000")
        );
        assert_eq!(
            finding.message_args["inErrors"], "1",
            "the cumulative u64MAX is not this window"
        );
    }
    #[test]
    fn first_reset_down_missing_time_and_stale_network_samples_do_not_raise_or_resolve_a_fault() {
        let base = adapter();
        let mut variants = vec![];
        let mut first = base.clone();
        first.counter_delta = None;
        variants.push(first);
        let mut down = base.clone();
        down.operational_status = Some(2);
        variants.push(down);
        let mut disabled = base.clone();
        disabled.admin_enabled = Some(false);
        variants.push(disabled);
        let mut missing = base.clone();
        missing.coverage.observed_unix_ms = None;
        variants.push(missing);
        let mut failed = base.clone();
        failed.counter_availability = Availability::Failed;
        variants.push(failed);
        for value in variants {
            let facts = diagnostics(
                &DiagnosticsSnapshot {
                    state: DiagnosticScanState::Ready,
                    network_adapters: vec![value],
                    ..Default::default()
                },
                1000,
            );
            assert!(
                facts
                    .iter()
                    .all(|f| f.payload.kind_name() != "networkCounterWindow")
            );
            assert!(crate::rules::evaluate(&facts, 1000).is_empty());
        }
        assert!(findings(base, 3600000).is_empty());
    }
}

#[cfg(test)]
mod battery_capacity_tests {
    use super::*;
    use aethercore_diagnostic_engine::measurements::{Availability, Battery, Coverage};
    fn battery() -> Battery {
        Battery {
            stable_id: "pack0".into(),
            display_name: "Battery".into(),
            design_capacity_mwh: Some(56000),
            full_charge_capacity_mwh: Some(41000),
            coverage: Coverage {
                source: "IOCTL_BATTERY_QUERY_INFORMATION".into(),
                observed_unix_ms: Some(1000),
                availability: Availability::Measured,
                ..Default::default()
            },
            ..Default::default()
        }
    }
    fn facts(value: Battery, now: i64) -> Vec<SystemFact> {
        diagnostics(
            &DiagnosticsSnapshot {
                state: DiagnosticScanState::Ready,
                completed_unix_ms: 1000,
                batteries: vec![value],
                ..Default::default()
            },
            now,
        )
    }
    #[test]
    fn absolute_capacity_loss_is_an_informational_estimate_with_its_measured_basis() {
        let found = crate::rules::evaluate(&facts(battery(), 1000), 1000);
        let value = found
            .iter()
            .find(|f| f.code == "BATTERY_CAPACITY_BELOW_DESIGN")
            .expect("capacity interpretation");
        assert_eq!(value.severity, Severity::Informational);
        assert!(!value.remediation_available && !value.automatic_eligible);
        assert_eq!(value.message_args["designCapacityMwh"], "56000");
        assert_eq!(value.message_args["fullChargeCapacityMwh"], "41000");
        assert_eq!(value.message_args["lossPercent"], "26.7");
        assert_eq!(value.evidence[0].source, "IOCTL_BATTERY_QUERY_INFORMATION");
        assert_eq!(value.evidence[0].observed_unix_ms, 1000);
    }
    #[test]
    fn relative_zero_missing_failed_and_old_capacity_cannot_become_a_health_percentage() {
        let base = battery();
        let mut variants = vec![];
        let mut relative = base.clone();
        relative.design_capacity_mwh = None;
        relative.full_charge_capacity_mwh = None;
        relative.design_capacity_relative = Some(100);
        relative.full_charge_capacity_relative = Some(87);
        variants.push(relative);
        let mut mixed = base.clone();
        mixed.design_capacity_relative = Some(100);
        variants.push(mixed);
        let mut zero = base.clone();
        zero.design_capacity_mwh = Some(0);
        variants.push(zero);
        let mut missing = base.clone();
        missing.coverage.observed_unix_ms = None;
        variants.push(missing);
        let mut failed = base.clone();
        failed.coverage.availability = Availability::Failed;
        variants.push(failed);
        for value in variants {
            let facts = facts(value, 1000);
            assert!(
                facts
                    .iter()
                    .all(|f| f.payload.kind_name() != "batteryCapacity")
            );
            assert!(crate::rules::evaluate(&facts, 1000).is_empty());
        }
        assert!(crate::rules::evaluate(&facts(base, 3600000), 3600000).is_empty());
    }
}
