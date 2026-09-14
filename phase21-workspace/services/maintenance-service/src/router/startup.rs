//! Startup items: scan, snapshot, change and restore plans, history.

use super::*;

pub(super) fn start_startup_scan(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let lease = ctx
        .kernel
        .reads()
        .try_acquire(ReadWorkload::StartupDiscovery)
        .map_err(err)?;
    let value = ctx
        .startup
        .start_scan_with_lease(principal_key, lease)
        .map_err(err)?;
    let proto = startup_snapshot_proto(value);
    publish(
        ctx,
        principal_key,
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

pub(super) fn get_startup_snapshot(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    Ok(Some(response::Payload::StartupSnapshot(
        v1::StartupSnapshotResponse {
            snapshot: Some(startup_snapshot_proto(
                ctx.startup.snapshot_for_owner(principal_key).map_err(err)?,
            )),
        },
    )))
}

pub(super) fn create_startup_plan(call: &Call<'_>, v: v1::CreateStartupPlanRequest) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
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
            principal_key,
            &v.scan_id,
            v.inventory_epoch,
            &decisions,
            v.confirm_service_changes,
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

pub(super) fn create_startup_restore_plan(
    call: &Call<'_>,
    v: v1::CreateStartupRestorePlanRequest,
) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let plan = ctx
        .startup
        .create_restore_plan(principal_key, &v.change_id, v.confirm_service_changes)
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

pub(super) fn start_startup_changes(call: &Call<'_>, v: v1::StartStartupChangesRequest) -> Routed {
    let ctx = call.ctx;
    let request_context = call.request_context;
    let principal_key = &call.principal_key;
    request_context.checkpoint().map_err(err)?;
    let lease = ctx
        .kernel
        .mutations()
        .try_acquire(MutationWorkload::Startup, &v.plan_id, principal_key)
        .map_err(err)?;
    let value = ctx
        .startup
        .start_with_lease(principal_key, &v.plan_id, lease)
        .map_err(err)?;
    let proto = startup_status_proto(value);
    publish(
        ctx,
        principal_key,
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

pub(super) fn get_startup_status(call: &Call<'_>, v: v1::GetStartupStatusRequest) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let id = (!v.plan_id.trim().is_empty()).then_some(v.plan_id.as_str());
    let status = ctx.startup.status(principal_key, id).map_err(err)?;
    Ok(Some(response::Payload::StartupStatus(
        v1::StartupStatusResponse {
            status: status.map(startup_status_proto),
        },
    )))
}

pub(super) fn get_startup_history(call: &Call<'_>, v: v1::GetStartupHistoryRequest) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let entries = ctx
        .startup
        .history(principal_key, v.limit.clamp(1, 200) as usize)
        .map_err(err)?;
    Ok(Some(response::Payload::StartupHistory(
        v1::StartupHistoryResponse {
            entries: entries.into_iter().map(startup_history_proto).collect(),
        },
    )))
}
