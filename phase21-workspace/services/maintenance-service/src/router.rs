use std::{path::PathBuf, sync::Arc};

use aethercore_cleaner::CleanupEngine;
use aethercore_contracts::{
    MAX_REQUEST_ID_BYTES, PROTOCOL_VERSION,
    v1::{self, EventKind, Request, Response, ResponseHeader, event_envelope, request, response},
};
use aethercore_diagnostic_engine::DiagnosticEngine;
use aethercore_driver_hub::DriverHub;
use aethercore_driver_install::DriverInstallCoordinator;
use aethercore_operation_engine::OperationEngine;
use aethercore_operation_kernel::{
    MutationWorkload, OperationKernel, ReadWorkload, RequestContext,
};
use aethercore_pc_intelligence::DeepScanCoordinator;
use aethercore_persistence::Database;
use aethercore_startup_manager::{RecommendationDecision, StartupDecision, StartupManager};
use aethercore_support_bundle::SupportBundleEngine;
use aethercore_system_repair::RepairCoordinator;
use aethercore_update_engine::{UpdateChannel, UpdateCoordinator};
use anyhow::{Context, Result};

use crate::{errors::ServiceError, protocol::*, streaming::*};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct ServiceContext {
    pub kernel: Arc<OperationKernel>,
    pub engine: Arc<OperationEngine>,
    pub driver_hub: Arc<DriverHub>,
    pub installer: Arc<DriverInstallCoordinator>,
    pub repair: Arc<RepairCoordinator>,
    pub cleaner: Arc<CleanupEngine>,
    pub startup: Arc<StartupManager>,
    pub diagnostics: Arc<DiagnosticEngine>,
    pub intelligence: Arc<DeepScanCoordinator>,
    pub db: Arc<Database>,
    pub updates: Arc<UpdateCoordinator>,
    pub support: Arc<SupportBundleEngine>,
    /// Phase 20: performance telemetry ring, bottleneck analysis, and optimization governance.
    pub performance: Arc<PerformanceEngine>,
    /// Phase 21: Timeline Intelligence — read-only coordinator over persisted history.
    pub timeline: Arc<crate::timeline::TimelineCoordinator>,
    /// Phase 22: One-Click Care — orchestration over existing domain plans only.
    pub care: Arc<crate::care::CareCoordinator>,
}

