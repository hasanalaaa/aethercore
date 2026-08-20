use aethercore_cleaner::{CleanupExecutionStatus, CleanupSnapshot};
use aethercore_contracts::v1::{self};
use aethercore_diagnostic_engine::{DiagnosticHistoryEntry, DiagnosticsSnapshot};
use aethercore_driver_hub::DriverHubSnapshot;
use aethercore_driver_install::{InstallStatus, RecoveryEntry};
use aethercore_operation_engine::{OperationEngine, PlanView};
use aethercore_startup_manager::{StartupExecutionStatus, StartupHistoryEntry, StartupSnapshot};
use aethercore_system_repair::{RepairAssessment, RepairCheck, RepairExecutionStatus};
use chrono::Utc;

const VERSION:&str=env!("CARGO_PKG_VERSION");

fn lower_camel_debug<T: std::fmt::Debug>(value: T) -> String {
    let raw = format!("{value:?}");
    let mut chars = raw.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => raw,
    }
}

pub(crate) fn consent_intent_proto(intent: aethercore_operation_engine::ConsentIntentView) -> v1::ConsentIntentResponse {
    let risk_code=risk_code(&intent.risk) as i32;
    v1::ConsentIntentResponse { intent_id:intent.intent_id, plan_id:intent.plan_id, plan_digest:intent.plan_digest, title:intent.title, risk:intent.risk, action_count:intent.action_count, expires_unix_ms:intent.expires_unix_ms, risk_code }
}

#[cfg(windows)]
pub(crate) fn snapshot(engine: &OperationEngine, owner_principal_key: &str) -> aethercore_operation_engine::Result<v1::ServiceSnapshot> {
    let active_plan = engine.latest_plan_for_owner(owner_principal_key)?.map(plan_proto);
    Ok(v1::ServiceSnapshot {
        service_version: VERSION.into(),
        health: "Platform ready".into(),
        server_time_unix_ms: Utc::now().timestamp_millis(),
        active_plan,
        journal_event_count: engine.event_count_for_owner(owner_principal_key)?,
    })
}


pub(crate) fn driver_hub_proto(snapshot: DriverHubSnapshot) -> v1::DriverHubSnapshot {
    let state_code = discovery_state_code(snapshot.state.as_str()) as i32;
    v1::DriverHubSnapshot {
        scan_id: snapshot.scan_id,
        state: snapshot.state.as_str().into(),
        inventory_epoch: snapshot.inventory_epoch,
        started_unix_ms: snapshot.started_unix_ms,
        completed_unix_ms: snapshot.completed_unix_ms,
        error_message: snapshot.error_message,
        summary: Some(v1::DriverHubSummary {
            device_count: snapshot.summary.device_count,
            missing_driver_count: snapshot.summary.missing_driver_count,
            problem_device_count: snapshot.summary.problem_device_count,
            update_offer_count: snapshot.summary.update_offer_count,
            matched_device_count: snapshot.summary.matched_device_count,
            matched_candidate_count: snapshot.summary.matched_candidate_count,
            selectable_update_count: snapshot.summary.selectable_update_count,
            vendor_managed_gpu_count: snapshot.summary.vendor_managed_gpu_count,
            vendor_managed_display_count: snapshot.summary.vendor_managed_display_count,
            recommended_update_count: snapshot.summary.recommended_update_count,
            optional_update_count: snapshot.summary.optional_update_count,
            vendor_managed_update_count: snapshot.summary.vendor_managed_update_count,
            status_unknown_count: snapshot.summary.status_unknown_count,
            management_authority_count: snapshot.summary.management_authority_count,
        }),
        devices: snapshot
            .devices
            .into_iter()
            .map(|device| v1::DriverDeviceInfo {
                instance_id: device.instance_id,
                display_name: device.display_name,
                description: device.description,
                class_name: device.class_name,
                class_guid: device.class_guid,
                manufacturer: device.manufacturer,
                enumerator: device.enumerator,
                location: device.location,
                hardware_ids: device.hardware_ids,
                compatible_ids: device.compatible_ids,
                raw_status: device.raw_status,
                problem_code: device.problem_code,
                has_problem: device.has_problem,
                missing_driver: device.missing_driver,
                driver: device.driver.map(|driver| v1::InstalledDriverInfo {
                    provider: driver.provider,
                    version: driver.version,
                    inf_path: driver.inf_path,
                    date: driver.date,
                }),
                device_state: device.device_state,
                gpu: device.gpu.map(|gpu| v1::GpuManagementInfo {
                    vendor: gpu.vendor.as_str().into(),
                    app_installed: gpu.app_installed,
                    app_name: gpu.app_name,
                    official_url: gpu.official_url,
                    provider_id: gpu.provider_id,
                    management_status: gpu.management_status,
                    update_availability: gpu.update_availability,
                }),
                display_managed: device.display_managed,
                recommended_candidate_id: device.recommended_candidate_id,
                update_status: device.update_status,
                authority_coverage: device.authority_coverage,
                required_authorities: device.required_authorities,
                evaluated_authorities: device.evaluated_authorities,
                unavailable_authorities: device.unavailable_authorities,
                unsupported_authorities: device.unsupported_authorities,
                manual_authorities: device.manual_authorities,
                management_authorities: device.management_authorities.into_iter().map(|authority| v1::DriverManagementAuthorityInfo {
                    provider_id: authority.provider_id,
                    authority_type: authority.authority_type,
                    display_name: authority.display_name,
                    official_authority: authority.official_authority,
                    official_url: authority.official_url,
                    availability: authority.availability,
                    update_availability: authority.update_availability,
                    installation_mode: authority.installation_mode,
                    update_evidence_candidate_id: authority.update_evidence_candidate_id,
                }).collect(),
                candidates: device
                    .candidates
                    .into_iter()
                    .map(|candidate| v1::DriverCandidateInfo {
                        candidate_id: candidate.candidate_id,
                        update_id: candidate.update_id,
                        revision: candidate.revision,
                        title: candidate.title,
                        provider: candidate.provider,
                        manufacturer: candidate.manufacturer,
                        model: candidate.model,
                        driver_class: candidate.driver_class,
                        matched_hardware_id: candidate.matched_hardware_id,
                        match_quality: candidate.match_quality,
                        driver_date_iso: candidate.driver_date_iso,
                        min_download_bytes: candidate.min_download_bytes,
                        max_download_bytes: candidate.max_download_bytes,
                        target_version: candidate.target_version,
                        target_version_source: candidate.target_version_source,
                        selectable: candidate.selectable,
                        selected_by_default: candidate.selected_by_default,
                        vendor_managed: candidate.vendor_managed,
                        firmware_managed: candidate.firmware_managed,
                        selection_policy: candidate.selection_policy,
                        authority_type: candidate.authority_type,
                        authority_provider_id: candidate.authority_provider_id,
                        authority_name: candidate.authority_name,
                        official_source: candidate.official_source,
                        applicability: candidate.applicability,
                        trust_state: candidate.trust_state,
                        recommendation_state: candidate.recommendation_state,
                        recommendation_reasons: candidate.recommendation_reasons,
                        acquisition_mode: candidate.acquisition_mode,
                        installation_mode: candidate.installation_mode,
                        official_support_url: candidate.official_support_url,
                        recommended: candidate.recommended,
                    })
                    .collect(),
            })
            .collect(),
        unmatched_offers: snapshot
            .unmatched_offers
            .into_iter()
            .map(|offer| v1::UnmatchedDriverOfferInfo {
                update_id: offer.update_id,
                revision: offer.revision,
                title: offer.title,
                hardware_id: offer.hardware_id,
                provider: offer.provider,
                driver_class: offer.driver_class,
                min_download_bytes: offer.min_download_bytes,
                max_download_bytes: offer.max_download_bytes,
            })
            .collect(),
        warnings: snapshot.warnings,
        state_code,
        authority_coverage: snapshot.authority_coverage,
        provider_status: snapshot.provider_status,
    }
}

