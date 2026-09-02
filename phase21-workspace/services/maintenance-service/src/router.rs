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

/// Phase 29 (T1): decodes the owner's 64-hex-char Ed25519 seed file. Returns None on
/// any malformed input — signing is then skipped and the export ships digest-only.
fn decode_key_seed(contents: &str) -> Option<[u8; 32]> {
    let hex = contents.trim();
    if hex.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    let bytes: Vec<u8> = hex
        .as_bytes()
        .chunks(2)
        .map(|c| {
            let hi = (c[0] as char).to_digit(16).ok_or(())? as u8;
            let lo = (c[1] as char).to_digit(16).ok_or(())? as u8;
            Ok::<u8, ()>((hi << 4) | lo)
        })
        .collect::<Result<Vec<u8>, ()>>()
        .ok()?;
    out.copy_from_slice(&bytes);
    Some(out)
}
use aethercore_startup_manager::{RecommendationDecision, StartupDecision, StartupManager};
use aethercore_support_bundle::SupportBundleEngine;
use aethercore_system_repair::RepairCoordinator;
use aethercore_update_engine::{UpdateChannel, UpdateCoordinator};
use anyhow::{Context, Result};

use crate::{errors::ServiceError, performance::PerformanceEngine, protocol::*, streaming::*};

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
    /// Phase 23: Embedded Local Intelligence — advisory-only, ephemeral session state.
    pub intelligence_core: Arc<crate::intelligence::IntelligenceCoordinator>,
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
                            false,
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
                let status = ctx.care.plan_preview(&principal_key);
                publish(
                    ctx,
                    &principal_key,
                    EventKind::CareRun,
                    "",
                    Some(event_envelope::Payload::CareStatus(status.clone())),
                );
                Ok(Some(response::Payload::CareStatus(
                    v1::CareStatusResponse {
                        status: Some(status),
                    },
                )))
            }
            request::Payload::GrantCareSessionConsent(_) => {
                ctx.care.grant_session_consent(&principal_key);
                let status = ctx.care.plan_preview(&principal_key);
                publish(
                    ctx,
                    &principal_key,
                    EventKind::CareRun,
                    "",
                    Some(event_envelope::Payload::CareStatus(status.clone())),
                );
                Ok(Some(response::Payload::CareStatus(
                    v1::CareStatusResponse {
                        status: Some(status),
                    },
                )))
            }
            request::Payload::StartCareRun(_) => {
                let run_id = format!("care-{}", chrono::Utc::now().timestamp_millis());
                let status = ctx.care.start_run(&principal_key, &run_id).map_err(|e| {
                    ServiceError::new(
                        6,
                        v1::ErrorCode::Conflict,
                        "care",
                        "care.error.startFailed",
                        e.to_string(),
                        false,
                    )
                })?;
                publish(
                    ctx,
                    &principal_key,
                    EventKind::CareRun,
                    "",
                    Some(event_envelope::Payload::CareStatus(status.clone())),
                );
                Ok(Some(response::Payload::CareStatus(
                    v1::CareStatusResponse {
                        status: Some(status),
                    },
                )))
            }
            request::Payload::CancelCareRun(_) => {
                ctx.care.cancel();
                let status = ctx.care.plan_preview(&principal_key);
                publish(
                    ctx,
                    &principal_key,
                    EventKind::CareRun,
                    "",
                    Some(event_envelope::Payload::CareStatus(status.clone())),
                );
                Ok(Some(response::Payload::CareStatus(
                    v1::CareStatusResponse {
                        status: Some(status),
                    },
                )))
            }
            // ---------------- Phase 23: Local Intelligence (advisory-only) ----------------
            request::Payload::ListInsights(_) => {
                request_context.checkpoint().map_err(err)?;
                let response = ctx.intelligence_core.list();
                publish(
                    ctx,
                    &principal_key,
                    EventKind::Insights,
                    "",
                    Some(event_envelope::Payload::Insights(response.clone())),
                );
                Ok(Some(response::Payload::InsightsResponse(response)))
            }
            request::Payload::RequestInsight(_) => {
                request_context.checkpoint().map_err(err)?;
                // Observer-effect guard: no inference while any mutation or care run holds
                // the machine-wide lease. Kernel state is the single source of truth.
                let mutation_active = ctx.kernel.mutations().is_active();
                match ctx
                    .intelligence_core
                    .request(&principal_key, mutation_active)
                {
                    Ok(response) => {
                        publish(
                            ctx,
                            &principal_key,
                            EventKind::Insights,
                            "",
                            Some(event_envelope::Payload::Insights(response.clone())),
                        );
                        Ok(Some(response::Payload::InsightsResponse(response)))
                    }
                    Err(e) => Err(ServiceError::new(
                        6,
                        v1::ErrorCode::Busy,
                        "intelligence",
                        "insight.error.busy",
                        e,
                        false,
                    )),
                }
            }
            request::Payload::DismissInsight(v) => {
                request_context.checkpoint().map_err(err)?;
                let _ = ctx.intelligence_core.dismiss(&v.insight_id);
                let response = ctx.intelligence_core.list();
                publish(
                    ctx,
                    &principal_key,
                    EventKind::Insights,
                    "",
                    Some(event_envelope::Payload::Insights(response)),
                );
                Ok(Some(response::Payload::InsightsResponse(
                    v1::InsightsResponse {
                        engine_label: ctx.intelligence_core.engine_label().into(),
                        insights: Vec::new(),
                    },
                )))
            }
            // ---------------- Phase 26/27: honest platform + engine surface ----------
            request::Payload::GetPlatformCapabilities(_) => {
                let capabilities = aethercore_platform_capabilities::matrix_for_current_platform_observed(
                    crate::performance::observe_telemetry(),
                )
                    .into_iter()
                    .map(|(name, availability)| {
                        let state = match &availability {
                            aethercore_platform_capabilities::Availability::Native => "native",
                            aethercore_platform_capabilities::Availability::Degraded { .. } => {
                                "degraded"
                            }
                            aethercore_platform_capabilities::Availability::NotAvailable {
                                ..
                            } => "notAvailable",
                        };
                        let key = match availability {
                            aethercore_platform_capabilities::Availability::Native => String::new(),
                            aethercore_platform_capabilities::Availability::Degraded {
                                note_key,
                            } => note_key.to_string(),
                            aethercore_platform_capabilities::Availability::NotAvailable {
                                reason_key,
                            } => reason_key.to_string(),
                        };
                        v1::PlatformCapabilityStatus {
                            name: name.to_string(),
                            availability: Some(v1::CapabilityAvailability {
                                state: state.to_string(),
                                key,
                            }),
                        }
                    })
                    .collect();
                Ok(Some(response::Payload::PlatformCapabilitiesResponse(
                    v1::PlatformCapabilitiesResponse {
                        platform: aethercore_platform_capabilities::current_platform_name().to_string(),
                        capabilities,
                    },
                )))
            }
            request::Payload::GetEngineSource(_) => Ok(Some(
                response::Payload::EngineSourceResponse(v1::EngineSourceResponse {
                    source: crate::performance::engine_source().to_string(),
                    platform: aethercore_platform_capabilities::current_platform_name().to_string(),
                }),
            )),
            // ---------------- Phase 29 (T1): signed journal export -------------------
            request::Payload::ExportJournal(v) => {
                // Read-only RPC: pulls through EXISTING persistence accessors only;
                // single-writer discipline untouched.
                // `operations.proto` documents empty as "the calling principal's own
                // records". A NON-empty value is a request to read someone else's, and
                // no authorization exists in this product that grants that: there is no
                // administrative scope, no delegation, no broker gate for it. Honouring
                // it let any caller holding another binding key read that owner's
                // executions, support journal and repair timeline out of a LocalSystem
                // service. The only correct answer is to refuse.
                let owner = if v.owner_principal_key.trim().is_empty()
                    || v.owner_principal_key == principal_key
                {
                    principal_key.clone()
                } else {
                    return Err(ServiceError::forbidden(
                        "persistence",
                        "journal.ownerScopeForbidden",
                        "owner_principal_key must be empty or equal to the calling principal",
                    ));
                };
                let executions = ctx
                    .db
                    .maintenance_executions_for_owner(&owner, 2000)
                    .map_err(err)?;
                let events = ctx
                    .db
                    .support_journal_events_for_owner(&owner, 500)
                    .map_err(err)?;
                let timeline = ctx
                    .db
                    .repair_timeline_events_for_owner(&owner, 2000)
                    .map_err(err)?;
                let mut records: Vec<(String, i64, serde_json::Value)> = Vec::new();
                for e in &executions {
                    records.push((
                        "maintenance_execution".to_string(),
                        e.updated_unix_ms,
                        serde_json::json!({
                            "planId": e.plan_id,
                            "domain": e.domain,
                            "stage": e.stage,
                            "overallPercent": e.overall_percent,
                            "outcome": e.outcome,
                            "startedUnixMs": e.started_unix_ms,
                            "completedUnixMs": e.completed_unix_ms,
                        }),
                    ));
                }
                for e in &events {
                    records.push((
                        "plan_event".to_string(),
                        e.created_unix_ms,
                        serde_json::json!({
                            "seq": e.seq,
                            "planId": e.plan_id,
                            "fromState": e.from_state,
                            "toState": e.to_state,
                            "eventKind": e.event_kind,
                            "detail": e.detail,
                        }),
                    ));
                }
                for t in &timeline {
                    records.push((
                        "repair_timeline".to_string(),
                        t.created_unix_ms,
                        serde_json::json!({
                            "eventId": t.event_id,
                            "planId": t.plan_id,
                            "domain": t.domain,
                            "actionId": t.action_id,
                            "eventKind": t.event_kind,
                            "outcome": t.outcome,
                        }),
                    ));
                }
                let now = chrono::Utc::now().timestamp_millis();
                let to_bound = if v.to_unix_ms > 0 {
                    v.to_unix_ms
                } else {
                    i64::MAX
                };
                let from_bound = v.from_unix_ms;
                records.retain(|(_, ordinal, _)| *ordinal >= from_bound && *ordinal < to_bound);
                // source_db_fingerprint: hash of the record count + latest ordinal per
                // class (deterministic over the same data set without reading the db file).
                let mut fp = sha2::Sha256::new();
                use sha2::Digest as _;
                fp.update(records.len().to_le_bytes());
                for (kind, ordinal, _) in &records {
                    fp.update(kind.as_bytes());
                    fp.update(ordinal.to_le_bytes());
                }
                let fingerprint = format!("{:x}", fp.finalize());
                // Phase 31 (W9): the request id IS the correlation id for this export;
                // it is clamped typed by CorrelationId::parse inside the crate.
                let correlation_id = aethercore_persistence::export::CorrelationId::parse(
                    &principal_key,
                )
                .or_else(|| {
                    aethercore_persistence::export::CorrelationId::parse(&format!("export-{now}"))
                });
                let mut envelope = aethercore_persistence::export::build_envelope_with_correlation(
                    records,
                    now,
                    fingerprint,
                    correlation_id,
                );
                // Signing happens ONLY with an explicitly provisioned owner key file
                // pointed at by AETHERCORE_EXPORT_KEY; otherwise digest-only honesty.
                if let Some(key_path) = std::env::var_os("AETHERCORE_EXPORT_KEY") {
                    if let Ok(key_hex) = std::fs::read_to_string(std::path::Path::new(&key_path)) {
                        let key_hex = key_hex.trim();
                        if let Some(seed) = decode_key_seed(key_hex) {
                            let signing =
                                aethercore_persistence::export::signing_key_from_seed(&seed);
                            aethercore_persistence::export::sign_envelope(&mut envelope, &signing);
                        }
                    }
                }
                let response_payload = v1::ExportJournalResponse {
                    envelope_json: serde_json::to_vec(&envelope).unwrap_or_default(),
                    record_count: envelope.records.len() as u64,
                    signed: envelope.signed,
                };
                aethercore_diagnostics::emit_structured(
                    "info",
                    aethercore_diagnostics::events::EXPORT_PRODUCED,
                    serde_json::json!({
                        "records": response_payload.record_count,
                        "signed": response_payload.signed,
                    }),
                );
                Ok(Some(response::Payload::ExportJournalResponse(
                    response_payload,
                )))
            }
            // ---------------- Phase 32 (T1): read-only security audit ----------------
            request::Payload::RunSecurityAudit(v) => {
                // Read-only RPC over the aethercore-security-audit domain: zero
                // system mutations, zero network. It is NOT zero-privilege — the
                // Windows host runs as LocalSystem and reverts impersonation before
                // dispatch, so a caller-named path is read with the service's authority
                // and not the caller's. Targets are therefore confined to the roots the
                // OS reported for THIS caller's own token, which is the owner-scoped
                // allowlist `operations.proto` has always promised here. Refusal is
                // typed and happens before any provider runs.
                let parsed: Result<Vec<aethercore_security_audit::model::AuditTarget>, String> =
                    serde_json::from_slice::<Vec<serde_json::Value>>(&v.targets_json)
                        .map_err(|e| format!("targets parse: {e}"))
                        .and_then(|vals| {
                            vals.into_iter()
                                .map(|val| {
                                    serde_json::from_value::<
                                        aethercore_security_audit::model::AuditTarget,
                                    >(val)
                                    .map_err(|e| format!("target decode: {e}"))
                                })
                                .collect()
                        });
                let targets = match parsed {
                    Ok(t) if !t.is_empty() => t,
                    Ok(_) => {
                        // Typed rejection BEFORE any provider runs.
                        return Err(ServiceError::invalid(
                            "security",
                            "sec.invalidTargets",
                            "targets_json must be a non-empty AuditTarget array",
                        ));
                    }
                    Err(detail) => {
                        return Err(ServiceError::invalid(
                            "security",
                            "sec.invalidTargets",
                            detail,
                        ));
                    }
                };
                let scope = aethercore_security_audit::OwnerScope::new(peer.owner_roots());
                if let Err(denial) =
                    aethercore_security_audit::authorize_targets(&targets, &scope)
                {
                    return Err(ServiceError::forbidden(
                        "security",
                        denial.code(),
                        denial.to_string(),
                    ));
                }
                let report = aethercore_security_audit::run_audit(&targets);
                let lanes = report
                    .lanes
                    .iter()
                    .map(|l| v1::SecurityAuditReportLane {
                        lane: l.lane.clone(),
                        status: match l.status {
                            aethercore_security_audit::LaneStatus::Ok { .. } => "ok".to_string(),
                            aethercore_security_audit::LaneStatus::NotAvailable { .. } => {
                                "notAvailable".to_string()
                            }
                        },
                        reason: match &l.status {
                            aethercore_security_audit::LaneStatus::NotAvailable { reason } => {
                                reason.clone()
                            }
                            _ => String::new(),
                        },
                        finding_count: l.findings.len() as u64,
                        findings_json: serde_json::to_vec(&l.findings).unwrap_or_default(),
                    })
                    .collect();
                aethercore_diagnostics::emit_structured(
                    "info",
                    aethercore_diagnostics::events::SECURITY_AUDIT_RAN,
                    serde_json::json!({
                        "digest": report.digest,
                        "lanes": report.lanes.len(),
                    }),
                );
                publish(
                    ctx,
                    &principal_key,
                    EventKind::SecurityAudit,
                    "",
                    Some(event_envelope::Payload::SecurityAudit(
                        v1::SecurityAuditResponse {
                            schema_version: report.schema_version,
                            platform: report.platform.clone(),
                            digest: report.digest.clone(),
                            lanes,
                        },
                    )),
                );
                Ok(Some(response::Payload::SecurityAuditResponse(
                    v1::SecurityAuditResponse {
                        schema_version: report.schema_version,
                        platform: report.platform.clone(),
                        digest: report.digest,
                        lanes: report
                            .lanes
                            .iter()
                            .map(|l| v1::SecurityAuditReportLane {
                                lane: l.lane.clone(),
                                status: match l.status {
                                    aethercore_security_audit::LaneStatus::Ok { .. } => {
                                        "ok".to_string()
                                    }
                                    aethercore_security_audit::LaneStatus::NotAvailable {
                                        ..
                                    } => "notAvailable".to_string(),
                                },
                                reason: match &l.status {
                                    aethercore_security_audit::LaneStatus::NotAvailable {
                                        reason,
                                    } => reason.clone(),
                                    _ => String::new(),
                                },
                                finding_count: l.findings.len() as u64,
                                findings_json: serde_json::to_vec(&l.findings).unwrap_or_default(),
                            })
                            .collect(),
                    },
                )))
            }
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

// P36 (Hermes): broker authorization gates are platform-neutral trust checks over
// PrincipalContext; the named-pipe transport enforces the same broker contract.
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

fn expected_broker_path() -> Result<PathBuf> {
    let service = std::env::current_exe().context("resolve service executable path")?;
    let dir = service
        .parent()
        .ok_or_else(|| anyhow::anyhow!("service executable has no parent directory"))?;
    Ok(dir.join("aethercore-consent-broker.exe"))
}

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
