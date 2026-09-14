//! Driver discovery, candidate policy, and the install plan/run surface.

use super::*;

pub(super) fn start_driver_scan(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let request_context = call.request_context;
    let principal_key = &call.principal_key;
    let lease = ctx
        .kernel
        .reads()
        .try_acquire(ReadWorkload::DriverDiscovery)
        .map_err(err)?;
    request_context.checkpoint().map_err(err)?;
    let value = ctx
        .driver_hub
        .start_scan_with_lease(principal_key, lease)
        .map_err(err)?;
    let proto = driver_hub_proto(value.clone());
    publish(
        ctx,
        principal_key,
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

pub(super) fn get_driver_hub_snapshot(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    Ok(Some(response::Payload::DriverHubSnapshot(
        v1::DriverHubSnapshotResponse {
            snapshot: Some(driver_hub_proto(
                ctx.driver_hub
                    .snapshot_for_owner(principal_key)
                    .map_err(err)?,
            )),
        },
    )))
}

pub(super) fn set_driver_candidate_policy(
    call: &Call<'_>,
    v: v1::SetDriverCandidatePolicyRequest,
) -> Routed {
    let ctx = call.ctx;
    let request_context = call.request_context;
    let principal_key = &call.principal_key;
    request_context.checkpoint().map_err(err)?;
    let value = ctx
        .driver_hub
        .set_candidate_policy(
            principal_key,
            &v.scan_id,
            v.inventory_epoch,
            &v.candidate_id,
            &v.policy,
        )
        .map_err(err)?;
    let proto = driver_hub_proto(value);
    publish(
        ctx,
        principal_key,
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

pub(super) fn create_driver_install_plan(
    call: &Call<'_>,
    v: v1::CreateDriverInstallPlanRequest,
) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let plan = ctx
        .installer
        .create_plan(
            principal_key,
            &v.scan_id,
            v.inventory_epoch,
            &v.candidate_ids,
        )
        .map_err(err)?;
    let proto = plan_proto(plan);
    publish(
        ctx,
        principal_key,
        EventKind::PlanChanged,
        &proto.id,
        Some(event_envelope::Payload::Plan(proto.clone())),
    );
    Ok(Some(response::Payload::Plan(v1::PlanResponse {
        plan: Some(proto),
    })))
}

pub(super) fn start_driver_install(call: &Call<'_>, v: v1::StartDriverInstallRequest) -> Routed {
    let ctx = call.ctx;
    let request_context = call.request_context;
    let principal_key = &call.principal_key;
    request_context.checkpoint().map_err(err)?;
    let lease = ctx
        .kernel
        .mutations()
        .try_acquire(MutationWorkload::DriverInstall, &v.plan_id, principal_key)
        .map_err(err)?;
    let value = ctx
        .installer
        .start_with_lease(principal_key, &v.plan_id, lease)
        .map_err(err)?;
    let proto = install_status_proto(value);
    publish(
        ctx,
        principal_key,
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

pub(super) fn get_driver_install_status(
    call: &Call<'_>,
    v: v1::GetDriverInstallStatusRequest,
) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let id = (!v.plan_id.trim().is_empty()).then_some(v.plan_id.as_str());
    let status = ctx.installer.status(principal_key, id).map_err(err)?;
    Ok(Some(response::Payload::DriverInstallStatus(
        v1::DriverInstallStatusResponse {
            status: status.map(install_status_proto),
        },
    )))
}

pub(super) fn get_recovery_history(call: &Call<'_>, v: v1::GetRecoveryHistoryRequest) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let entries = ctx
        .installer
        .recovery_history(principal_key, v.limit.clamp(1, 100) as usize)
        .map_err(err)?;
    Ok(Some(response::Payload::RecoveryHistory(
        v1::RecoveryHistoryResponse {
            entries: entries.into_iter().map(recovery_proto).collect(),
        },
    )))
}