pub(crate) fn plan_proto(p: PlanView) -> v1::PlanSnapshot {
    let state_code=operation_state_code(p.state) as i32;
    let risk_code=risk_code(&p.risk) as i32;
    v1::PlanSnapshot {
        id:p.id, kind:p.kind, title:p.title, state:p.state.as_str().into(), digest:p.digest, risk:p.risk,
        created_unix_ms:p.created_unix_ms, updated_unix_ms:p.updated_unix_ms,
        requires_authorization:p.state==aethercore_operation_engine::PlanState::AwaitingAuthorization,
        consent_ready_until_unix_ms:p.consent_ready_until_unix_ms, action_count:p.action_count, inventory_epoch:p.inventory_epoch, scan_id:p.scan_id, state_code, risk_code,
    }
}

pub(crate) fn install_status_proto(v: InstallStatus) -> v1::DriverInstallStatus {
    let state_code = operation_state_code_str(&v.plan_state) as i32;
    v1::DriverInstallStatus {
        plan_id: v.plan_id,
        plan_state: v.plan_state,
        stage: v.stage,
        progress_known: v.progress_known,
        overall_percent: v.overall_percent,
        current_candidate_id: v.current_candidate_id,
        detail: v.detail,
        reboot_required: v.reboot_required,
        restore_point_verified: v.restore_point_verified,
        restore_point_sequence: v.restore_point_sequence,
        backup_root: v.backup_root,
        mutation_started: v.mutation_started,
        recovery_required: v.recovery_required,
        failure_message: v.failure_message,
        started_unix_ms: v.started_unix_ms,
        updated_unix_ms: v.updated_unix_ms,
        completed_unix_ms: v.completed_unix_ms,
        items: v.items.into_iter().map(|item| v1::DriverInstallItemStatus {
            candidate_id: item.candidate_id,
            instance_id: item.instance_id,
            title: item.title,
            stage: item.stage,
            progress_known: item.progress_known,
            progress_percent: item.progress_percent,
            result_code: item.result_code,
            hresult: item.hresult,
            reboot_required: item.reboot_required,
            verified: item.verified,
            before_version: item.before_version,
            after_version: item.after_version,
            before_problem_code: item.before_problem_code,
            after_problem_code: item.after_problem_code,
            backup_path: item.backup_path,
            detail: item.detail,
        }).collect(),
        bytes_downloaded: v.bytes_downloaded,
        bytes_total: v.bytes_total,
        state_code,
    }
}

pub(crate) fn recovery_proto(v: RecoveryEntry) -> v1::RecoveryEntry {
    v1::RecoveryEntry {
        seq: v.seq,
        plan_id: v.plan_id,
        severity: v.severity,
        kind: v.kind,
        summary: v.summary,
        detail: v.detail,
        restore_point_sequence: v.restore_point_sequence,
        backup_root: v.backup_root,
        created_unix_ms: v.created_unix_ms,
    }
}


pub(crate) fn repair_check_proto(v: RepairCheck) -> v1::RepairCheckInfo {
    v1::RepairCheckInfo {
        id: v.id,
        title: v.title,
        stage: v.stage,
        result_code: v.result_code,
        exit_code: v.exit_code,
        detail: v.detail,
        log_hint: v.log_hint,
    }
}

