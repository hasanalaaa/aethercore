//! Cleanup: scan, snapshot, plan, run, status.

use super::*;

pub(super) fn start_cleanup_scan(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let lease = ctx
        .kernel
        .reads()
        .try_acquire(ReadWorkload::CleanupDiscovery)
        .map_err(err)?;
    let value = ctx
        .cleaner
        .start_scan_with_lease(principal_key, lease)
        .map_err(err)?;
    let proto = cleanup_snapshot_proto(value);
    publish(
        ctx,
        principal_key,
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

pub(super) fn get_cleanup_snapshot(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    Ok(Some(response::Payload::CleanupSnapshot(
        v1::CleanupSnapshotResponse {
            snapshot: Some(cleanup_snapshot_proto(
                ctx.cleaner.snapshot_for_owner(principal_key).map_err(err)?,
            )),
        },
    )))
}

pub(super) fn create_cleanup_plan(call: &Call<'_>, v: v1::CreateCleanupPlanRequest) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let plan = ctx
        .cleaner
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

pub(super) fn start_cleanup(call: &Call<'_>, v: v1::StartCleanupRequest) -> Routed {
    let ctx = call.ctx;
    let request_context = call.request_context;
    let principal_key = &call.principal_key;
    request_context.checkpoint().map_err(err)?;
    let lease = ctx
        .kernel
        .mutations()
        .try_acquire(MutationWorkload::Cleanup, &v.plan_id, principal_key)
        .map_err(err)?;
    let value = ctx
        .cleaner
        .start_with_lease(principal_key, &v.plan_id, lease)
        .map_err(err)?;
    let proto = cleanup_status_proto(value);
    publish(
        ctx,
        principal_key,
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

pub(super) fn get_cleanup_status(call: &Call<'_>, v: v1::GetCleanupStatusRequest) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let id = (!v.plan_id.trim().is_empty()).then_some(v.plan_id.as_str());
    let status = ctx.cleaner.status(principal_key, id).map_err(err)?;
    Ok(Some(response::Payload::CleanupStatus(
        v1::CleanupStatusResponse {
            status: status.map(cleanup_status_proto),
        },
    )))
}
