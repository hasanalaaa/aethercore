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
        ServiceJob::CareConsentGrant => "care consent-grant".to_string(),
        ServiceJob::InsightsList => "insights list".to_string(),
        ServiceJob::InsightsExplain { .. } => "insights explain".to_string(),
        ServiceJob::InsightsDismiss { .. } => "insights dismiss".to_string(),
        ServiceJob::ScanStart => "scan start".to_string(),
        ServiceJob::ScanCancel { .. } => "scan cancel".to_string(),
        ServiceJob::ScanStatus => "scan status".to_string(),
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
        ServiceJob::Doctor => {
            let payload = require_ok(client.call(request::Payload::GetDiagnosticsSnapshot(
                aethercore_contracts::v1::GetDiagnosticsSnapshotRequest {},
            ))?)?;
            doctor_value(&payload)
        }
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
        ServiceJob::CareConsentGrant => {
            require_ok(client.call(request::Payload::GrantCareSessionConsent(
                aethercore_contracts::v1::GrantCareSessionConsentRequest {},
            ))?)?;
            Ok(serde_json::json!({
                "granted": true,
                "scope": "this maintenance-service session",
            }))
        }
        ServiceJob::InsightsList => {
            let payload = require_ok(client.call(request::Payload::ListInsights(
                aethercore_contracts::v1::ListInsightsRequest {},
            ))?)?;
            insights_value(&payload).ok_or_else(|| CliError::ProtocolViolation {
                detail: "expected InsightsResponse".to_string(),
            })
        }
        ServiceJob::InsightsExplain { question_key } => {
            let payload = require_ok(client.call(request::Payload::RequestInsight(
                aethercore_contracts::v1::RequestInsightRequest {
                    question_key,
                    question: String::new(),
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
    println!("One-click care will run safety-level 0-1 auto steps under session consent.");
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
    if !digest.starts_with(answer) {
        return Err(CliError::ConsentRequired {
            message_key: "cli.consent.digestMismatch".to_string(),
        });
    }

    // Step 3: grant on THIS connection, then start on the SAME connection so the
    // service-side per-principal session registry sees both from one principal.
    require_ok(client.call(request::Payload::GrantCareSessionConsent(
        aethercore_contracts::v1::GrantCareSessionConsentRequest {},
    ))?)?;
    let started = require_ok(client.call(request::Payload::StartCareRun(
        aethercore_contracts::v1::StartCareRunRequest {},
    ))?)?;
    care_status_from_payload(&started).ok_or_else(|| CliError::ProtocolViolation {
        detail: "expected CareStatusResponse after start".to_string(),
    })
}