pub(crate) fn repair_assessment_proto(v: RepairAssessment) -> v1::RepairAssessmentSnapshot {
    let state_code = discovery_state_code(v.state.as_str()) as i32;
    let intelligence = v.intelligence.map(|snapshot| v1::RepairIntelligenceInfo {
        schema: snapshot.schema,
        observation_id: snapshot.observation_id,
        machine_state_fingerprint: snapshot.machine_state_fingerprint,
        facts: snapshot.facts.into_iter().map(|fact| v1::RepairFactInfo {
            id: fact.id, domain: lower_camel_debug(fact.domain), state: lower_camel_debug(fact.state), resource: fact.resource,
            evidence_code: fact.evidence_code, technical_code: fact.technical_code, detail: fact.detail,
            observed_unix_ms: fact.observed_unix_ms, confidence: lower_camel_debug(fact.confidence),
        }).collect(),
        diagnoses: snapshot.diagnoses.into_iter().map(|diagnosis| v1::RepairDiagnosisInfo {
            id: diagnosis.id, code: diagnosis.code, role: lower_camel_debug(diagnosis.role), domain: lower_camel_debug(diagnosis.domain),
            confidence: lower_camel_debug(diagnosis.confidence), scope: diagnosis.scope, evidence_ids: diagnosis.evidence_ids,
            uncertainty: diagnosis.uncertainty, rule_version: diagnosis.rule_version,
        }).collect(),
        recovery: Some(v1::RecoveryReadinessInfo {
            system_restore: lower_camel_debug(snapshot.recovery.system_restore),
            restore_point_creation: lower_camel_debug(snapshot.recovery.restore_point_creation),
            win_re: lower_camel_debug(snapshot.recovery.win_re),
            journal_recovery: lower_camel_debug(snapshot.recovery.journal_recovery),
            driver_rollback: lower_camel_debug(snapshot.recovery.driver_rollback),
        }),
        graph: Some(v1::RepairGraphInfo {
            schema: snapshot.graph.schema, valid: snapshot.graph.valid, invalid_reason: snapshot.graph.invalid_reason,
            nodes: snapshot.graph.nodes.into_iter().map(|node| v1::RepairNodeInfo {
                id: node.id, action: lower_camel_debug(node.action), safety: lower_camel_debug(node.safety), dependencies: node.dependencies,
                target_resource: node.target_resource, diagnosis_ids: node.diagnosis_ids, verification: lower_camel_debug(node.verification),
                reversibility: lower_camel_debug(node.reversibility), requires_explicit_consent: node.requires_explicit_consent,
                requires_recovery_protection: node.requires_recovery_protection, reboot_boundary_after: node.reboot_boundary_after,
                executable_automatically: node.executable_automatically,
            }).collect(),
            deterministic_order: snapshot.graph.deterministic_order, digest_sha256: snapshot.graph.digest_sha256,
        }),
    });
    v1::RepairAssessmentSnapshot {
        assessment_id: v.assessment_id,
        state: format!("{:?}", v.state),
        started_unix_ms: v.started_unix_ms,
        completed_unix_ms: v.completed_unix_ms,
        error_message: v.error_message,
        system_volume: v.system_volume,
        checks: v.checks.into_iter().map(repair_check_proto).collect(),
        state_code,
        intelligence,
    }
}

pub(crate) fn repair_status_proto(v: RepairExecutionStatus) -> v1::SystemRepairStatus {
    let state_code = operation_state_code_str(&v.plan_state) as i32;
    v1::SystemRepairStatus {
        plan_id: v.plan_id,
        plan_state: v.plan_state,
        stage: v.stage,
        progress_known: v.progress_known,
        overall_percent: v.overall_percent,
        current_step_id: v.current_step_id,
        detail: v.detail,
        mutation_started: v.mutation_started,
        recovery_required: v.recovery_required,
        failure_message: v.failure_message,
        started_unix_ms: v.started_unix_ms,
        updated_unix_ms: v.updated_unix_ms,
        completed_unix_ms: v.completed_unix_ms,
        steps: v.steps.into_iter().map(repair_check_proto).collect(),
        state_code,
        outcome: v.outcome,
        repair_graph_digest: v.repair_graph_digest,
        machine_state_fingerprint: v.machine_state_fingerprint,
        safety_tier: v.safety_tier,
        reboot_required: v.reboot_required,
        verification_state: v.verification_state,
    }
}

pub(crate) fn cleanup_snapshot_proto(v: CleanupSnapshot) -> v1::CleanupSnapshot {
    let state_code = discovery_state_code(v.state.as_str()) as i32;
    v1::CleanupSnapshot {
        scan_id: v.scan_id,
        state: format!("{:?}", v.state),
        inventory_epoch: v.inventory_epoch,
        started_unix_ms: v.started_unix_ms,
        completed_unix_ms: v.completed_unix_ms,
        error_message: v.error_message,
        total_reclaimable_bytes: v.total_reclaimable_bytes,
        total_file_count: v.total_file_count,
        candidates: v
            .candidates
            .into_iter()
            .map(|candidate| v1::CleanupCandidateInfo {
                candidate_id: candidate.candidate_id,
                provider: candidate.provider,
                title: candidate.title,
                description: candidate.description,
                reclaimable_bytes: candidate.reclaimable_bytes,
                file_count: candidate.file_count,
                selected_by_default: candidate.selected_by_default,
                requires_explicit_confirmation: candidate.requires_explicit_confirmation,
                truncated: candidate.truncated,
                special_kind: candidate.special_kind,
            })
            .collect(),
        warnings: v.warnings,
        state_code,
    }
}

pub(crate) fn cleanup_status_proto(v: CleanupExecutionStatus) -> v1::CleanupStatus {
    let state_code = operation_state_code_str(&v.plan_state) as i32;
    v1::CleanupStatus {
        plan_id: v.plan_id,
        plan_state: v.plan_state,
        stage: v.stage,
        progress_known: v.progress_known,
        overall_percent: v.overall_percent,
        current_candidate_id: v.current_candidate_id,
        detail: v.detail,
        mutation_started: v.mutation_started,
        recovery_required: v.recovery_required,
        failure_message: v.failure_message,
        reclaimed_bytes: v.reclaimed_bytes,
        skipped_bytes: v.skipped_bytes,
        started_unix_ms: v.started_unix_ms,
        updated_unix_ms: v.updated_unix_ms,
        completed_unix_ms: v.completed_unix_ms,
        items: v
            .items
            .into_iter()
            .map(|item| v1::CleanupItemStatus {
                item_id: item.item_id,
                title: item.title,
                stage: item.stage,
                result_code: item.result_code,
                bytes_affected: item.bytes_affected,
                detail: item.detail,
            })
            .collect(),
        state_code,
    }
}

