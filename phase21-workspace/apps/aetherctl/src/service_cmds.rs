//! Service-backed commands (T4): every verb maps onto an EXISTING wire tag — no new
//! allocations (see docs/phase28/ROUTING_TABLE.md). All mutations ride the SAME
//! governed RPC path the desktop uses; nothing bypasses service governance (CX-3).
//!
//! Consent discipline for `care start`:
//!   1. fetch care status → print plan digest;
//!   2. interactive text-mode confirmation requires retyping the digest prefix
//!      (default answer NO; wrong digest = typed refusal);
//!   3. only then is `grant_care_session_consent` executed by THIS principal
//!      connection in-process, immediately followed by the start call;
//!   4. --non-interactive and --output json NEVER auto-consent (typed exit 6).

use crate::cli::{Config, ServiceJob};
use crate::error::CliError;
use crate::render;
#[cfg(unix)]
use crate::transport::{self, ServiceState};
use crate::transport::{CallOutcome, ServiceClient};

use aethercore_contracts::v1::{request, response};

pub fn run(config: &Config, job: ServiceJob) -> i32 {
    let command = command_label(&job);
    let outcome = execute(config, job);
    render::finish(config, &command, outcome)
}

pub fn command_label(job: &ServiceJob) -> String {
    match job {
        ServiceJob::Doctor => "doctor".to_string(),
        ServiceJob::PerfStart { .. } => "perf start".to_string(),
        ServiceJob::PerfStop => "perf stop".to_string(),
        ServiceJob::PerfSnapshot => "perf snapshot".to_string(),
        ServiceJob::PerfReport => "perf report".to_string(),
        ServiceJob::OptimizePlan { .. } => "optimize plan".to_string(),
        ServiceJob::OptimizeStart { .. } => "optimize start".to_string(),
        ServiceJob::OptimizeStatus { .. } => "optimize status".to_string(),
        ServiceJob::TimelinePage { .. } => "timeline page".to_string(),
        ServiceJob::TimelinePatterns => "timeline patterns".to_string(),
        ServiceJob::CareStatus => "care status".to_string(),
        ServiceJob::CareStart { .. } => "care start".to_string(),
        ServiceJob::CareCancel => "care cancel".to_string(),
        ServiceJob::CareConsentGrant { .. } => "care consent-grant".to_string(),
        ServiceJob::InsightsList => "insights list".to_string(),
        ServiceJob::InsightsExplain { .. } => "insights explain".to_string(),
        ServiceJob::Ask { .. } => "ask".to_string(),
        ServiceJob::InsightsDismiss { .. } => "insights dismiss".to_string(),
        ServiceJob::ScanStart => "scan start".to_string(),
        ServiceJob::ScanCancel { .. } => "scan cancel".to_string(),
        ServiceJob::ScanStatus => "scan status".to_string(),
        ServiceJob::ScanFindings => "scan findings".to_string(),
        ServiceJob::ScanHistory { .. } => "scan history".to_string(),
        ServiceJob::ExportJournal { .. } => "export journal".to_string(),
    }
}

