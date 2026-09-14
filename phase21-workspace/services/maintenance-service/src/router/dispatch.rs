//! The request preamble, the dispatch table, and the response envelope.
//!
//! Everything here ran once per request before any verb did: protocol version,
//! request-id syntax, the cancellation checkpoint and the principal/session
//! binding revalidation. `DBT-P63-004` moved the verbs out; the order of these
//! four gates, and of the dispatch arms below, is unchanged.

use super::*;

pub fn handle_request(
    ctx: &ServiceContext,
    peer: &aethercore_security::PrincipalContext,
    request_context: &RequestContext,
    req: Request,
) -> Response {
    let request_id = req
        .header
        .as_ref()
        .map(|h| h.request_id.clone())
        .unwrap_or_default();
    let request_id_valid = is_safe_request_id(&request_id);
    let header = ResponseHeader {
        protocol_version: PROTOCOL_VERSION,
        request_id: if request_id_valid {
            request_id
        } else {
            String::new()
        },
    };
    if req.header.as_ref().map(|h| h.protocol_version) != Some(PROTOCOL_VERSION) {
        return failure(
            header,
            ServiceError::new(
                426,
                v1::ErrorCode::UnsupportedProtocol,
                "ipc",
                "ipc.unsupportedProtocol",
                "unsupported protocol version",
                false,
            ),
        );
    }
    if !request_id_valid {
        return failure(
            header,
            ServiceError::invalid("ipc", "ipc.invalidRequestId", "invalid request id"),
        );
    }
    if let Err(error) = request_context.checkpoint() {
        return failure(header, error.into());
    }
    let principal_key = peer.binding_key();
    if principal_key != request_context.owner_principal_key {
        return failure(
            header,
            ServiceError::forbidden(
                "ipc",
                "ipc.principalBindingChanged",
                "request principal/session binding changed",
            ),
        );
    }

    // The four locals the arms used to close over, handed to every verb as one
    // borrow. `DBT-P63-004`: the decomposition moved the arms out of this
    // function; it did not change what they are given or the order they see it.
    let call = &Call {
        ctx,
        peer,
        request_context,
        principal_key,
    };

    let result: Routed = (|| {
        match req.payload.ok_or("missing payload")? {
            request::Payload::Ping(_) => session::ping(),
            request::Payload::GetSnapshot(_) => session::get_snapshot(call),
            request::Payload::HydrateSession(_) => session::hydrate_session(call),
            request::Payload::BeginConsentIntent(v) => consent::begin_consent_intent(call, v),
            request::Payload::GetConsentIntent(v) => consent::get_consent_intent(call, v),
            request::Payload::ApproveConsentIntent(v) => consent::approve_consent_intent(call, v),
            request::Payload::StartDriverScan(_) => drivers::start_driver_scan(call),
            request::Payload::GetDriverHubSnapshot(_) => drivers::get_driver_hub_snapshot(call),
            request::Payload::SetDriverCandidatePolicy(v) => {
                drivers::set_driver_candidate_policy(call, v)
            }
            request::Payload::CreateDriverInstallPlan(v) => {
                drivers::create_driver_install_plan(call, v)
            }
            request::Payload::StartDriverInstall(v) => drivers::start_driver_install(call, v),
            request::Payload::GetDriverInstallStatus(v) => {
                drivers::get_driver_install_status(call, v)
            }
            request::Payload::GetRecoveryHistory(v) => drivers::get_recovery_history(call, v),
            request::Payload::StartRepairAssessment(_) => repair::start_repair_assessment(call),
            request::Payload::GetRepairAssessment(_) => repair::get_repair_assessment(call),
            request::Payload::CreateSystemRepairPlan(v) => {
                repair::create_system_repair_plan(call, v)
            }
            request::Payload::StartSystemRepair(v) => repair::start_system_repair(call, v),
            request::Payload::GetSystemRepairStatus(v) => repair::get_system_repair_status(call, v),
            request::Payload::StartCleanupScan(_) => cleanup::start_cleanup_scan(call),
            request::Payload::GetCleanupSnapshot(_) => cleanup::get_cleanup_snapshot(call),
            request::Payload::CreateCleanupPlan(v) => cleanup::create_cleanup_plan(call, v),
            request::Payload::StartCleanup(v) => cleanup::start_cleanup(call, v),
            request::Payload::GetCleanupStatus(v) => cleanup::get_cleanup_status(call, v),
            request::Payload::StartStartupScan(_) => startup::start_startup_scan(call),
            request::Payload::GetStartupSnapshot(_) => startup::get_startup_snapshot(call),
            request::Payload::CreateStartupPlan(v) => startup::create_startup_plan(call, v),
            request::Payload::CreateStartupRestorePlan(v) => {
                startup::create_startup_restore_plan(call, v)
            }
            request::Payload::StartStartupChanges(v) => startup::start_startup_changes(call, v),
            request::Payload::GetStartupStatus(v) => startup::get_startup_status(call, v),
            request::Payload::GetStartupHistory(v) => startup::get_startup_history(call, v),
            request::Payload::StartDiagnosticsScan(_) => diagnostics::start_diagnostics_scan(call),
            request::Payload::StartDeepScan(_) => diagnostics::start_deep_scan(call),
            request::Payload::CancelDeepScan(v) => diagnostics::cancel_deep_scan(call, v),
            request::Payload::GetDeepScanSnapshot(_) => diagnostics::get_deep_scan_snapshot(call),
            request::Payload::GetDeepScanHistory(v) => diagnostics::get_deep_scan_history(call, v),
            request::Payload::SealRemediationPlan(v) => diagnostics::seal_remediation_plan(call, v),
            request::Payload::GetDiagnosticsSnapshot(_) => {
                diagnostics::get_diagnostics_snapshot(call)
            }
            request::Payload::GetDiagnosticsHistory(v) => {
                diagnostics::get_diagnostics_history(call, v)
            }
            request::Payload::CheckForUpdates(_) => {
                Err("legacy service-side update download is disabled".into())
            }
            request::Payload::GetUpdateSnapshot(_) => updates::get_update_snapshot(call),
            request::Payload::StageUpdate(_) => {
                Err("legacy service-side update download is disabled".into())
            }
            request::Payload::GetUpdateCheckDescriptor(v) => {
                updates::get_update_check_descriptor(call, v)
            }
            request::Payload::SubmitUpdateManifest(v) => updates::submit_update_manifest(call, v),
            request::Payload::BeginUpdateStageUpload(v) => {
                updates::begin_update_stage_upload(call, v)
            }
            request::Payload::WriteUpdateStageChunk(v) => {
                updates::write_update_stage_chunk(call, v)
            }
            request::Payload::FinalizeUpdateStageUpload(v) => {
                updates::finalize_update_stage_upload(call, v)
            }
            request::Payload::CancelUpdateStageUpload(v) => {
                updates::cancel_update_stage_upload(call, v)
            }
            request::Payload::BeginUpdateInstallIntent(v) => {
                updates::begin_update_install_intent(call, v)
            }
            request::Payload::GetUpdateInstallIntent(v) => {
                updates::get_update_install_intent(call, v)
            }
            request::Payload::ClaimUpdateInstall(v) => updates::claim_update_install(call, v),
            request::Payload::CompleteUpdateInstall(v) => updates::complete_update_install(call, v),
            request::Payload::CancelUpdateInstallIntent(v) => {
                updates::cancel_update_install_intent(call, v)
            }
            request::Payload::CreateSupportBundlePreview(v) => {
                support::create_support_bundle_preview(call, v)
            }
            request::Payload::PrepareSupportBundle(v) => support::prepare_support_bundle(call, v),
            request::Payload::ReadSupportBundleChunk(v) => {
                support::read_support_bundle_chunk(call, v)
            }
            request::Payload::DiscardSupportBundle(v) => support::discard_support_bundle(call, v),
            request::Payload::MarkSupportBundleExported(v) => {
                support::mark_support_bundle_exported(call, v)
            }
            // ---------------- Phase 20: performance intelligence ----------------
            request::Payload::StartPerfSampling(v) => performance::start_perf_sampling(call, v),
            request::Payload::StopPerfSampling(_) => performance::stop_perf_sampling(call),
            request::Payload::GetPerformanceSnapshot(_) => {
                performance::get_performance_snapshot(call)
            }
            // Phase 53 (DBT-P50-005): performance window series accessor
            request::Payload::GetPerformanceWindow(v) => {
                performance::get_performance_window(call, v)
            }
            request::Payload::GetBottleneckReport(_) => performance::get_bottleneck_report(call),
            request::Payload::CreateOptimizationPlan(v) => {
                performance::create_optimization_plan(call, v)
            }
            request::Payload::StartOptimization(v) => performance::start_optimization(call, v),
            request::Payload::GetOptimizationStatus(_) => performance::get_optimization_status(),
            // ---------------- Phase 21: timeline intelligence ----------------
            request::Payload::GetTimelinePage(v) => timeline::get_timeline_page(call, v),
            request::Payload::GetRecurrencePatterns(_) => timeline::get_recurrence_patterns(call),
            // ---------------- Phase 22: One-Click Care ----------------
            request::Payload::GetCareStatus(_) => care::get_care_status(call),
            request::Payload::GrantCareSessionConsent(_) => care::grant_care_session_consent(call),
            request::Payload::StartCareRun(_) => care::start_care_run(call),
            request::Payload::CancelCareRun(_) => care::cancel_care_run(call),
            // ---------------- Phase 23: Local Intelligence (advisory-only) ----------------
            request::Payload::ListInsights(_) => insights::list_insights(call),
            request::Payload::RequestInsight(v) => insights::request_insight(call, v),
            request::Payload::DismissInsight(v) => insights::dismiss_insight(call, v),
            // ---------------- Phase 56: the grounded assistant ----------------------
            request::Payload::AskAssistant(v) => assistant::ask_assistant(call, v),
            request::Payload::CancelAssistantTurn(v) => assistant::cancel_assistant_turn(call, v),
            // P57: the drawer's empty state reads the pack without starting a
            // turn, so it can count the rows the assistant actually holds
            // instead of listing what the feature could do in principle.
            request::Payload::GetAssistantPack(_) => assistant::get_assistant_pack(call),
            // ---------------- Phase 26/27: honest platform + engine surface ----------
            request::Payload::GetPlatformCapabilities(_) => platform::get_platform_capabilities(),
            request::Payload::GetEngineSource(_) => platform::get_engine_source(),
            // ---------------- Phase 29 (T1): signed journal export -------------------
            request::Payload::ExportJournal(v) => journal::export_journal(call, v),
            // ---------------- Phase 32 (T1): read-only security audit ----------------
            request::Payload::RunSecurityAudit(v) => security_audit::run_security_audit(call, v),
        }
    })();
    match result {
        Ok(payload) => Response {
            header: Some(header),
            status_code: 0,
            // Wire contract keeps the deprecated placeholder field; empty by design.
            #[allow(deprecated)]
            error_message: String::new(),
            error: None,
            payload,
        },
        Err(error) => failure(header, error),
    }
}

fn is_safe_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_REQUEST_ID_BYTES
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
}

fn failure(header: ResponseHeader, error: ServiceError) -> Response {
    let correlation_id = header.request_id.clone();
    let detail: String = error.detail.chars().take(512).collect();
    Response {
        header: Some(header),
        status_code: error.status,
        #[allow(deprecated)] // wire-contract placeholder field (proto field 3)
        error_message: detail.clone(),
        error: Some(v1::ErrorInfo {
            code: error.code as i32,
            domain: error.domain.into(),
            message_key: error.message_key.into(),
            message_args: Vec::new(),
            correlation_id,
            technical_detail: detail,
            retryable: error.retryable,
        }),
        payload: None,
    }
}