pub(crate) fn parse_startup_decision(v: &str) -> std::result::Result<RecommendationDecision, String> {
    match v {
        "Unreviewed" => Ok(RecommendationDecision::Unreviewed),
        "KeepEnabled" => Ok(RecommendationDecision::KeepEnabled),
        "Disable" => Ok(RecommendationDecision::Disable),
        _ => Err(format!("unknown startup decision: {v}")),
    }
}

pub(crate) fn startup_snapshot_proto(v: StartupSnapshot) -> v1::StartupSnapshot {
    let state_code = discovery_state_code(v.state.as_str()) as i32;
    v1::StartupSnapshot {
        scan_id: v.scan_id, state: v.state.as_str().into(), inventory_epoch: v.inventory_epoch,
        started_unix_ms: v.started_unix_ms, completed_unix_ms: v.completed_unix_ms, error_message: v.error_message,
        summary: Some(v1::StartupSummaryInfo { total: v.summary.total, registry: v.summary.registry, startup_folders: v.summary.startup_folders, scheduled_tasks: v.summary.scheduled_tasks, services: v.summary.services, protected: v.summary.protected, manageable: v.summary.manageable, high_impact: v.summary.high_impact }),
        items: v.items.into_iter().map(|i| v1::StartupItemInfo { item_id:i.item_id, kind:i.kind, scope:i.scope, display_name:i.display_name, publisher:i.publisher, command:i.command, source:i.source, enabled:i.enabled, manageable:i.manageable, protected:i.protected, protection_reason:i.protection_reason, impact:i.impact, confidence:i.confidence, evidence_detail:i.evidence_detail, recommendation:i.recommendation, service_change:i.service_change }).collect(),
        warnings: v.warnings,
        state_code,
    }
}

pub(crate) fn startup_status_proto(v: StartupExecutionStatus) -> v1::StartupStatus {
    let state_code = operation_state_code_str(&v.plan_state) as i32;
    v1::StartupStatus {
        plan_id:v.plan_id, plan_state:v.plan_state, stage:v.stage, progress_known:v.progress_known, overall_percent:v.overall_percent, current_item_id:v.current_item_id, detail:v.detail, mutation_started:v.mutation_started, recovery_required:v.recovery_required, failure_message:v.failure_message, started_unix_ms:v.started_unix_ms, updated_unix_ms:v.updated_unix_ms, completed_unix_ms:v.completed_unix_ms,
        items:v.items.into_iter().map(|i|v1::StartupExecutionItemInfo{item_id:i.item_id,display_name:i.display_name,kind:i.kind,stage:i.stage,result_code:i.result_code,detail:i.detail}).collect(),
        state_code,
    }
}

pub(crate) fn startup_history_proto(v: StartupHistoryEntry) -> v1::StartupHistoryEntryInfo {
    v1::StartupHistoryEntryInfo { change_id:v.change_id, origin_change_id:v.origin_change_id, plan_id:v.plan_id, item_id:v.item_id, kind:v.kind, display_name:v.display_name, direction:v.direction, state:v.state, detail:v.detail, created_unix_ms:v.created_unix_ms, updated_unix_ms:v.updated_unix_ms, restored_unix_ms:v.restored_unix_ms, restorable:v.restorable }
}

pub(crate) fn diagnostic_history_proto(v: DiagnosticHistoryEntry) -> v1::DiagnosticHistoryEntryInfo {
    v1::DiagnosticHistoryEntryInfo{scan_id:v.scan_id,state:v.state,collected_unix_ms:v.collected_unix_ms,warning_count:v.warning_count,card_count:v.card_count}
}