fn execute(config: &Config, job: ServiceJob) -> Result<serde_json::Value, CliError> {
    // ONE IPC session per command (P36 defect fix).
    //
    // On Windows the Hello handshake happens inside SessionClient::connect, so the
    // pre-probe below opened a full second session per verb: detect_service (windows)
    // is itself a ServiceClient::connect whose result is discarded via `Ok(_)`, and
    // ServiceClient has no Drop, so the discarded session's Arc stays alive in its
    // reader thread and the pipe instance is never released. Every ServiceJob verb
    // therefore emitted two Hellos, the server logged "duplicate client hello ignored"
    // (services/maintenance-service/src/server.rs), and the client burned its whole
    // --timeout-ms budget before dispatching any Request. `update stage` is an
    // OfflineJob and never took this path, which is the only reason it appeared to work.
    //
    // The probe is redundant on Windows: ServiceClient::connect already returns the
    // same typed unreachable errors (cli.detect.offline and the windowsLane keys), so
    // reachability is derived from the single connect below.
    //
    // Unix keeps the pre-probe verbatim — detect_service is the sole producer of
    // ServiceState::StaleEndpointRecovered from the endpoint-exists/no-live-pid
    // signature, and of the read_live_pid fallback when the socket file is absent.
    // Neither is reconstructible from connect() alone, so unix behaviour and every
    // unix message key are unchanged.
    #[cfg(unix)]
    {
        let state = transport::detect_service(config);
        if !matches!(state, ServiceState::Reachable { .. }) {
            return Err(CliError::ServiceUnreachable {
                message_key: state.message_key().to_string(),
            });
        }
    }
    let mut client = ServiceClient::connect(config)?;
    match job {
        ServiceJob::Doctor => doctor(config, &mut client),
        ServiceJob::PerfStart { interval_ms } => {
            require_ok(client.call(request::Payload::StartPerfSampling(
                aethercore_contracts::v1::StartPerfSamplingRequest { interval_ms },
            ))?)?;
            Ok(serde_json::json!({ "accepted": true, "intervalMs": interval_ms }))
        }
        ServiceJob::PerfStop => {
            require_ok(client.call(request::Payload::StopPerfSampling(
                aethercore_contracts::v1::StopPerfSamplingRequest {},
            ))?)?;
            Ok(serde_json::json!({ "stopped": true }))
        }
        ServiceJob::PerfSnapshot => {
            let payload = require_ok(client.call(request::Payload::GetPerformanceSnapshot(
                aethercore_contracts::v1::GetPerformanceSnapshotRequest {},
            ))?)?;
            perf_snapshot_value(&payload).ok_or_else(|| CliError::ProtocolViolation {
                detail: "expected PerformanceSnapshotResponse".to_string(),
            })
        }
        ServiceJob::PerfReport => {
            let payload = require_ok(client.call(request::Payload::GetBottleneckReport(
                aethercore_contracts::v1::GetBottleneckReportRequest {},
            ))?)?;
            bottleneck_report_value(&payload).ok_or_else(|| CliError::ProtocolViolation {
                detail: "expected BottleneckReportResponse".to_string(),
            })
        }
        ServiceJob::OptimizePlan { findings } => {
            let selected_finding_ids = findings.unwrap_or_default();
            let payload = require_ok(client.call(request::Payload::CreateOptimizationPlan(
                aethercore_contracts::v1::CreateOptimizationPlanRequest {
                    selected_finding_ids,
                },
            ))?)?;
            optimization_plan_value(&payload).ok_or_else(|| CliError::ProtocolViolation {
                detail: "expected OptimizationPlanSnapshotResponse".to_string(),
            })
        }
        ServiceJob::OptimizeStart { plan_id } => {
            // The service governs execution; if it refuses we surface its typed answer.
            let payload = require_ok(client.call(request::Payload::StartOptimization(
                aethercore_contracts::v1::StartOptimizationRequest {
                    plan_id: plan_id.unwrap_or_default(),
                },
            ))?)?;
            Ok(optimization_status_value(&payload)
                .unwrap_or_else(|| serde_json::json!({ "started": true })))
        }
        ServiceJob::OptimizeStatus { plan_id } => {
            let payload = require_ok(client.call(request::Payload::GetOptimizationStatus(
                aethercore_contracts::v1::GetOptimizationStatusRequest {
                    plan_id: plan_id.unwrap_or_default(),
                },
            ))?)?;
            Ok(optimization_status_value(&payload)
                .unwrap_or_else(|| serde_json::json!({ "status": null })))
        }
        ServiceJob::TimelinePage {
            page_size,
            before_sequence,
        } => {
            let payload = require_ok(client.call(request::Payload::GetTimelinePage(
                aethercore_contracts::v1::GetTimelinePageRequest {
                    page_size,
                    before_sequence,
                    snapshot_cursor: None,
                },
            ))?)?;
            timeline_page_value(&payload).ok_or_else(|| CliError::ProtocolViolation {
                detail: "expected TimelineResponse".to_string(),
            })
        }
        ServiceJob::TimelinePatterns => {
            let payload = require_ok(client.call(request::Payload::GetRecurrencePatterns(
                aethercore_contracts::v1::GetRecurrencePatternsRequest {},
            ))?)?;
            patterns_value(&payload).ok_or_else(|| CliError::ProtocolViolation {
                detail: "expected RecurrencePatternsResponse".to_string(),
            })
        }
        ServiceJob::CareStatus => {
            let payload = require_ok(client.call(request::Payload::GetCareStatus(
                aethercore_contracts::v1::GetCareStatusRequest {},
            ))?)?;
            care_status_from_payload(&payload).ok_or_else(|| CliError::ProtocolViolation {
                detail: "expected CareStatusResponse".to_string(),
            })
        }
        ServiceJob::CareStart { non_interactive } => {
            care_start_flow(config, &mut client, non_interactive)
        }
        ServiceJob::CareCancel => {
            let payload = require_ok(client.call(request::Payload::CancelCareRun(
                aethercore_contracts::v1::CancelCareRunRequest {},
            ))?)?;
            Ok(care_status_from_payload(&payload)
                .unwrap_or_else(|| serde_json::json!({ "cancelled": true })))
        }
        ServiceJob::CareConsentGrant { plan_digest } => {
            require_ok(client.call(request::Payload::GrantCareSessionConsent(
                aethercore_contracts::v1::GrantCareSessionConsentRequest {
                    plan_digest_sha256: plan_digest,
                },
            ))?)?;
            Ok(serde_json::json!({
                "granted": true,
                "scope": "the next care run of the plan shown, within 120 s",
            }))
        }
        ServiceJob::InsightsList => {
            let payload = require_ok(client.call(request::Payload::ListInsights(
                aethercore_contracts::v1::ListInsightsRequest { locale: None },
            ))?)?;
            insights_value(&payload).ok_or_else(|| CliError::ProtocolViolation {
                detail: "expected InsightsResponse".to_string(),
            })
        }
        ServiceJob::InsightsExplain { question_key } => {
            // The prose is in the language the rest of the output is in.
            let locale = match config.lang {
                crate::i18n::Lang::En => "en",
                crate::i18n::Lang::Ar => "ar",
            };
            let payload = require_ok(client.call(request::Payload::RequestInsight(
                aethercore_contracts::v1::RequestInsightRequest {
                    question_key,
                    question: String::new(),
                    locale: locale.to_string(),
                },
            ))?)?;
            insights_value(&payload).ok_or_else(|| CliError::ProtocolViolation {
                detail: "expected InsightsResponse".to_string(),
            })
        }
        ServiceJob::InsightsDismiss { insight_id } => {
            let payload = require_ok(client.call(request::Payload::DismissInsight(
                aethercore_contracts::v1::DismissInsightRequest { insight_id },
            ))?)?;
            let mut value = insights_value(&payload).unwrap_or_else(|| serde_json::json!({}));
            value["dismissed"] = serde_json::json!(true);
            Ok(value)
        }
        ServiceJob::ScanStart => {
            let payload = require_ok(client.call(request::Payload::StartDeepScan(
                aethercore_contracts::v1::StartDeepScanRequest {},
            ))?)?;
            scan_snapshot_value(&payload).ok_or_else(|| CliError::ProtocolViolation {
                detail: "expected DeepScanSnapshotResponse".to_string(),
            })
        }
        ServiceJob::ScanCancel { scan_id } => {
            let payload = require_ok(client.call(request::Payload::CancelDeepScan(
                aethercore_contracts::v1::CancelDeepScanRequest { scan_id },
            ))?)?;
            Ok(scan_snapshot_value(&payload)
                .unwrap_or_else(|| serde_json::json!({ "cancelRequested": true })))
        }
        ServiceJob::ScanStatus => {
            let payload = require_ok(client.call(request::Payload::GetDeepScanSnapshot(
                aethercore_contracts::v1::GetDeepScanSnapshotRequest {},
            ))?)?;
            scan_snapshot_value(&payload).ok_or_else(|| CliError::ProtocolViolation {
                detail: "expected DeepScanSnapshotResponse".to_string(),
            })
        }
        ServiceJob::Ask { question, locale } => {
            let turn_id = format!(
                "cli-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or_default()
            );
            let payload = require_ok(client.call(request::Payload::AskAssistant(
                aethercore_contracts::v1::AskAssistantRequest {
                    turn_id: turn_id.clone(),
                    question,
                    locale,
                },
            ))?)?;
            let Some(response::Payload::AssistantTurn(response)) = payload else {
                return Err(CliError::ProtocolViolation {
                    detail: "expected AssistantTurnResponse".to_string(),
                });
            };
            let mut turn = response.turn.unwrap_or_default();
            let deadline = std::time::Instant::now() + config.timeout;
            while !is_terminal_turn(&turn) {
                match client.wait_assistant_turn(&turn_id, deadline)? {
                    Some(next) => turn = next,
                    None => break,
                }
            }
            let mut value = assistant_turn_value(&turn);
            value["observationTimedOut"] = serde_json::Value::from(!is_terminal_turn(&turn));
            Ok(value)
        }
        ServiceJob::ScanFindings => {
            let payload = require_ok(client.call(request::Payload::GetDeepScanSnapshot(
                aethercore_contracts::v1::GetDeepScanSnapshotRequest {},
            ))?)?;
            scan_findings_value(&payload).ok_or_else(|| CliError::ProtocolViolation {
                detail: "expected DeepScanSnapshotResponse".to_string(),
            })
        }
        ServiceJob::ScanHistory { limit } => {
            let payload = require_ok(client.call(request::Payload::GetDeepScanHistory(
                aethercore_contracts::v1::GetDeepScanHistoryRequest { limit },
            ))?)?;
            scan_history_value(&payload).ok_or_else(|| CliError::ProtocolViolation {
                detail: "expected DeepScanHistoryResponse".to_string(),
            })
        }
        ServiceJob::ExportJournal {
            from_unix_ms,
            to_unix_ms,
            out,
        } => {
            let payload = require_ok(client.call(request::Payload::ExportJournal(
                aethercore_contracts::v1::ExportJournalRequest {
                    from_unix_ms,
                    to_unix_ms,
                    owner_principal_key: String::new(),
                },
            ))?)?;
            let envelope_json = match payload {
                Some(aethercore_contracts::v1::response::Payload::ExportJournalResponse(r)) => {
                    (r.envelope_json, r.signed, r.record_count)
                }
                _ => {
                    return Err(CliError::ProtocolViolation {
                        detail: "expected ExportJournalResponse".to_string(),
                    });
                }
            };
            let (envelope_json, signed, record_count) = envelope_json;

            std::fs::write(std::path::Path::new(&out), &envelope_json).map_err(|e| {
                CliError::LocalIo {
                    message_key: "local.io.write".to_string(),
                    detail: Some(format!("write export file: {e}")),
                }
            })?;
            Ok(serde_json::json!({
                "written": out,
                "recordCount": record_count,
                "signed": signed,
            }))
        }
    }
}