impl ServiceContext {
    pub fn handle(
        &self,
        peer: &aethercore_security::PrincipalContext,
        request_context: &RequestContext,
        req: Request,
    ) -> Response {
        handle_request(self, peer, request_context, req)
    }
}

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

    let result: std::result::Result<Option<response::Payload>, ServiceError> = (|| {
        match req.payload.ok_or("missing payload")? {
            request::Payload::Ping(_) => Ok(Some(response::Payload::Pong(v1::PongResponse {
                service_version: VERSION.into(),
            }))),
            request::Payload::GetSnapshot(_) => {
                Ok(Some(response::Payload::Snapshot(v1::SnapshotResponse {
                    snapshot: Some(snapshot(&ctx.engine, &principal_key).map_err(err)?),
                })))
            }
            request::Payload::HydrateSession(_) => {
                request_context.checkpoint().map_err(err)?;
                publish_hydration(ctx, &principal_key);
                Ok(None)
            }
            request::Payload::BeginConsentIntent(v) => {
                let intent = ctx
                    .kernel
                    .authorization()
                    .begin(&v.plan_id, &principal_key)
                    .map_err(err)?;
                let proto = consent_intent_proto(intent);
                publish(
                    ctx,
                    &principal_key,
                    EventKind::ConsentChanged,
                    &proto.plan_id,
                    Some(event_envelope::Payload::ConsentIntent(proto.clone())),
                );
                Ok(Some(response::Payload::ConsentIntent(proto)))
            }
            request::Payload::GetConsentIntent(v) => {
                require_broker(peer)?;
                let intent = ctx
                    .kernel
                    .authorization()
                    .for_broker(&v.intent_id, &principal_key)
                    .map_err(err)?;
                Ok(Some(response::Payload::ConsentIntent(
                    consent_intent_proto(intent),
                )))
            }
            request::Payload::ApproveConsentIntent(v) => {
                require_broker(peer)?;
                let (intent, approved_unix_ms) = ctx
                    .kernel
                    .authorization()
                    .approve(&v.intent_id, &principal_key, peer.pid)
                    .map_err(err)?;
                let proto = consent_intent_proto(intent.clone());
                publish(
                    ctx,
                    &principal_key,
                    EventKind::ConsentChanged,
                    &intent.plan_id,
                    Some(event_envelope::Payload::ConsentIntent(proto)),
                );
                publish_latest_plan(ctx, &principal_key);
                Ok(Some(response::Payload::ConsentApproval(
                    v1::ConsentApprovalResponse {
                        intent_id: intent.intent_id,
                        plan_id: intent.plan_id,
                        approved_unix_ms,
                    },
                )))
            }
            request::Payload::StartDriverScan(_) => {
                let lease = ctx
                    .kernel
                    .reads()
                    .try_acquire(ReadWorkload::DriverDiscovery)
                    .map_err(err)?;
                request_context.checkpoint().map_err(err)?;
                let value = ctx
                    .driver_hub
                    .start_scan_with_lease(&principal_key, lease)
                    .map_err(err)?;
                let proto = driver_hub_proto(value.clone());
                publish(
                    ctx,
                    &principal_key,
                    EventKind::DriverDiscovery,
                    "",
                    Some(event_envelope::Payload::DriverHubSnapshot(proto.clone())),
                );
                watch_driver_scan(ctx.clone(), principal_key.clone());
                Ok(Some(response::Payload::DriverHubSnapshot(
                    v1::DriverHubSnapshotResponse {
                        snapshot: Some(proto),
                    },
                )))
            }
            request::Payload::GetDriverHubSnapshot(_) => Ok(Some(
                response::Payload::DriverHubSnapshot(v1::DriverHubSnapshotResponse {
                    snapshot: Some(driver_hub_proto(
                        ctx.driver_hub
                            .snapshot_for_owner(&principal_key)
                            .map_err(err)?,
                    )),
                }),
            )),
            request::Payload::SetDriverCandidatePolicy(v) => {
                request_context.checkpoint().map_err(err)?;
                let value = ctx
                    .driver_hub
                    .set_candidate_policy(
                        &principal_key,
                        &v.scan_id,
                        v.inventory_epoch,
                        &v.candidate_id,
                        &v.policy,
                    )
                    .map_err(err)?;
                let proto = driver_hub_proto(value);
                publish(
                    ctx,
                    &principal_key,
                    EventKind::DriverDiscovery,
                    "",
                    Some(event_envelope::Payload::DriverHubSnapshot(proto.clone())),
                );
                Ok(Some(response::Payload::DriverHubSnapshot(
                    v1::DriverHubSnapshotResponse {
                        snapshot: Some(proto),
                    },
                )))
            }
            request::Payload::CreateDriverInstallPlan(v) => {
                let plan = ctx
                    .installer
                    .create_plan(
                        &principal_key,
                        &v.scan_id,
                        v.inventory_epoch,
                        &v.candidate_ids,
                    )
                    .map_err(err)?;
                let proto = plan_proto(plan);
                publish(
                    ctx,
                    &principal_key,
                    EventKind::PlanChanged,
                    &proto.id,
                    Some(event_envelope::Payload::Plan(proto.clone())),
                );
                Ok(Some(response::Payload::Plan(v1::PlanResponse {
                    plan: Some(proto),
                })))
            }
            request::Payload::StartDriverInstall(v) => {
                request_context.checkpoint().map_err(err)?;
                let lease = ctx
                    .kernel
                    .mutations()
                    .try_acquire(MutationWorkload::DriverInstall, &v.plan_id, &principal_key)
                    .map_err(err)?;
                let value = ctx
                    .installer
                    .start_with_lease(&principal_key, &v.plan_id, lease)
                    .map_err(err)?;
                let proto = install_status_proto(value);
                publish(
                    ctx,
                    &principal_key,
                    EventKind::DriverInstall,
                    &v.plan_id,
                    Some(event_envelope::Payload::DriverInstallStatus(proto.clone())),
                );
                watch_driver_install(ctx.clone(), principal_key.clone(), v.plan_id.clone());
                Ok(Some(response::Payload::DriverInstallStatus(
                    v1::DriverInstallStatusResponse {
                        status: Some(proto),
                    },
                )))
            }
            request::Payload::GetDriverInstallStatus(v) => {
                let id = (!v.plan_id.trim().is_empty()).then_some(v.plan_id.as_str());
                let status = ctx.installer.status(&principal_key, id).map_err(err)?;
                Ok(Some(response::Payload::DriverInstallStatus(
                    v1::DriverInstallStatusResponse {
                        status: status.map(install_status_proto),
                    },
                )))
            }
            request::Payload::GetRecoveryHistory(v) => {
                let entries = ctx
                    .installer
                    .recovery_history(&principal_key, v.limit.clamp(1, 100) as usize)
                    .map_err(err)?;
                Ok(Some(response::Payload::RecoveryHistory(
                    v1::RecoveryHistoryResponse {
                        entries: entries.into_iter().map(recovery_proto).collect(),
                    },
                )))
            }
            request::Payload::StartRepairAssessment(_) => {
                let lease = ctx
                    .kernel
                    .reads()
                    .try_acquire(ReadWorkload::RepairAssessment)
                    .map_err(err)?;
                let value = ctx
                    .repair
                    .start_assessment_with_lease(&principal_key, lease)
                    .map_err(err)?;
                let proto = repair_assessment_proto(value);
                publish(
                    ctx,
                    &principal_key,
                    EventKind::RepairAssessment,
                    "",
                    Some(event_envelope::Payload::RepairAssessment(proto.clone())),
                );
                watch_repair_assessment(ctx.clone(), principal_key.clone());
                Ok(Some(response::Payload::RepairAssessment(
                    v1::RepairAssessmentResponse {
                        assessment: Some(proto),
                    },
                )))
            }
            request::Payload::GetRepairAssessment(_) => Ok(Some(
                response::Payload::RepairAssessment(v1::RepairAssessmentResponse {
                    assessment: Some(repair_assessment_proto(
                        ctx.repair
                            .assessment_for_owner(&principal_key)
                            .map_err(err)?,
                    )),
                }),
            )),
            request::Payload::CreateSystemRepairPlan(v) => {
                let plan = ctx
                    .repair
                    .create_plan(&principal_key, &v.assessment_id, v.run_disk_scan)
                    .map_err(err)?;
                let proto = plan_proto(plan);
                publish(
                    ctx,
                    &principal_key,
                    EventKind::PlanChanged,
                    &proto.id,
                    Some(event_envelope::Payload::Plan(proto.clone())),
                );
                Ok(Some(response::Payload::Plan(v1::PlanResponse {
                    plan: Some(proto),
                })))
            }
            request::Payload::StartSystemRepair(v) => {
                request_context.checkpoint().map_err(err)?;
                let lease = ctx
                    .kernel
                    .mutations()
                    .try_acquire(MutationWorkload::SystemRepair, &v.plan_id, &principal_key)
                    .map_err(err)?;
                let value = ctx
                    .repair
                    .start_with_lease(&principal_key, &v.plan_id, lease)
                    .map_err(err)?;
                let proto = repair_status_proto(value);
                publish(
                    ctx,
                    &principal_key,
                    EventKind::SystemRepair,
                    &v.plan_id,
                    Some(event_envelope::Payload::SystemRepairStatus(proto.clone())),
                );
                watch_repair(ctx.clone(), principal_key.clone(), v.plan_id.clone());
                Ok(Some(response::Payload::SystemRepairStatus(
                    v1::SystemRepairStatusResponse {
                        status: Some(proto),
                    },
                )))
            }
            request::Payload::GetSystemRepairStatus(v) => {
                let id = (!v.plan_id.trim().is_empty()).then_some(v.plan_id.as_str());
                let status = ctx.repair.status(&principal_key, id).map_err(err)?;
                Ok(Some(response::Payload::SystemRepairStatus(
                    v1::SystemRepairStatusResponse {
                        status: status.map(repair_status_proto),
                    },
                )))
            }
            request::Payload::StartCleanupScan(_) => {
                let lease = ctx
                    .kernel
                    .reads()
                    .try_acquire(ReadWorkload::CleanupDiscovery)
                    .map_err(err)?;
                let value = ctx
                    .cleaner
                    .start_scan_with_lease(&principal_key, lease)
                    .map_err(err)?;
                let proto = cleanup_snapshot_proto(value);
                publish(
                    ctx,
                    &principal_key,
                    EventKind::CleanupDiscovery,
                    "",
                    Some(event_envelope::Payload::CleanupSnapshot(proto.clone())),
                );
                watch_cleanup_scan(ctx.clone(), principal_key.clone());
                Ok(Some(response::Payload::CleanupSnapshot(
                    v1::CleanupSnapshotResponse {
                        snapshot: Some(proto),
                    },
                )))
            }
            request::Payload::GetCleanupSnapshot(_) => Ok(Some(
                response::Payload::CleanupSnapshot(v1::CleanupSnapshotResponse {
                    snapshot: Some(cleanup_snapshot_proto(
                        ctx.cleaner
                            .snapshot_for_owner(&principal_key)
                            .map_err(err)?,
                    )),
                }),
            )),
            request::Payload::CreateCleanupPlan(v) => {
                let plan = ctx
                    .cleaner
                    .create_plan(
                        &principal_key,
                        &v.scan_id,
                        v.inventory_epoch,
                        &v.candidate_ids,
                    )
                    .map_err(err)?;
                let proto = plan_proto(plan);
                publish(
                    ctx,
                    &principal_key,
                    EventKind::PlanChanged,
                    &proto.id,
                    Some(event_envelope::Payload::Plan(proto.clone())),
                );
                Ok(Some(response::Payload::Plan(v1::PlanResponse {
                    plan: Some(proto),
                })))
            }
            request::Payload::StartCleanup(v) => {
                request_context.checkpoint().map_err(err)?;
                let lease = ctx
                    .kernel
                    .mutations()
                    .try_acquire(MutationWorkload::Cleanup, &v.plan_id, &principal_key)
                    .map_err(err)?;
                let value = ctx
                    .cleaner
                    .start_with_lease(&principal_key, &v.plan_id, lease)
                    .map_err(err)?;
                let proto = cleanup_status_proto(value);
                publish(
                    ctx,
                    &principal_key,
                    EventKind::CleanupExecution,
                    &v.plan_id,
                    Some(event_envelope::Payload::CleanupStatus(proto.clone())),
                );
                watch_cleanup(ctx.clone(), principal_key.clone(), v.plan_id.clone());
                Ok(Some(response::Payload::CleanupStatus(
                    v1::CleanupStatusResponse {
                        status: Some(proto),
                    },
                )))
            }
            request::Payload::GetCleanupStatus(v) => {
                let id = (!v.plan_id.trim().is_empty()).then_some(v.plan_id.as_str());
                let status = ctx.cleaner.status(&principal_key, id).map_err(err)?;
                Ok(Some(response::Payload::CleanupStatus(
                    v1::CleanupStatusResponse {
                        status: status.map(cleanup_status_proto),
                    },
                )))
            }
            request::Payload::StartStartupScan(_) => {
                let lease = ctx
                    .kernel
                    .reads()
                    .try_acquire(ReadWorkload::StartupDiscovery)
                    .map_err(err)?;
                let value = ctx
                    .startup
                    .start_scan_with_lease(&principal_key, lease)
                    .map_err(err)?;
                let proto = startup_snapshot_proto(value);
                publish(
                    ctx,
                    &principal_key,
                    EventKind::StartupDiscovery,
                    "",
                    Some(event_envelope::Payload::StartupSnapshot(proto.clone())),
                );
                watch_startup_scan(ctx.clone(), principal_key.clone());
                Ok(Some(response::Payload::StartupSnapshot(
                    v1::StartupSnapshotResponse {
                        snapshot: Some(proto),
                    },
                )))
            }
            request::Payload::GetStartupSnapshot(_) => Ok(Some(
                response::Payload::StartupSnapshot(v1::StartupSnapshotResponse {
                    snapshot: Some(startup_snapshot_proto(
                        ctx.startup
                            .snapshot_for_owner(&principal_key)
                            .map_err(err)?,
                    )),
                }),
            )),
            request::Payload::CreateStartupPlan(v) => {
                let decisions = v
                    .decisions
                    .into_iter()
                    .map(|d| {
                        let decision = parse_startup_decision(&d.decision)?;
                        Ok(StartupDecision {
                            item_id: d.item_id,
                            decision,
                        })
                    })
                    .collect::<std::result::Result<Vec<_>, String>>()?;
                let plan = ctx
                    .startup
                    .create_plan(
                        &principal_key,
                        &v.scan_id,
                        v.inventory_epoch,
                        &decisions,
                        v.confirm_service_changes,
                    )
                    .map_err(err)?;
                let proto = plan_proto(plan);
                publish(
                    ctx,
                    &principal_key,
                    EventKind::PlanChanged,
                    &proto.id,
                    Some(event_envelope::Payload::Plan(proto.clone())),
                );
                Ok(Some(response::Payload::Plan(v1::PlanResponse {
                    plan: Some(proto),
                })))
            }
            request::Payload::CreateStartupRestorePlan(v) => {
                let plan = ctx
                    .startup
                    .create_restore_plan(&principal_key, &v.change_id, v.confirm_service_changes)
                    .map_err(err)?;
                let proto = plan_proto(plan);
                publish(
                    ctx,
                    &principal_key,
                    EventKind::PlanChanged,
                    &proto.id,
                    Some(event_envelope::Payload::Plan(proto.clone())),
                );
                Ok(Some(response::Payload::Plan(v1::PlanResponse {
                    plan: Some(proto),
                })))
            }
            request::Payload::StartStartupChanges(v) => {
                request_context.checkpoint().map_err(err)?;
                let lease = ctx
                    .kernel
                    .mutations()
                    .try_acquire(MutationWorkload::Startup, &v.plan_id, &principal_key)
                    .map_err(err)?;
                let value = ctx
                    .startup
                    .start_with_lease(&principal_key, &v.plan_id, lease)
                    .map_err(err)?;
                let proto = startup_status_proto(value);
                publish(
                    ctx,
                    &principal_key,
                    EventKind::StartupExecution,
                    &v.plan_id,
                    Some(event_envelope::Payload::StartupStatus(proto.clone())),
                );
                watch_startup(ctx.clone(), principal_key.clone(), v.plan_id.clone());
                Ok(Some(response::Payload::StartupStatus(
                    v1::StartupStatusResponse {
                        status: Some(proto),
                    },
                )))
            }
            request::Payload::GetStartupStatus(v) => {
                let id = (!v.plan_id.trim().is_empty()).then_some(v.plan_id.as_str());
                let status = ctx.startup.status(&principal_key, id).map_err(err)?;
                Ok(Some(response::Payload::StartupStatus(
                    v1::StartupStatusResponse {
                        status: status.map(startup_status_proto),
                    },
                )))
            }
            request::Payload::GetStartupHistory(v) => {
                let entries = ctx
                    .startup
                    .history(&principal_key, v.limit.clamp(1, 200) as usize)
                    .map_err(err)?;
                Ok(Some(response::Payload::StartupHistory(
                    v1::StartupHistoryResponse {
                        entries: entries.into_iter().map(startup_history_proto).collect(),
                    },
                )))
            }
            request::Payload::StartDiagnosticsScan(_) => {
                let lease = ctx
                    .kernel
                    .reads()
                    .try_acquire(ReadWorkload::Diagnostics)
                    .map_err(err)?;
                let value = ctx
                    .diagnostics
                    .start_scan_with_lease(&principal_key, lease)
                    .map_err(err)?;
                let proto = diagnostics_snapshot_proto(value);
                publish(
                    ctx,
                    &principal_key,
                    EventKind::Diagnostics,
                    "",
                    Some(event_envelope::Payload::DiagnosticsSnapshot(proto.clone())),
                );
                watch_diagnostics(ctx.clone(), principal_key.clone());
                Ok(Some(response::Payload::DiagnosticsSnapshot(
                    v1::DiagnosticsSnapshotResponse {
                        snapshot: Some(proto),
                    },
                )))
            }
            request::Payload::StartDeepScan(_) => {
                request_context.checkpoint().map_err(err)?;
                let value = ctx.intelligence.start(&principal_key).map_err(err)?;
                let _ = ctx
                    .intelligence
                    .record_stream_event(&principal_key, &value.scan_id);
                let value = ctx
                    .intelligence
                    .snapshot_for_owner(&principal_key)
                    .unwrap_or(value);
                let proto = deep_scan_snapshot_proto(value);
                publish(
                    ctx,
                    &principal_key,
                    EventKind::DeepScan,
                    "",
                    Some(event_envelope::Payload::DeepScanSnapshot(proto.clone())),
                );
                watch_deep_scan(ctx.clone(), principal_key.clone());
                Ok(Some(response::Payload::DeepScanSnapshot(
                    v1::DeepScanSnapshotResponse {
                        snapshot: Some(proto),
                    },
                )))
            }
            request::Payload::CancelDeepScan(v) => {
                let value = ctx
                    .intelligence
                    .cancel(&principal_key, &v.scan_id)
                    .map_err(err)?;
                let _ = ctx
                    .intelligence
                    .record_stream_event(&principal_key, &value.scan_id);
                let value = ctx
                    .intelligence
                    .snapshot_for_owner(&principal_key)
                    .unwrap_or(value);
                let proto = deep_scan_snapshot_proto(value);
                publish(
                    ctx,
                    &principal_key,
                    EventKind::DeepScan,
                    "",
                    Some(event_envelope::Payload::DeepScanSnapshot(proto.clone())),
                );
                Ok(Some(response::Payload::DeepScanSnapshot(
                    v1::DeepScanSnapshotResponse {
                        snapshot: Some(proto),
                    },
                )))
            }
            request::Payload::GetDeepScanSnapshot(_) => {
                let value = ctx
                    .intelligence
                    .snapshot_for_owner(&principal_key)
                    .map_err(err)?;
                Ok(Some(response::Payload::DeepScanSnapshot(
                    v1::DeepScanSnapshotResponse {
                        snapshot: Some(deep_scan_snapshot_proto(value)),
                    },
                )))
            }
            request::Payload::GetDeepScanHistory(v) => {
                let entries = ctx
                    .intelligence
                    .history(&principal_key, v.limit.clamp(1, 50) as usize)
                    .map_err(err)?;
                Ok(Some(response::Payload::DeepScanHistory(
                    v1::DeepScanHistoryResponse {
                        entries: entries.into_iter().map(deep_scan_history_proto).collect(),
                    },
                )))
            }
            request::Payload::SealRemediationPlan(v) => {
                request_context.checkpoint().map_err(err)?;
                let plan = ctx
                    .intelligence
                    .seal_remediation_plan(&principal_key, &v.scan_id, &v.selected_action_ids)
                    .map_err(err)?;
                Ok(Some(response::Payload::PcRemediationPlan(
                    v1::PcRemediationPlanResponse {
                        plan: Some(remediation_plan_proto(plan)),
                    },
                )))
            }
            request::Payload::GetDiagnosticsSnapshot(_) => Ok(Some(
                response::Payload::DiagnosticsSnapshot(v1::DiagnosticsSnapshotResponse {
                    snapshot: Some(diagnostics_snapshot_proto(
                        ctx.diagnostics
                            .snapshot_for_owner(&principal_key)
                            .map_err(err)?,
                    )),
                }),
            )),
            request::Payload::GetDiagnosticsHistory(v) => {
                let entries = ctx
                    .diagnostics
                    .history(&principal_key, v.limit.clamp(1, 100) as usize)
                    .map_err(err)?;
                Ok(Some(response::Payload::DiagnosticsHistory(
                    v1::DiagnosticsHistoryResponse {
                        entries: entries.into_iter().map(diagnostic_history_proto).collect(),
                    },
                )))
            }
            request::Payload::CheckForUpdates(_) => {
                Err("legacy service-side update download is disabled".into())
            }
            request::Payload::GetUpdateSnapshot(_) => {
                ctx.updates.reap_expired_execution();
                Ok(Some(response::Payload::UpdateSnapshot(
                    v1::UpdateSnapshotResponse {
                        snapshot: Some(update_snapshot_proto(ctx.updates.snapshot(&principal_key))),
                    },
                )))
            }
            request::Payload::StageUpdate(_) => {
                Err("legacy service-side update download is disabled".into())
            }
            request::Payload::GetUpdateCheckDescriptor(v) => {
                ctx.updates.reap_expired_execution();
                let channel = update_channel_from_proto(v.channel)?;
                let descriptor = ctx.updates.check_descriptor(channel).map_err(err)?;
                Ok(Some(response::Payload::UpdateCheckDescriptor(
                    update_check_descriptor_proto(descriptor),
                )))
            }
            request::Payload::SubmitUpdateManifest(v) => {
                ctx.updates.reap_expired_execution();
                let channel = update_channel_from_proto(v.channel)?;
                let snapshot = ctx
                    .updates
                    .submit_manifest(
                        &principal_key,
                        channel,
                        &v.manifest_bytes,
                        &v.signature_bytes,
                    )
                    .map_err(err)?;
                Ok(Some(response::Payload::UpdateSnapshot(
                    v1::UpdateSnapshotResponse {
                        snapshot: Some(update_snapshot_proto(snapshot)),
                    },
                )))
            }
            request::Payload::BeginUpdateStageUpload(v) => {
                ctx.updates.reap_expired_execution();
                let descriptor = ctx
                    .updates
                    .begin_stage_upload(&principal_key, &v.release_id)
                    .map_err(err)?;
                Ok(Some(response::Payload::UpdateStageUploadDescriptor(
                    update_stage_upload_descriptor_proto(descriptor),
                )))
            }
            request::Payload::WriteUpdateStageChunk(v) => {
                ctx.updates.reap_expired_execution();
                let snapshot = ctx
                    .updates
                    .write_stage_chunk(&principal_key, &v.upload_id, v.offset, &v.data)
                    .map_err(err)?;
                Ok(Some(response::Payload::UpdateSnapshot(
                    v1::UpdateSnapshotResponse {
                        snapshot: Some(update_snapshot_proto(snapshot)),
                    },
                )))
            }
            request::Payload::FinalizeUpdateStageUpload(v) => {
                ctx.updates.reap_expired_execution();
                let snapshot = ctx
                    .updates
                    .finalize_stage_upload(&principal_key, &v.upload_id)
                    .map_err(err)?;
                Ok(Some(response::Payload::UpdateSnapshot(
                    v1::UpdateSnapshotResponse {
                        snapshot: Some(update_snapshot_proto(snapshot)),
                    },
                )))
            }
            request::Payload::CancelUpdateStageUpload(v) => {
                ctx.updates.reap_expired_execution();
                let snapshot = ctx
                    .updates
                    .cancel_stage_upload(&principal_key, &v.upload_id)
                    .map_err(err)?;
                Ok(Some(response::Payload::UpdateSnapshot(
                    v1::UpdateSnapshotResponse {
                        snapshot: Some(update_snapshot_proto(snapshot)),
                    },
                )))
            }
            request::Payload::BeginUpdateInstallIntent(v) => {
                ctx.updates.reap_expired_execution();
                let intent = ctx
                    .updates
                    .begin_install_intent(&principal_key, &v.release_id)
                    .map_err(err)?;
                Ok(Some(response::Payload::UpdateInstallIntent(
                    v1::UpdateInstallIntentResponse {
                        intent_id: intent.intent_id,
                        release: Some(update_release_proto(intent.release)),
                        expires_unix_ms: intent.expires_unix_ms,
                    },
                )))
            }
            request::Payload::GetUpdateInstallIntent(v) => {
                require_update_broker(peer)?;
                ctx.updates.reap_expired_execution();
                let intent = ctx
                    .updates
                    .intent_for_broker(&principal_key, &v.intent_id)
                    .map_err(err)?;
                Ok(Some(response::Payload::UpdateInstallIntent(
                    v1::UpdateInstallIntentResponse {
                        intent_id: intent.intent_id,
                        release: Some(update_release_proto(intent.release)),
                        expires_unix_ms: intent.expires_unix_ms,
                    },
                )))
            }
            request::Payload::ClaimUpdateInstall(v) => {
                require_update_broker(peer)?;
                ctx.updates.reap_expired_execution();
                let ticket = ctx
                    .updates
                    .claim_install(&principal_key, &v.intent_id)
                    .map_err(err)?;
                Ok(Some(response::Payload::UpdateExecutionTicket(
                    v1::UpdateExecutionTicketResponse {
                        ticket_id: ticket.ticket_id,
                        release: Some(update_release_proto(ticket.release)),
                        staged_path: ticket.staged_path.to_string_lossy().into_owned(),
                        expected_sha256: ticket.expected_sha256,
                        expected_size: ticket.expected_size,
                        expires_unix_ms: ticket.expires_unix_ms,
                    },
                )))
            }
            request::Payload::CompleteUpdateInstall(v) => {
                require_update_broker(peer)?;
                let completion = ctx
                    .updates
                    .complete_install(&principal_key, &v.ticket_id, v.exit_code)
                    .map_err(err)?;
                Ok(Some(response::Payload::UpdateCompletion(
                    v1::UpdateCompletionResponse {
                        ticket_id: completion.ticket_id,
                        release_id: completion.release_id,
                        exit_code: completion.exit_code,
                        succeeded: completion.succeeded,
                        reboot_recommended: completion.reboot_recommended,
                    },
                )))
            }
            request::Payload::CancelUpdateInstallIntent(v) => {
                let snapshot = ctx
                    .updates
                    .cancel_install_intent(&principal_key, &v.intent_id)
                    .map_err(err)?;
                Ok(Some(response::Payload::UpdateSnapshot(
                    v1::UpdateSnapshotResponse {
                        snapshot: Some(update_snapshot_proto(snapshot)),
                    },
                )))
            }
            request::Payload::CreateSupportBundlePreview(v) => {
                let sections = crate::support::build_sections(
                    ctx,
                    &principal_key,
                    v.include_hardware,
                    v.include_crash_metadata,
                    v.include_operation_history,
                    v.include_scheduler_activity,
                )?;
                let preview = ctx
                    .support
                    .create_preview(&principal_key, sections)
                    .map_err(err)?;
                publish(
                    ctx,
                    &principal_key,
                    EventKind::SupportBundle,
                    "",
                    Some(event_envelope::Payload::SupportBundle(
                        v1::SupportBundleEvent {
                            state: v1::SupportBundleEventState::PreviewCreated as i32,
                            preview_id: preview.preview_id.clone(),
                            bundle_id: String::new(),
                            size_bytes: preview.estimated_size_bytes,
                            sha256: String::new(),
                            changed_unix_ms: chrono::Utc::now().timestamp_millis(),
                        },
                    )),
                );
                Ok(Some(response::Payload::SupportBundlePreview(
                    v1::SupportBundlePreviewResponse {
                        preview: Some(support_preview_proto(preview)),
                    },
                )))
            }
            request::Payload::PrepareSupportBundle(v) => {
                let ready = ctx
                    .support
                    .prepare(&principal_key, &v.preview_id)
                    .map_err(err)?;
                publish(
                    ctx,
                    &principal_key,
                    EventKind::SupportBundle,
                    "",
                    Some(event_envelope::Payload::SupportBundle(
                        v1::SupportBundleEvent {
                            state: v1::SupportBundleEventState::Prepared as i32,
                            preview_id: v.preview_id,
                            bundle_id: ready.bundle_id.clone(),
                            size_bytes: ready.size_bytes,
                            sha256: ready.sha256.clone(),
                            changed_unix_ms: chrono::Utc::now().timestamp_millis(),
                        },
                    )),
                );
                Ok(Some(response::Payload::SupportBundleReady(
                    v1::SupportBundleReadyResponse {
                        bundle: Some(support_ready_proto(ready)),
                    },
                )))
            }
            request::Payload::ReadSupportBundleChunk(v) => {
                let chunk = ctx
                    .support
                    .read_chunk(&principal_key, &v.bundle_id, v.offset, v.max_bytes)
                    .map_err(err)?;
                Ok(Some(response::Payload::SupportBundleChunk(
                    v1::SupportBundleChunkResponse {
                        bundle_id: v.bundle_id,
                        offset: chunk.offset,
                        data: chunk.bytes,
                        eof: chunk.eof,
                        total_size: chunk.total_size,
                    },
                )))
            }
            request::Payload::DiscardSupportBundle(v) => {
                ctx.support
                    .discard(&principal_key, &v.bundle_id)
                    .map_err(err)?;
                publish(
                    ctx,
                    &principal_key,
                    EventKind::SupportBundle,
                    "",
                    Some(event_envelope::Payload::SupportBundle(
                        v1::SupportBundleEvent {
                            state: v1::SupportBundleEventState::Discarded as i32,
                            preview_id: String::new(),
                            bundle_id: v.bundle_id,
                            size_bytes: 0,
                            sha256: String::new(),
                            changed_unix_ms: chrono::Utc::now().timestamp_millis(),
                        },
                    )),
                );
                Ok(None)
            }
            request::Payload::MarkSupportBundleExported(v) => {
                let ready = ctx
                    .support
                    .ready_for_owner(&principal_key, &v.bundle_id)
                    .map_err(err)?;
                publish(
                    ctx,
                    &principal_key,
                    EventKind::SupportBundle,
                    "",
                    Some(event_envelope::Payload::SupportBundle(
                        v1::SupportBundleEvent {
                            state: v1::SupportBundleEventState::Exported as i32,
                            preview_id: String::new(),
                            bundle_id: v.bundle_id,
                            size_bytes: ready.size_bytes,
                            sha256: ready.sha256,
                            changed_unix_ms: chrono::Utc::now().timestamp_millis(),
                        },
                    )),
                );
                Ok(None)
            }
            // ---------------- Phase 20: performance intelligence ----------------
            request::Payload::StartPerfSampling(v) => {
                request_context.checkpoint().map_err(err)?;
                ctx.performance
                    .start_sampling(&principal_key, v.interval_ms)
                    .map_err(|detail| {
                        ServiceError::internal("performance", "perf.error.startSampling", detail)
                    })?;
                Ok(None)
            }
            request::Payload::StopPerfSampling(_) => {
                ctx.performance.stop_sampling();
                Ok(None)
            }
            request::Payload::GetPerformanceSnapshot(_) => {
                let interval = 1000u32;
                ctx.performance.ensure_sample(&principal_key, interval);
                let snapshot = ctx.performance.latest(&principal_key).ok_or_else(|| {
                    ServiceError::new(
                        5,
                        v1::ErrorCode::NotFound,
                        "performance",
                        "perf.error.noSamples",
                        "no performance samples available",
                        true,
                    )
                })?;
                let proto = crate::performance::perf_snapshot_proto(&snapshot);
                publish(
                    ctx,
                    &principal_key,
                    EventKind::PerformanceSnapshot,
                    "",
                    Some(event_envelope::Payload::PerformanceSnapshot(proto.clone())),
                );
                Ok(Some(response::Payload::PerformanceSnapshot(
                    v1::PerformanceSnapshotResponse {
                        snapshot: Some(proto),
                    },
                )))
            }
            request::Payload::GetBottleneckReport(_) => {
                let (report, _aggregate) =
                    ctx.performance.analyze(&principal_key).ok_or_else(|| {
                        ServiceError::new(
                            5,
                            v1::ErrorCode::NotFound,
                            "performance",
                            "perf.error.insufficientEvidence",
                            "insufficient samples for bottleneck analysis",
                            true,
                        )
                    })?;
                let proto = crate::performance::bottleneck_report_proto(&report);
                publish(
                    ctx,
                    &principal_key,
                    EventKind::BottleneckReport,
                    "",
                    Some(event_envelope::Payload::BottleneckReport(proto.clone())),
                );
                Ok(Some(response::Payload::BottleneckReport(
                    v1::BottleneckReportResponse {
                        report: Some(proto),
                    },
                )))
            }
            request::Payload::CreateOptimizationPlan(v) => {
                request_context.checkpoint().map_err(err)?;
                let (report, _aggregate) =
                    ctx.performance.analyze(&principal_key).ok_or_else(|| {
                        ServiceError::new(
                            5,
                            v1::ErrorCode::NotFound,
                            "performance",
                            "perf.error.insufficientEvidence",
                            "insufficient samples for planning",
                            true,
                        )
                    })?;
                let plan = ctx
                    .performance
                    .create_plan(&report, &v.selected_finding_ids, &Default::default())
                    .map_err(|e| {
                        ServiceError::new(
                            6,
                            v1::ErrorCode::Conflict,
                            "performance",
                            "perf.error.planning",
                            e.to_string(),
                        )
                    })?;
                let proto = crate::performance::plan_proto(&plan);
                publish(
                    ctx,
                    &principal_key,
                    EventKind::PlanChanged,
                    &proto.plan_id,
                    Some(event_envelope::Payload::OptimizationStatus(
                        v1::OptimizationStatus {
                            plan_id: proto.plan_id.clone(),
                            plan_state: "ReadyForReview".into(),
                            stage: "PlanSealed".into(),
                            progress_known: false,
                            overall_percent: 0,
                            current_candidate_id: String::new(),
                            detail: format!("digest={}", proto.digest_sha256),
                            mutation_started: false,
                            recovery_required: false,
                            failure_message: String::new(),
                            started_unix_ms: 0,
                            updated_unix_ms: chrono::Utc::now().timestamp_millis(),
                            completed_unix_ms: 0,
                            items: Vec::new(),
                        },
                    )),
                );
                Ok(Some(response::Payload::OptimizationPlan(
                    v1::OptimizationPlanSnapshotResponse { plan: Some(proto) },
                )))
            }
            request::Payload::StartOptimization(v) => {
                request_context.checkpoint().map_err(err)?;
                Err(ServiceError::forbidden(
                    "performance",
                    "perf.error.executionRequiresConsent",
                    "optimization execution is driven through the consent broker after plan review",
                ))
            }
            request::Payload::GetOptimizationStatus(_) => {
                Ok(Some(response::Payload::OptimizationStatus(
                    v1::OptimizationStatusResponse { status: None },
                )))
            }
            // ---------------- Phase 21: timeline intelligence ----------------
            request::Payload::GetTimelinePage(v) => {
                let (page, _timeline) = ctx
                    .timeline
                    .page_for_owner(&principal_key, v.page_size, v.before_sequence)
                    .map_err(err)?;
                publish(
                    ctx,
                    &principal_key,
                    EventKind::TimelinePage,
                    "",
                    Some(event_envelope::Payload::TimelinePage(page.clone())),
                );
                Ok(Some(response::Payload::TimelinePage(page)))
            }
            request::Payload::GetRecurrencePatterns(_) => {
                let (patterns, _timeline) = ctx
                    .timeline
                    .patterns_for_owner(&principal_key)
                    .map_err(err)?;
                Ok(Some(response::Payload::RecurrencePatterns(patterns)))
            }
            // ---------------- Phase 22: One-Click Care ----------------
            request::Payload::GetCareStatus(_) => {
                let status=ctx.care.plan_preview(&principal_key);
                publish(ctx,&principal_key,EventKind::CareRun,"",Some(event_envelope::Payload::CareStatus(status.clone())));
                Ok(Some(response::Payload::CareStatus(v1::CareStatusResponse{status:Some(status)})))
            }
            request::Payload::GrantCareSessionConsent(_) => {
                ctx.care.grant_session_consent(&principal_key);
                let status=ctx.care.plan_preview(&principal_key);
                publish(ctx,&principal_key,EventKind::CareRun,"",Some(event_envelope::Payload::CareStatus(status.clone())));
                Ok(Some(response::Payload::CareStatus(v1::CareStatusResponse{status:Some(status)})))
            }
            request::Payload::StartCareRun(_) => {
                let run_id=format!("care-{}", chrono::Utc::now().timestamp_millis());
                let status=ctx.care.start_run(&principal_key,&run_id).map_err(|e|ServiceError::new(6,v1::ErrorCode::Conflict,"care","care.error.startFailed",e.to_string()))?;
                publish(ctx,&principal_key,EventKind::CareRun,"",Some(event_envelope::Payload::CareStatus(status.clone())));
                Ok(Some(response::Payload::CareStatus(v1::CareStatusResponse{status:Some(status)})))
            }
            request::Payload::CancelCareRun(_) => {
                ctx.care.cancel();
                let status=ctx.care.plan_preview(&principal_key);
                publish(ctx,&principal_key,EventKind::CareRun,"",Some(event_envelope::Payload::CareStatus(status.clone())));
                Ok(Some(response::Payload::CareStatus(v1::CareStatusResponse{status:Some(status)})))
            }
        }
    })();
    match result {
        Ok(payload) => Response {
            header: Some(header),
            status_code: 0,
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

#[cfg(windows)]
fn require_broker(
    peer: &aethercore_security::PrincipalContext,
) -> std::result::Result<(), ServiceError> {
    let expected = expected_broker_path().map_err(|error| {
        ServiceError::internal(
            "authorization",
            "authorization.brokerPathUnavailable",
            error.to_string(),
        )
    })?;
    if !aethercore_security::is_expected_broker(peer, &expected) {
        return Err(ServiceError::forbidden(
            "authorization",
            "authorization.brokerRejected",
            format!(
                "consent broker rejected (pid={}, elevated={})",
                peer.pid, peer.elevated
            ),
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn expected_broker_path() -> Result<PathBuf> {
    let service = std::env::current_exe().context("resolve service executable path")?;
    let dir = service
        .parent()
        .ok_or_else(|| anyhow::anyhow!("service executable has no parent directory"))?;
    Ok(dir.join("aethercore-consent-broker.exe"))
}

#[cfg(windows)]
fn require_update_broker(
    peer: &aethercore_security::PrincipalContext,
) -> std::result::Result<(), ServiceError> {
    let expected = expected_update_broker_path().map_err(|error| {
        ServiceError::internal("update", "update.error.brokerPath", error.to_string())
    })?;
    if !aethercore_security::is_expected_broker(peer, &expected) {
        return Err(ServiceError::forbidden(
            "update",
            "update.error.brokerRejected",
            format!(
                "update broker rejected (pid={}, elevated={})",
                peer.pid, peer.elevated
            ),
        ));
    }
    Ok(())
}
#[cfg(windows)]
fn expected_update_broker_path() -> Result<PathBuf> {
    let service = std::env::current_exe().context("resolve service executable path")?;
    let dir = service
        .parent()
        .ok_or_else(|| anyhow::anyhow!("service executable has no parent directory"))?;
    Ok(dir.join("aethercore-update-broker.exe"))
}

fn err<E: Into<ServiceError>>(error: E) -> ServiceError {
    error.into()
}

fn failure(header: ResponseHeader, error: ServiceError) -> Response {
    let correlation_id = header.request_id.clone();
    let detail: String = error.detail.chars().take(512).collect();
    Response {
        header: Some(header),
        status_code: error.status,
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