pub(crate) fn diagnostics_snapshot_proto(v: DiagnosticsSnapshot) -> v1::DiagnosticsSnapshot {
    let state_code = discovery_state_code(v.state.as_str()) as i32;
    v1::DiagnosticsSnapshot{
        scan_id:v.scan_id,state:v.state.as_str().into(),started_unix_ms:v.started_unix_ms,completed_unix_ms:v.completed_unix_ms,
        storage:v.storage.into_iter().map(|d|{
            let r=d.reliability;
            v1::StorageDeviceTelemetryInfo{device_id:d.device_id,friendly_name:d.friendly_name,firmware_version:d.firmware_version,serial_number:d.serial_number,bus_type:d.bus_type,media_type:d.media_type,size_bytes:d.size_bytes,windows_health_status:d.windows_health_status,operational_status:d.operational_status,reliability:Some(v1::StorageReliabilityInfo{has_temperature:r.temperature_c.is_some(),temperature_c:r.temperature_c.unwrap_or_default(),has_temperature_max:r.temperature_max_c.is_some(),temperature_max_c:r.temperature_max_c.unwrap_or_default(),has_wear:r.wear_percent_used.is_some(),wear_percent_used:r.wear_percent_used.unwrap_or_default(),has_power_on_hours:r.power_on_hours.is_some(),power_on_hours:r.power_on_hours.unwrap_or_default(),has_read_errors_uncorrected:r.read_errors_uncorrected.is_some(),read_errors_uncorrected:r.read_errors_uncorrected.unwrap_or_default(),has_write_errors_uncorrected:r.write_errors_uncorrected.is_some(),write_errors_uncorrected:r.write_errors_uncorrected.unwrap_or_default(),has_nvme_critical_warning:r.nvme_critical_warning.is_some(),nvme_critical_warning:r.nvme_critical_warning.unwrap_or_default() as u32,has_nvme_available_spare:r.nvme_available_spare_percent.is_some(),nvme_available_spare_percent:r.nvme_available_spare_percent.unwrap_or_default() as u32,has_nvme_percentage_used:r.nvme_percentage_used.is_some(),nvme_percentage_used:r.nvme_percentage_used.unwrap_or_default() as u32,nvme_media_errors:r.nvme_media_errors.unwrap_or_default(),nvme_unsafe_shutdowns:r.nvme_unsafe_shutdowns.unwrap_or_default(),nvme_error_log_entries:r.nvme_error_log_entries.unwrap_or_default(),has_read_latency_max:r.read_latency_max_ms.is_some(),read_latency_max_ms:r.read_latency_max_ms.unwrap_or_default(),has_write_latency_max:r.write_latency_max_ms.is_some(),write_latency_max_ms:r.write_latency_max_ms.unwrap_or_default(),has_flush_latency_max:r.flush_latency_max_ms.is_some(),flush_latency_max_ms:r.flush_latency_max_ms.unwrap_or_default()}),severity:d.severity,summary:d.summary,reasons:d.reasons,source_notes:d.source_notes,ata_smart_attributes:d.ata_smart_attributes.into_iter().map(|a|v1::AtaSmartAttributeInfo{id:a.id,current:a.current,worst:a.worst,raw_value_decimal:a.raw_value_decimal,raw_value_hex:a.raw_value_hex}).collect()}
        }).collect(),
        memory:v.memory.map(|memory|v1::MemoryTelemetryInfo{total_physical_bytes:memory.total_physical_bytes,available_physical_bytes:memory.available_physical_bytes,memory_load_percent:memory.memory_load_percent,pressure_label:memory.pressure_label,pressure_explanation:memory.pressure_explanation}),
        events:v.events.into_iter().map(|e|v1::HardwareEventInfo{event_id:e.event_id,provider:e.provider,recorded_unix_ms:e.recorded_unix_ms,category:e.category,severity:e.severity,confidence:e.confidence,summary:e.summary,detail:e.detail}).collect(),
        crashes:v.crashes.into_iter().map(|c|v1::CrashRecordInfo{crash_id:c.crash_id,recorded_unix_ms:c.recorded_unix_ms,has_bugcheck_code:c.bugcheck_code.is_some(),bugcheck_code:c.bugcheck_code.unwrap_or_default(),bugcheck_hex:c.bugcheck_hex,parameters:c.parameters,dump_file:c.dump_file,dump_size_bytes:c.dump_size_bytes,source:c.source,confidence:c.confidence,summary:c.summary}).collect(),
        cards:v.cards.into_iter().map(|c|v1::DiagnosticCardInfo{card_id:c.card_id,domain:c.domain,severity:c.severity,confidence:c.confidence,title:c.title,summary:c.summary,evidence:c.evidence,actions:c.actions}).collect(),
        warnings:v.warnings,event_window_days:v.event_window_days,state_code,
        provider_faults:v.provider_faults.into_iter().map(|fault|{
            let kind_code=provider_fault_kind_code(&fault.kind) as i32;
            v1::ProviderFaultInfo{provider:fault.provider,operation:fault.operation,detail:fault.detail,kind:fault.kind,kind_code}
        }).collect(),
    }
}

pub(crate) fn provider_fault_kind_code(kind:&str)->v1::ProviderFaultKind{
    match kind {
        "Timeout"=>v1::ProviderFaultKind::Timeout,
        "Cancelled"=>v1::ProviderFaultKind::Cancelled,
        "Unavailable"=>v1::ProviderFaultKind::Unavailable,
        "PermissionDenied"=>v1::ProviderFaultKind::PermissionDenied,
        "MalformedResponse"=>v1::ProviderFaultKind::MalformedResponse,
        "ProviderFailure"=>v1::ProviderFaultKind::ProviderFailure,
        "Io"=>v1::ProviderFaultKind::Io,
        "Internal"=>v1::ProviderFaultKind::Internal,
        _=>v1::ProviderFaultKind::Unspecified,
    }
}

pub(crate) fn discovery_state_code(state: &str) -> v1::DiscoveryState {
    match state {
        "Idle" => v1::DiscoveryState::Idle,
        "Scanning" => v1::DiscoveryState::Scanning,
        "Collecting" => v1::DiscoveryState::Collecting,
        "Ready" => v1::DiscoveryState::Ready,
        "Failed" => v1::DiscoveryState::Failed,
        _ => v1::DiscoveryState::Unspecified,
    }
}

pub(crate) fn operation_state_code_str(state: &str) -> v1::OperationState {
    match state {
        "Draft" => v1::OperationState::Draft,
        "Scanning" => v1::OperationState::Scanning,
        "ReadyForReview" => v1::OperationState::ReadyForReview,
        "AwaitingAuthorization" => v1::OperationState::AwaitingAuthorization,
        "Preflight" => v1::OperationState::Preflight,
        "Protected" => v1::OperationState::Protected,
        "Executing" => v1::OperationState::Executing,
        "Verifying" => v1::OperationState::Verifying,
        "Completed" => v1::OperationState::Completed,
        "Failed" => v1::OperationState::Failed,
        "RebootPending" => v1::OperationState::RebootPending,
        "Resuming" => v1::OperationState::Resuming,
        _ => v1::OperationState::Unspecified,
    }
}

pub(crate) fn operation_state_code(state: aethercore_operation_engine::PlanState) -> v1::OperationState {
    use aethercore_operation_engine::PlanState as S;
    match state {
        S::Draft => v1::OperationState::Draft, S::Scanning => v1::OperationState::Scanning,
        S::ReadyForReview => v1::OperationState::ReadyForReview, S::AwaitingAuthorization => v1::OperationState::AwaitingAuthorization,
        S::Preflight => v1::OperationState::Preflight, S::Protected => v1::OperationState::Protected,
        S::Executing => v1::OperationState::Executing, S::Verifying => v1::OperationState::Verifying,
        S::Completed => v1::OperationState::Completed, S::Failed => v1::OperationState::Failed,
        S::RebootPending => v1::OperationState::RebootPending, S::Resuming => v1::OperationState::Resuming,
    }
}