/// Maps non-zero service answers to the typed rejection (exit 5) carrying the
/// SERVICE's own message key — the CLI invents nothing.
/// The one-shot health report. A service that has not collected diagnostics yet answers
/// diagnostics.stateUnavailable; doctor then collects them (a read-only scan) and reports
/// once the collection settles, within --timeout-ms.
fn doctor(config: &Config, client: &mut ServiceClient) -> Result<serde_json::Value, CliError> {
    let read = |client: &mut ServiceClient| {
        client.call(request::Payload::GetDiagnosticsSnapshot(
            aethercore_contracts::v1::GetDiagnosticsSnapshotRequest {},
        ))
    };
    let first = read(client)?;
    if first.status_code == 0
        || first.error_message_key.as_deref() != Some("diagnostics.stateUnavailable")
    {
        return doctor_value(&require_ok(first)?);
    }
    require_ok(client.call(request::Payload::StartDiagnosticsScan(
        aethercore_contracts::v1::StartDiagnosticsScanRequest {},
    ))?)?;
    let deadline = std::time::Instant::now() + config.timeout;
    loop {
        let payload = require_ok(read(client)?)?;
        let collecting = matches!(
            &payload,
            Some(response::Payload::DiagnosticsSnapshot(value))
                if value.snapshot.as_ref().is_some_and(|s| s.state == "Collecting")
        );
        if !collecting {
            return doctor_value(&payload);
        }
        if std::time::Instant::now() >= deadline {
            return Err(CliError::Timeout);
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

fn require_ok(outcome: CallOutcome) -> Result<Option<response::Payload>, CliError> {
    if outcome.status_code != 0 {
        return Err(CliError::Rejected {
            message_key: outcome
                .error_message_key
                .filter(|key| !key.is_empty())
                .unwrap_or_else(|| "cli.service.rejected".to_string()),
            detail: outcome.error_detail.filter(|detail| !detail.is_empty()),
        });
    }
    Ok(outcome.payload)
}

// ---------------------------------------------------------------------------
// proto → JSON projections (pinned keys; snake_case wire names → camelCase)
// ---------------------------------------------------------------------------

fn doctor_value(payload: &Option<response::Payload>) -> Result<serde_json::Value, CliError> {
    let Some(response::Payload::DiagnosticsSnapshot(value)) = payload.as_ref() else {
        return Err(CliError::ProtocolViolation {
            detail: "expected DiagnosticsSnapshotResponse".to_string(),
        });
    };
    let Some(snapshot) = value.snapshot.as_ref() else {
        return Err(CliError::ProtocolViolation {
            detail: "diagnostics response lost its snapshot".to_string(),
        });
    };
    let faults: Vec<serde_json::Value> = snapshot
        .provider_faults
        .iter()
        .map(|fault| {
            serde_json::json!({
                "provider": fault.provider,
                "operation": fault.operation,
                "kind": fault.kind,
                "detail": fault.detail,
            })
        })
        .collect();
    Ok(serde_json::json!({
        "scanId": snapshot.scan_id,
        "state": snapshot.state,
        "startedUnixMs": snapshot.started_unix_ms,
        "completedUnixMs": snapshot.completed_unix_ms,
        "eventWindowDays": snapshot.event_window_days,
        "warningCount": snapshot.warnings.len(),
        "warnings": snapshot.warnings,
        "eventCount": snapshot.events.len(),
        "crashCount": snapshot.crashes.len(),
        "cardCount": snapshot.cards.len(),
        "storageCount": snapshot.storage.len(),
        "providerFaults": faults,
    }))
}

fn perf_snapshot_value(payload: &Option<response::Payload>) -> Option<serde_json::Value> {
    let Some(response::Payload::PerformanceSnapshot(value)) = payload.as_ref() else {
        return None;
    };
    let snapshot = value.snapshot.as_ref()?;
    let storage: Vec<serde_json::Value> = snapshot
        .storage
        .iter()
        .map(|device| {
            serde_json::json!({
                "deviceId": device.device_id,
                "friendlyName": device.friendly_name,
                "activeTimeBp": device.active_time_bp,
                "queueDepthX100": device.queue_depth_x100,
                "avgTransferLatencyUs": device.avg_transfer_latency_us,
                "readBytesPerSec": device.read_bytes_per_sec,
                "writeBytesPerSec": device.write_bytes_per_sec,
            })
        })
        .collect();
    let process_top: Vec<serde_json::Value> = snapshot
        .process_top
        .iter()
        .map(|entry| {
            serde_json::json!({
                "pid": entry.pid,
                "name": entry.name,
                "cpuBusyBp": entry.cpu_busy_bp,
                "readBytesPerSec": entry.read_bytes_per_sec,
                "writeBytesPerSec": entry.write_bytes_per_sec,
                "workingSetBytes": entry.working_set_bytes,
            })
        })
        .collect();
    let faults: Vec<serde_json::Value> = snapshot
        .collector_faults
        .iter()
        .map(|fault| {
            serde_json::json!({
                "collector": fault.collector,
                "kind": fault.kind,
                "detail": fault.detail,
            })
        })
        .collect();
    Some(serde_json::json!({
        "capturedUnixMs": snapshot.captured_unix_ms,
        "intervalMs": snapshot.interval_ms,
        "cpu": {
            "totalBusyBp": snapshot.cpu.as_ref().map(|c| c.total_busy_bp),
            "dpcIsrBusyBp": snapshot.cpu.as_ref().map(|c| c.dpc_isr_busy_bp),
            "contextSwitchesPerSec": snapshot.cpu.as_ref().map(|c| c.context_switches_per_sec),
        },
        "power": {
            "throttleActive": snapshot.power.as_ref().map(|p| p.throttle_active),
            "hasTemperature": snapshot.power.as_ref().map(|p| p.has_temperature),
            "temperatureC": snapshot.power.as_ref().map(|p| p.temperature_c),
        },
        "memory": {
            "totalPhysicalBytes": snapshot.memory.as_ref().map(|m| m.total_physical_bytes),
            "availablePhysicalBytes": snapshot.memory.as_ref().map(|m| m.available_physical_bytes),
            "memoryLoadPercent": snapshot.memory.as_ref().map(|m| m.memory_load_percent),
            "hardFaultsPerSec": snapshot.memory.as_ref().map(|m| m.hard_faults_per_sec),
        },
        "storage": storage,
        "processTop": process_top,
        "collectorFaults": faults,
    }))
}

fn bottleneck_role_str(code: i32) -> &'static str {
    match code {
        1 => "rootCause",
        2 => "contributingCondition",
        3 => "symptom",
        _ => "unspecified",
    }
}

fn confidence_str(code: i32) -> &'static str {
    match code {
        1 => "low",
        2 => "medium",
        3 => "high",
        4 => "confirmed",
        _ => "unspecified",
    }
}

fn bottleneck_report_value(payload: &Option<response::Payload>) -> Option<serde_json::Value> {
    let Some(response::Payload::BottleneckReport(value)) = payload.as_ref() else {
        return None;
    };
    let report = value.report.as_ref()?;
    let findings: Vec<serde_json::Value> = report
        .findings
        .iter()
        .map(|finding| {
            let evidence: Vec<serde_json::Value> = finding
                .evidence
                .iter()
                .map(|item| {
                    serde_json::json!({
                        "factKey": item.fact_key,
                        "observedValue": item.observed_value,
                        "threshold": item.threshold,
                    })
                })
                .collect();
            serde_json::json!({
                "id": finding.id,
                "code": finding.code,
                "role": bottleneck_role_str(finding.role),
                "confidence": confidence_str(finding.confidence),
                "titleKey": finding.title_key,
                "summaryKey": finding.summary_key,
                "applicableActionKinds": finding.applicable_action_kinds,
                "evidence": evidence,
            })
        })
        .collect();
    Some(serde_json::json!({
        "reportId": report.report_id,
        "generatedUnixMs": report.generated_unix_ms,
        "analyzedSampleCount": report.analyzed_sample_count,
        "analysisWindowMs": report.analysis_window_ms,
        "digestSha256": report.digest_sha256,
        "ruleEngineVersion": report.rule_engine_version,
        "findings": findings,
    }))
}

fn optimization_plan_value(payload: &Option<response::Payload>) -> Option<serde_json::Value> {
    let Some(response::Payload::OptimizationPlan(value)) = payload.as_ref() else {
        return None;
    };
    let plan = value.plan.as_ref()?;
    let candidates: Vec<serde_json::Value> = plan
        .candidates
        .iter()
        .map(|candidate| {
            serde_json::json!({
                "candidateId": candidate.candidate_id,
                "kind": candidate.kind,
                "reversibility": candidate.reversibility,
                "requiresExplicitConsent": candidate.requires_explicit_consent,
                "targetPids": candidate.target_pids,
            })
        })
        .collect();
    Some(serde_json::json!({
        "planId": plan.plan_id,
        "digestSha256": plan.digest_sha256,
        "createdUnixMs": plan.created_unix_ms,
        "immutable": plan.immutable,
        "candidateCount": candidates.len(),
        "candidates": candidates,
    }))
}

fn optimization_status_value(payload: &Option<response::Payload>) -> Option<serde_json::Value> {
    let wrapper = payload.as_ref()?;
    let status = match wrapper {
        response::Payload::OptimizationStatus(value) => value.status.as_ref()?,
        _ => return None,
    };
    Some(serde_json::json!({
        "status": {
            "planId": status.plan_id,
            "planState": status.plan_state,
            "stage": status.stage,
            "progressKnown": status.progress_known,
            "overallPercent": status.overall_percent,
            "mutationStarted": status.mutation_started,
            "recoveryRequired": status.recovery_required,
            "detail": status.detail,
        }
    }))
}

fn timeline_class_str(code: i32) -> &'static str {
    match code {
        1 => "operation",
        2 => "finding",
        3 => "verification",
        4 => "recovery",
        5 => "escalation",
        _ => "unspecified",
    }
}