pub(crate) fn risk_code(risk: &str) -> v1::RiskLevel {
    match risk { "Green" => v1::RiskLevel::Green, "Amber" => v1::RiskLevel::Amber, "Red" => v1::RiskLevel::Red, _ => v1::RiskLevel::Unspecified }
}



pub(crate) fn update_channel_proto(value:aethercore_update_engine::UpdateChannel)->v1::UpdateChannel{
    match value{aethercore_update_engine::UpdateChannel::Stable=>v1::UpdateChannel::Stable,aethercore_update_engine::UpdateChannel::Beta=>v1::UpdateChannel::Beta}
}
pub(crate) fn update_channel_from_proto(value:i32)->Result<aethercore_update_engine::UpdateChannel,String>{
    match v1::UpdateChannel::try_from(value).unwrap_or(v1::UpdateChannel::Unspecified){v1::UpdateChannel::Stable=>Ok(aethercore_update_engine::UpdateChannel::Stable),v1::UpdateChannel::Beta=>Ok(aethercore_update_engine::UpdateChannel::Beta),_=>Err("invalid update channel".into())}
}
pub(crate) fn update_state_proto(value:aethercore_update_engine::UpdateState)->v1::UpdateState{
    use aethercore_update_engine::UpdateState as S;
    match value{S::Disabled=>v1::UpdateState::Disabled,S::Idle=>v1::UpdateState::Idle,S::Checking=>v1::UpdateState::Checking,S::UpToDate=>v1::UpdateState::UpToDate,S::Available=>v1::UpdateState::Available,S::Staging=>v1::UpdateState::Staging,S::Staged=>v1::UpdateState::Staged,S::AwaitingConsent=>v1::UpdateState::AwaitingConsent,S::Installing=>v1::UpdateState::Installing,S::Completed=>v1::UpdateState::Completed,S::Failed=>v1::UpdateState::Failed}
}
pub(crate) fn update_release_proto(v:aethercore_update_engine::UpdateReleaseView)->v1::UpdateReleaseInfo{
    v1::UpdateReleaseInfo{release_id:v.release_id,version:v.version,channel:update_channel_proto(v.channel) as i32,published_unix_ms:v.published_unix_ms,notes_message_key:v.notes_message_key,minimum_windows_build:v.minimum_windows_build,size_bytes:v.size_bytes,sha256:v.sha256,package_kind:match v.package_kind{aethercore_update_engine::UpdatePackageKind::Burn=>v1::UpdatePackageKind::Burn} as i32}
}
pub(crate) fn update_snapshot_proto(v:aethercore_update_engine::UpdateSnapshot)->v1::UpdateSnapshot{
    v1::UpdateSnapshot{state:update_state_proto(v.state) as i32,channel:update_channel_proto(v.channel) as i32,current_version:v.current_version,latest_release:v.latest_release.map(update_release_proto),staged_release:v.staged_release.map(update_release_proto),progress_known:v.progress_known,overall_percent:v.overall_percent,bytes_completed:v.bytes_completed,bytes_total:v.bytes_total,status_message_key:v.status_message_key,checked_unix_ms:v.checked_unix_ms,updated_unix_ms:v.updated_unix_ms}
}
pub(crate) fn update_check_descriptor_proto(v:aethercore_update_engine::UpdateCheckDescriptor)->v1::UpdateCheckDescriptorResponse{v1::UpdateCheckDescriptorResponse{channel:update_channel_proto(v.channel) as i32,manifest_url:v.manifest_url,signature_url:v.signature_url,max_manifest_bytes:v.max_manifest_bytes,max_signature_bytes:v.max_signature_bytes}}
pub(crate) fn update_stage_upload_descriptor_proto(v:aethercore_update_engine::UpdateStageUploadDescriptor)->v1::UpdateStageUploadDescriptorResponse{v1::UpdateStageUploadDescriptorResponse{upload_id:v.upload_id,release:Some(update_release_proto(v.release)),package_url:v.package_url,expected_size:v.expected_size,expected_sha256:v.expected_sha256,max_chunk_bytes:v.max_chunk_bytes,expires_unix_ms:v.expires_unix_ms}}
pub(crate) fn support_preview_proto(v:aethercore_support_bundle::SupportPreview)->v1::SupportBundlePreview{
    v1::SupportBundlePreview{preview_id:v.preview_id,expires_unix_ms:v.expires_unix_ms,sections:v.sections.into_iter().map(|s|v1::SupportPreviewSection{file_name:s.file_name,display_key:s.display_key,size_bytes:s.size_bytes}).collect(),privacy:Some(v1::SupportPrivacyReport{user_path_redactions:v.privacy.user_path_redactions,account_identifier_redactions:v.privacy.account_identifier_redactions,hardware_serial_redactions:v.privacy.hardware_serial_redactions,email_redactions:v.privacy.email_redactions}),estimated_size_bytes:v.estimated_size_bytes}
}
pub(crate) fn support_ready_proto(v:aethercore_support_bundle::SupportBundleReady)->v1::SupportBundleReady{
    v1::SupportBundleReady{bundle_id:v.bundle_id,file_name:v.file_name,size_bytes:v.size_bytes,sha256:v.sha256,public_key_hex:v.public_key_hex,expires_unix_ms:v.expires_unix_ms,public_key_fingerprint_sha256:v.public_key_fingerprint_sha256}
}