fn timeline_outcome_str(code: i32) -> &'static str {
    match code {
        1 => "succeeded",
        2 => "failed",
        3 => "neutral",
        _ => "unspecified",
    }
}

fn timeline_page_value(payload: &Option<response::Payload>) -> Option<serde_json::Value> {
    let Some(response::Payload::TimelinePage(page)) = payload.as_ref() else {
        return None;
    };
    let entries: Vec<serde_json::Value> = page
        .entries
        .iter()
        .map(|entry| {
            serde_json::json!({
                "sourceId": entry.source_id,
                "class": timeline_class_str(entry.class),
                "domain": entry.domain,
                "code": entry.code,
                "outcome": timeline_outcome_str(entry.outcome),
                "observedUnixMs": entry.observed_unix_ms,
                "semanticIdentitySha256": entry.semantic_identity_sha256,
            })
        })
        .collect();
    Some(serde_json::json!({
        "entryCount": entries.len(),
        "entries": entries,
        "hasMore": page.has_more,
        "nextBeforeSequence": page.next_before_sequence,
        "digestSha256": page.digest_sha256,
        "duplicatesCollapsed": page.duplicates_collapsed,
    }))
}

fn recurrence_confidence_str(code: i32) -> &'static str {
    match code {
        1 => "weak",
        2 => "moderate",
        3 => "strong",
        _ => "unspecified",
    }
}

fn patterns_value(payload: &Option<response::Payload>) -> Option<serde_json::Value> {
    let wrapper = payload.as_ref()?;
    let patterns_response = match wrapper {
        response::Payload::RecurrencePatterns(value) => value,
        _ => return None,
    };
    let patterns: Vec<serde_json::Value> = patterns_response
        .patterns
        .iter()
        .map(|pattern| {
            serde_json::json!({
                "semanticIdentitySha256": pattern.semantic_identity_sha256,
                "class": timeline_class_str(pattern.class),
                "domain": pattern.domain,
                "code": pattern.code,
                "confidence": recurrence_confidence_str(pattern.confidence),
                "occurrenceCount": pattern.occurrence_count,
                "firstObservedUnixMs": pattern.first_observed_unix_ms,
                "lastObservedUnixMs": pattern.last_observed_unix_ms,
                "meanGapMs": pattern.mean_gap_ms,
            })
        })
        .collect();
    Some(serde_json::json!({
        "patterns": patterns,
        "digestSha256": patterns_response.digest_sha256,
    }))
}

fn care_status_object(status: &aethercore_contracts::v1::CareRunStatus) -> serde_json::Value {
    let steps: Vec<serde_json::Value> = status
        .steps
        .iter()
        .map(|step| {
            serde_json::json!({
                "stepIndex": step.step_index,
                "domainPlanId": step.domain_plan_id,
                "domainKind": step.domain_kind,
                "safetyLevel": step.safety_level,
                "state": step.state,
                "outcome": step.outcome,
                "domainVerificationState": step.domain_verification_state,
                "failureMessageKey": step.failure_message_key,
            })
        })
        .collect();
    serde_json::json!({
        "runId": status.run_id,
        "state": status.state,
        "stage": status.stage,
        "sessionConsentGranted": status.session_consent_granted,
        "planDigestSha256": status.plan_digest_sha256,
        "summaryKey": status.summary_key,
        "updatedUnixMs": status.updated_unix_ms,
        "steps": steps,
    })
}

fn care_status_from_payload(payload: &Option<response::Payload>) -> Option<serde_json::Value> {
    let Some(response::Payload::CareStatus(value)) = payload.as_ref() else {
        return None;
    };
    let status = value.status.as_ref()?;
    Some(care_status_object(status))
}