pub(crate) fn pc_domain_proto(value:aethercore_pc_intelligence::Domain)->v1::PcDomain{use aethercore_pc_intelligence::Domain as D;match value{D::System=>v1::PcDomain::System,D::Hardware=>v1::PcDomain::Hardware,D::Drivers=>v1::PcDomain::Drivers,D::Windows=>v1::PcDomain::Windows,D::Storage=>v1::PcDomain::Storage,D::Memory=>v1::PcDomain::Memory,D::Diagnostics=>v1::PcDomain::Diagnostics,D::Performance=>v1::PcDomain::Performance,D::Startup=>v1::PcDomain::Startup,D::Cleanup=>v1::PcDomain::Cleanup,D::Updates=>v1::PcDomain::Updates,D::Recovery=>v1::PcDomain::Recovery}}
pub(crate) fn pc_confidence_proto(value:aethercore_pc_intelligence::Confidence)->v1::EvidenceConfidence{use aethercore_pc_intelligence::Confidence as C;match value{C::Unknown=>v1::EvidenceConfidence::Unknown,C::Low=>v1::EvidenceConfidence::Low,C::Medium=>v1::EvidenceConfidence::Medium,C::High=>v1::EvidenceConfidence::High,C::Confirmed=>v1::EvidenceConfidence::Confirmed}}
pub(crate) fn pc_severity_proto(value:aethercore_pc_intelligence::Severity)->v1::FindingSeverity{use aethercore_pc_intelligence::Severity as S;match value{S::Informational=>v1::FindingSeverity::Informational,S::Low=>v1::FindingSeverity::Low,S::Moderate=>v1::FindingSeverity::Moderate,S::High=>v1::FindingSeverity::High,S::Critical=>v1::FindingSeverity::Critical}}
pub(crate) fn deep_scan_state_proto(value:aethercore_pc_intelligence::ScanState)->v1::DeepScanState{use aethercore_pc_intelligence::ScanState as S;match value{S::Idle=>v1::DeepScanState::Idle,S::Scanning=>v1::DeepScanState::Scanning,S::Completed=>v1::DeepScanState::Completed,S::Partial=>v1::DeepScanState::Partial,S::Cancelled=>v1::DeepScanState::Cancelled,S::Failed=>v1::DeepScanState::Failed}}
pub(crate) fn pc_status_proto(value:aethercore_pc_intelligence::SystemStatus)->v1::PcSystemStatus{use aethercore_pc_intelligence::SystemStatus as S;match value{S::Healthy=>v1::PcSystemStatus::Healthy,S::AttentionRecommended=>v1::PcSystemStatus::AttentionRecommended,S::ActionRequired=>v1::PcSystemStatus::ActionRequired,S::Critical=>v1::PcSystemStatus::Critical}}
pub(crate) fn pc_collector_state_proto(value:aethercore_pc_intelligence::CollectorState)->v1::PcCollectorState{use aethercore_pc_intelligence::CollectorState as S;match value{S::Pending=>v1::PcCollectorState::Pending,S::Running=>v1::PcCollectorState::Running,S::Completed=>v1::PcCollectorState::Completed,S::CompletedWithWarnings=>v1::PcCollectorState::CompletedWithWarnings,S::Unavailable=>v1::PcCollectorState::Unavailable,S::PermissionDenied=>v1::PcCollectorState::PermissionDenied,S::TimedOut=>v1::PcCollectorState::TimedOut,S::Cancelled=>v1::PcCollectorState::Cancelled,S::Failed=>v1::PcCollectorState::Failed}}
pub(crate) fn pc_safety_proto(value:aethercore_pc_intelligence::RemediationSafety)->v1::PcRemediationSafety{use aethercore_pc_intelligence::RemediationSafety as S;match value{S::SafeAuto=>v1::PcRemediationSafety::SafeAuto,S::SafeReview=>v1::PcRemediationSafety::SafeReview,S::Sensitive=>v1::PcRemediationSafety::Sensitive,S::Manual=>v1::PcRemediationSafety::Manual,S::HardwareService=>v1::PcRemediationSafety::HardwareService}}
fn pc_resource_proto(v:aethercore_pc_intelligence::ResourceRef)->v1::PcResourceRef{v1::PcResourceRef{kind:v.kind,stable_id:v.stable_id,display_name:v.display_name}}
fn pc_evidence_proto(v:aethercore_pc_intelligence::EvidenceRef)->v1::PcEvidenceRef{v1::PcEvidenceRef{fact_id:v.fact_id,kind:format!("{:?}",v.kind),source:v.source,observed_unix_ms:v.observed_unix_ms,technical_value:v.technical_value}}
fn pc_resolution_evidence_proto(
    value: aethercore_pc_intelligence::ResolutionEvidence,
) -> v1::PcResolutionEvidence {
    v1::PcResolutionEvidence {
        scope: value.scope,
        collector_state: pc_collector_state_proto(value.collector_state) as i32,
        observed_unix_ms: value.observed_unix_ms,
        evidence_fact_ids: value.evidence_fact_ids,
    }
}

fn pc_correlation_proto(
    value: aethercore_pc_intelligence::CorrelationExplanation,
) -> v1::PcCorrelationExplanation {
    v1::PcCorrelationExplanation {
        strength: format!("{:?}", value.strength),
        time_distance_ms: value.time_distance_ms,
        shared_scope: value.shared_scope,
        rationale_key: value.rationale_key,
        contributing_fact_ids: value.contributing_fact_ids,
        conflicting_evidence_keys: value.conflicting_evidence_keys,
    }
}

fn pc_finding_proto(v: aethercore_pc_intelligence::Finding) -> v1::PcFinding {
    let has_resolved_at = v.resolved_at_unix_ms.is_some();
    v1::PcFinding {
        id: v.id,
        code: v.code,
        domain: pc_domain_proto(v.domain) as i32,
        severity: pc_severity_proto(v.severity) as i32,
        confidence: pc_confidence_proto(v.confidence) as i32,
        title_key: v.title_key,
        summary_key: v.summary_key,
        technical_key: v.technical_key,
        message_args: v
            .message_args
            .into_iter()
            .map(|(key, value)| v1::PcMessageArg { key, value })
            .collect(),
        evidence: v.evidence.into_iter().map(pc_evidence_proto).collect(),
        affected_resource: Some(pc_resource_proto(v.affected_resource)),
        first_observed_unix_ms: v.first_observed_unix_ms,
        last_observed_unix_ms: v.last_observed_unix_ms,
        lifecycle: format!("{:?}", v.lifecycle),
        remediation_available: v.remediation_available,
        remediation_safety: v
            .remediation_safety
            .map(pc_safety_proto)
            .unwrap_or(v1::PcRemediationSafety::Unspecified) as i32,
        reboot_requirement: format!("{:?}", v.reboot_requirement),
        privilege_requirement: format!("{:?}", v.privilege_requirement),
        automatic_eligible: v.automatic_eligible,
        reversibility: format!("{:?}", v.reversibility),
        estimated_impact: format!("{:?}", v.estimated_impact),
        uncertainty_key: v.uncertainty_key,
        ignored: v.ignored,
        rule_id: v.rule_id,
        rule_version: v.rule_version,
        verification_status: format!("{:?}", v.verification_status),
        resolution_authority: v.resolution_authority,
        has_resolved_at,
        resolved_at_unix_ms: v.resolved_at_unix_ms.unwrap_or_default(),
        resolution_scan_id: v.resolution_scan_id,
        resolution_reason_key: v.resolution_reason_key,
        resolution_evidence: v
            .resolution_evidence
            .into_iter()
            .map(pc_resolution_evidence_proto)
            .collect(),
        correlation: v.correlation.map(pc_correlation_proto),
    }
}

pub(crate) fn remediation_candidate_proto(v:aethercore_pc_intelligence::RemediationCandidate)->v1::PcRemediationCandidate{v1::PcRemediationCandidate{action_id:v.action_id,finding_id:v.finding_id,action_type:format!("{:?}",v.action_type),description_key:v.description_key,authority:v.authority,privilege:format!("{:?}",v.privilege),safety:pc_safety_proto(v.safety) as i32,reversibility:format!("{:?}",v.reversibility),reboot_requirement:format!("{:?}",v.reboot_requirement),expected_effect_key:v.expected_effect_key,preconditions:v.preconditions,verification_method_key:v.verification_method_key,conflicts:v.conflicts,duration_category:v.duration_category,automatic_eligible:v.automatic_eligible}}
pub(crate) fn deep_scan_snapshot_proto(v:aethercore_pc_intelligence::DeepScanSnapshot)->v1::DeepScanSnapshot{let percent=v.progress.percent();v1::DeepScanSnapshot{scan_id:v.scan_id,state:deep_scan_state_proto(v.state) as i32,status:pc_status_proto(v.status) as i32,started_unix_ms:v.started_unix_ms,completed_unix_ms:v.completed_unix_ms,progress:Some(v1::PcScanProgress{total_weight:v.progress.total_weight,completed_weight:v.progress.completed_weight,percent,completed_tasks:v.progress.completed_tasks,total_tasks:v.progress.total_tasks,active_tasks:v.progress.active_tasks,skipped_tasks:v.progress.skipped_tasks,failed_tasks:v.progress.failed_tasks,unavailable_tasks:v.progress.unavailable_tasks,current_stage_key:v.progress.current_stage_key}),facts_count:v.facts_count,findings:v.findings.into_iter().map(pc_finding_proto).collect(),remediation_candidates:v.remediation_candidates.into_iter().map(remediation_candidate_proto).collect(),collectors:v.collectors.into_iter().map(|c|v1::PcCollectorStatus{id:c.id,state:pc_collector_state_proto(c.state) as i32,stage_keys:c.stage_keys,started_unix_ms:c.started_unix_ms,completed_unix_ms:c.completed_unix_ms,detail:c.detail}).collect(),warnings:v.warnings,summary:Some(v1::PcFindingSummary{critical:v.summary.critical,high:v.summary.high,moderate:v.summary.moderate,low:v.summary.low,informational:v.summary.informational,recommended_actions:v.summary.recommended_actions,optional_optimizations:v.summary.optional_optimizations,healthy_checks:v.summary.healthy_checks}),machine_state_fingerprint:v.machine_state_fingerprint,rule_engine_version:v.rule_engine_version,app_version:v.app_version,metrics:Some(v1::PcScanMetrics{duration_ms:v.metrics.duration_ms,collector_duration_ms:v.metrics.collector_duration_ms,peak_active_tasks:v.metrics.peak_active_tasks,streamed_event_count:v.metrics.streamed_event_count,persistence_write_count:v.metrics.persistence_write_count,normalized_payload_bytes_estimate:v.metrics.normalized_payload_bytes_estimate})}}
pub(crate) fn deep_scan_history_proto(v:aethercore_pc_intelligence::DeepScanHistoryEntry)->v1::DeepScanHistoryEntry{v1::DeepScanHistoryEntry{scan_id:v.scan_id,state:deep_scan_state_proto(v.state) as i32,status:pc_status_proto(v.status) as i32,completed_unix_ms:v.completed_unix_ms,duration_ms:v.duration_ms,finding_count:v.finding_count,unavailable_collector_count:v.unavailable_collector_count,machine_state_fingerprint:v.machine_state_fingerprint}}
pub(crate) fn remediation_plan_proto(v:aethercore_pc_intelligence::RemediationPlan)->v1::PcRemediationPlan{v1::PcRemediationPlan{plan_id:v.plan_id,scan_id:v.scan_id,digest:v.digest,created_unix_ms:v.created_unix_ms,immutable:v.immutable,actions:v.actions.into_iter().map(remediation_candidate_proto).collect()}}