fn insights_value(payload: &Option<response::Payload>) -> Option<serde_json::Value> {
    let wrapper = payload.as_ref()?;
    let insights_response = match wrapper {
        response::Payload::InsightsResponse(value) => value,
        _ => return None,
    };
    let insights: Vec<serde_json::Value> = insights_response
        .insights
        .iter()
        .map(|insight| {
            let citations: Vec<serde_json::Value> = insight
                .citations
                .iter()
                .map(|citation| {
                    serde_json::json!({
                        "evidenceId": citation.evidence_id,
                        "surface": citation.surface,
                    })
                })
                .collect();
            serde_json::json!({
                "summaryKey": insight.summary_key,
                "explanation": insight.explanation,
                "confidence": insight.confidence,
                "engine": insight.engine,
                "citations": citations,
            })
        })
        .collect();
    Some(serde_json::json!({
        "engineLabel": insights_response.engine_label,
        "insights": insights,
    }))
}

fn deep_scan_state_str(code: i32) -> &'static str {
    match code {
        1 => "idle",
        2 => "scanning",
        3 => "completed",
        4 => "partial",
        5 => "cancelled",
        6 => "failed",
        _ => "unspecified",
    }
}

fn scan_snapshot_value(payload: &Option<response::Payload>) -> Option<serde_json::Value> {
    let Some(response::Payload::DeepScanSnapshot(value)) = payload.as_ref() else {
        return None;
    };
    let snapshot = value.snapshot.as_ref()?;
    Some(serde_json::json!({
        "scanId": snapshot.scan_id,
        "state": deep_scan_state_str(snapshot.state),
        "startedUnixMs": snapshot.started_unix_ms,
        "completedUnixMs": snapshot.completed_unix_ms,
        "factsCount": snapshot.facts_count,
        "findingCount": snapshot.findings.len(),
        "remediationCandidateCount": snapshot.remediation_candidates.len(),
        "collectorCount": snapshot.collectors.len(),
        "warningCount": snapshot.warnings.len(),
        "warnings": snapshot.warnings,
        "machineStateFingerprint": snapshot.machine_state_fingerprint,
        "ruleEngineVersion": snapshot.rule_engine_version,
        "appVersion": snapshot.app_version,
    }))
}

fn is_terminal_turn(turn: &aethercore_contracts::v1::AssistantTurn) -> bool {
    matches!(turn.state, 2..=5)
}

/// Quality pass A3: a turn as the CLI reports it. The answer appears only when the citation
/// gate passed (ANSWERED); streamed or refused text is never printed as an answer.
fn assistant_turn_value(turn: &aethercore_contracts::v1::AssistantTurn) -> serde_json::Value {
    let state = match turn.state {
        1 => "streaming",
        2 => "answered",
        3 => "refused",
        4 => "faulted",
        5 => "cancelled",
        _ => "unspecified",
    };
    let refusal = match turn.refusal {
        1 => "noEvidence",
        2 => "notCovered",
        3 => "mutationActive",
        4 => "busy",
        _ => "",
    };
    let refs = |items: &[aethercore_contracts::v1::AssistantEvidenceRef]| {
        items
            .iter()
            .map(|r| serde_json::json!({ "evidenceId": r.evidence_id, "surface": r.surface, "detail": r.detail }))
            .collect::<Vec<_>>()
    };
    serde_json::json!({
        "turnId": turn.turn_id,
        "state": state,
        "answer": (turn.state == 2).then(|| turn.answer.clone()),
        "citations": refs(&turn.citations),
        "refusal": refusal,
        "faultKey": turn.fault_key,
        "engineLabel": turn.engine_label,
        "tokensEmitted": turn.tokens_emitted,
        "packSize": turn.pack.len(),
    })
}

fn pc_domain_str(code: i32) -> &'static str {
    match code {
        1 => "system",
        2 => "hardware",
        3 => "drivers",
        4 => "windows",
        5 => "storage",
        6 => "memory",
        7 => "diagnostics",
        8 => "performance",
        9 => "startup",
        10 => "cleanup",
        11 => "updates",
        12 => "recovery",
        _ => "unspecified",
    }
}

fn finding_severity_str(code: i32) -> &'static str {
    match code {
        1 => "informational",
        2 => "low",
        3 => "moderate",
        4 => "high",
        5 => "critical",
        _ => "unspecified",
    }
}

/// Quality pass B7: every finding of the current deep scan with what it cites, so a scan's
/// usefulness can be judged without the desktop. The same read-only snapshot as `scan status`.
fn scan_findings_value(payload: &Option<response::Payload>) -> Option<serde_json::Value> {
    let Some(response::Payload::DeepScanSnapshot(value)) = payload.as_ref() else {
        return None;
    };
    let snapshot = value.snapshot.as_ref()?;
    let findings: Vec<serde_json::Value> = snapshot
        .findings
        .iter()
        .map(|finding| {
            let args: serde_json::Map<String, serde_json::Value> = finding
                .message_args
                .iter()
                .map(|arg| (arg.key.clone(), serde_json::Value::from(arg.value.clone())))
                .collect();
            let evidence: Vec<serde_json::Value> = finding
                .evidence
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "factId": e.fact_id,
                        "kind": e.kind,
                        "source": e.source,
                        "observedUnixMs": e.observed_unix_ms,
                        "technicalValue": e.technical_value,
                    })
                })
                .collect();
            serde_json::json!({
                "id": finding.id,
                "code": finding.code,
                "domain": pc_domain_str(finding.domain),
                "severity": finding_severity_str(finding.severity),
                "confidence": finding.confidence,
                "titleKey": finding.title_key,
                "summaryKey": finding.summary_key,
                "ruleId": finding.rule_id,
                "args": args,
                "resource": finding.affected_resource.as_ref().map(|r| serde_json::json!({
                    "kind": r.kind,
                    "stableId": r.stable_id,
                    "displayName": r.display_name,
                })),
                "evidence": evidence,
                "lifecycle": finding.lifecycle,
                "ignored": finding.ignored,
                "remediationAvailable": finding.remediation_available,
                "remediationSafety": finding.remediation_safety,
            })
        })
        .collect();
    Some(serde_json::json!({
        "scanId": snapshot.scan_id,
        "state": deep_scan_state_str(snapshot.state),
        "findings": findings,
    }))
}

fn scan_history_value(payload: &Option<response::Payload>) -> Option<serde_json::Value> {
    let wrapper = payload.as_ref()?;
    let history = match wrapper {
        response::Payload::DeepScanHistory(value) => value,
        _ => return None,
    };
    let entries: Vec<serde_json::Value> = history
        .entries
        .iter()
        .map(|entry| {
            serde_json::json!({
                "scanId": entry.scan_id,
                "state": deep_scan_state_str(entry.state),
                "completedUnixMs": entry.completed_unix_ms,
                "durationMs": entry.duration_ms,
                "findingCount": entry.finding_count,
                "unavailableCollectorCount": entry.unavailable_collector_count,
                "machineStateFingerprint": entry.machine_state_fingerprint,
            })
        })
        .collect();
    Some(serde_json::json!({ "entries": entries }))
}

// ---------------------------------------------------------------------------
// care start + consent discipline
// ---------------------------------------------------------------------------

const DIGEST_CONFIRM_CHARS: usize = 16;

fn care_start_flow(
    config: &Config,
    client: &mut ServiceClient,
    non_interactive: bool,
) -> Result<serde_json::Value, CliError> {
    use std::io::Write as _;

    // Step 1: current care status carries the composed plan digest.
    let payload = require_ok(client.call(request::Payload::GetCareStatus(
        aethercore_contracts::v1::GetCareStatusRequest {},
    ))?)?;
    let status = care_status_from_payload(&payload).ok_or_else(|| CliError::ProtocolViolation {
        detail: "expected CareStatusResponse before consent".to_string(),
    })?;

    let digest = status["planDigestSha256"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();
    if digest.is_empty() || digest == "-" || digest == "null" {
        return Err(CliError::ConsentRequired {
            message_key: "cli.care.noPlanDigest".to_string(),
        });
    }

    // Step 2: consent must be deliberate. Machine modes and --non-interactive never
    // auto-consent; they receive the typed refusal with the printed digest reason.
    if config.output == crate::cli::OutputMode::Json || non_interactive {
        return Err(CliError::ConsentRequired {
            message_key: "cli.consent.interactiveConfirmationRequired".to_string(),
        });
    }

    let prefix: String = digest.chars().take(DIGEST_CONFIRM_CHARS).collect();
    println!("care plan digest: {digest}");
    println!(
        "Consenting approves the automatic steps of this plan, as they are now, for this one run. \
         Review-only steps never run; a step whose plan changes before it runs is refused."
    );
    print!("Type the digest prefix ({prefix}) to consent, anything else refuses [default N]: ");
    let _ = std::io::stdout().flush();

    let mut answer = String::new();
    std::io::stdin()
        .read_line(&mut answer)
        .map_err(|e| CliError::local_io_with("cli.consent.promptRead", e.to_string()))?;
    let answer = answer.trim();
    if answer.is_empty() || answer.eq_ignore_ascii_case("n") || answer.eq_ignore_ascii_case("no") {
        return Err(CliError::ConsentRequired {
            message_key: "cli.consent.declined".to_string(),
        });
    }
    // Wrong-digest confirmations are refused BEFORE any consent RPC leaves this process.
    if !digest_confirmed(&digest, answer) {
        return Err(CliError::ConsentRequired {
            message_key: "cli.consent.digestMismatch".to_string(),
        });
    }

    // Step 3: grant on THIS connection, then start on the SAME connection so the
    // service-side per-principal session registry sees both from one principal.
    let granted = require_ok(client.call(request::Payload::GrantCareSessionConsent(
        aethercore_contracts::v1::GrantCareSessionConsentRequest {
            plan_digest_sha256: digest.clone(),
        },
    ))?)?;
    // DBT-P75-045: the service approves the plan it composes at grant time. If that is not
    // the plan whose digest was just confirmed, nothing starts.
    if !granted_plan_is_confirmed(&digest, &granted) {
        return Err(CliError::ConsentRequired {
            message_key: "cli.care.planChanged".to_string(),
        });
    }
    let started = require_ok(client.call(request::Payload::StartCareRun(
        aethercore_contracts::v1::StartCareRunRequest {},
    ))?)?;
    care_status_from_payload(&started).ok_or_else(|| CliError::ProtocolViolation {
        detail: "expected CareStatusResponse after start".to_string(),
    })
}

fn granted_plan_is_confirmed(confirmed: &str, granted: &Option<response::Payload>) -> bool {
    care_status_from_payload(granted)
        .is_some_and(|status| status["planDigestSha256"].as_str() == Some(confirmed))
}

/// The prompt asks for the first DIGEST_CONFIRM_CHARS characters; anything shorter
/// (even one matching character) is not the deliberate confirmation it asked for.
fn digest_confirmed(digest: &str, answer: &str) -> bool {
    let required = digest.chars().count().min(DIGEST_CONFIRM_CHARS);
    answer.chars().count() >= required && digest.starts_with(answer)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn care_consent_requires_the_full_printed_prefix() {
        assert!(digest_confirmed(DIGEST, "0123456789abcdef"));
        assert!(
            digest_confirmed(DIGEST, DIGEST),
            "the whole digest is also deliberate"
        );
        for short in ["0", "0123", "0123456789abcde"] {
            assert!(!digest_confirmed(DIGEST, short), "{short:?} must refuse");
        }
        assert!(!digest_confirmed(DIGEST, "0123456789abcdeX"));
        // A digest shorter than the prefix length must be typed in full.
        assert!(digest_confirmed("abc", "abc"));
        assert!(!digest_confirmed("abc", "ab"));
    }

    fn granted(digest: &str) -> Option<response::Payload> {
        Some(response::Payload::CareStatus(
            aethercore_contracts::v1::CareStatusResponse {
                status: Some(aethercore_contracts::v1::CareRunStatus {
                    plan_digest_sha256: digest.to_string(),
                    ..Default::default()
                }),
            },
        ))
    }

    /// Quality pass A3: the CLI reports a turn the way the drawer must. Streamed text is never
    /// an answer: `answer` is printed only for an ANSWERED turn, with its citations.
    #[test]
    fn assistant_turn_shows_an_answer_only_once_it_is_cited() {
        use aethercore_contracts::v1::{AssistantEvidenceRef, AssistantTurn};
        let streaming = AssistantTurn {
            turn_id: "cli-1".into(),
            state: 1,
            answer: "partial text".into(),
            engine_label: "localModel".into(),
            ..Default::default()
        };
        let value = assistant_turn_value(&streaming);
        assert_eq!(value["state"], "streaming");
        assert!(value["answer"].is_null(), "streamed text is not an answer");

        let answered = AssistantTurn {
            state: 2,
            answer: "Two crashes were recorded [diag-1].".into(),
            citations: vec![AssistantEvidenceRef {
                evidence_id: "diag-1".into(),
                surface: "diagnostics".into(),
                detail: "2 crashes".into(),
            }],
            ..streaming.clone()
        };
        let value = assistant_turn_value(&answered);
        assert_eq!(value["state"], "answered");
        assert_eq!(value["answer"], "Two crashes were recorded [diag-1].");
        assert_eq!(value["citations"][0]["surface"], "diagnostics");

        let refused = AssistantTurn {
            state: 3,
            refusal: 2,
            ..streaming.clone()
        };
        let value = assistant_turn_value(&refused);
        assert_eq!(value["state"], "refused");
        assert_eq!(value["refusal"], "notCovered");
        assert!(value["answer"].is_null());

        let faulted = AssistantTurn {
            state: 4,
            fault_key: "assistant.fault.modelUnavailable".into(),
            ..streaming
        };
        assert_eq!(
            assistant_turn_value(&faulted)["faultKey"],
            "assistant.fault.modelUnavailable"
        );
        assert!(is_terminal_turn(&answered) && is_terminal_turn(&refused));
        assert!(!is_terminal_turn(&AssistantTurn {
            state: 1,
            ..Default::default()
        }));
    }

    /// Quality pass B7: the findings themselves, not just their count, so a scan can be
    /// judged from the CLI. Read-only: it is the same snapshot request as `scan status`.
    #[test]
    fn scan_findings_lists_each_finding_with_its_evidence_and_resource() {
        use aethercore_contracts::v1::{
            DeepScanSnapshot, DeepScanSnapshotResponse, PcEvidenceRef, PcFinding, PcMessageArg,
            PcResourceRef,
        };
        let payload = Some(response::Payload::DeepScanSnapshot(
            DeepScanSnapshotResponse {
                snapshot: Some(DeepScanSnapshot {
                    scan_id: "scan-1".into(),
                    state: 3,
                    findings: vec![PcFinding {
                        id: "f-1".into(),
                        code: "driver.missing".into(),
                        domain: 3,
                        severity: 4,
                        confidence: 2,
                        title_key: "finding.driver.missing.title".into(),
                        rule_id: "drivers.problem-device".into(),
                        message_args: vec![PcMessageArg {
                            key: "device".into(),
                            value: "PCI Device".into(),
                        }],
                        evidence: vec![PcEvidenceRef {
                            fact_id: "fact-9".into(),
                            kind: "PnpDeviceState".into(),
                            source: "SetupAPI".into(),
                            observed_unix_ms: 7,
                            technical_value: "CM_PROB_FAILED_INSTALL (28)".into(),
                        }],
                        affected_resource: Some(PcResourceRef {
                            kind: "device".into(),
                            stable_id: "PCI\\VEN_1".into(),
                            display_name: "PCI Device".into(),
                        }),
                        remediation_available: true,
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
            },
        ));
        let value = scan_findings_value(&payload).expect("a snapshot maps to findings");
        assert_eq!(value["scanId"], "scan-1");
        assert_eq!(value["state"], "completed");
        let findings = value["findings"].as_array().unwrap();
        assert_eq!(findings.len(), 1);
        let finding = &findings[0];
        assert_eq!(finding["code"], "driver.missing");
        assert_eq!(finding["domain"], "drivers");
        assert_eq!(finding["severity"], "high");
        assert_eq!(finding["ruleId"], "drivers.problem-device");
        assert_eq!(finding["args"]["device"], "PCI Device");
        assert_eq!(finding["resource"]["displayName"], "PCI Device");
        assert_eq!(finding["evidence"][0]["source"], "SetupAPI");
        assert_eq!(
            finding["evidence"][0]["technicalValue"],
            "CM_PROB_FAILED_INSTALL (28)"
        );
        assert_eq!(finding["remediationAvailable"], true);
        assert!(scan_findings_value(&None).is_none());
    }

    /// DBT-P75-045: a start follows the grant only when the service approved the plan whose
    /// digest the owner confirmed.
    #[test]
    fn care_starts_only_the_plan_that_was_confirmed() {
        assert!(granted_plan_is_confirmed(DIGEST, &granted(DIGEST)));
        assert!(!granted_plan_is_confirmed(DIGEST, &granted("ffff")));
        assert!(!granted_plan_is_confirmed(DIGEST, &None));
    }
}
