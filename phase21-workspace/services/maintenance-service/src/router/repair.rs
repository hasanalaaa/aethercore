//! System repair: assessment, plan, run, status.

use super::*;

pub(super) fn start_repair_assessment(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let lease = ctx
        .kernel
        .reads()
        .try_acquire(ReadWorkload::RepairAssessment)
        .map_err(err)?;
    let value = ctx
        .repair
        .start_assessment_with_lease(principal_key, lease)
        .map_err(err)?;
    let proto = repair_assessment_proto(value);
    publish(
        ctx,
        principal_key,
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

pub(super) fn get_repair_assessment(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    Ok(Some(response::Payload::RepairAssessment(
        v1::RepairAssessmentResponse {
            assessment: Some(repair_assessment_proto(
                ctx.repair
                    .assessment_for_owner(principal_key)
                    .map_err(err)?,
            )),
        },
    )))
}

pub(super) fn create_system_repair_plan(
    call: &Call<'_>,
    v: v1::CreateSystemRepairPlanRequest,
) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let plan = ctx
        .repair
        .create_plan(principal_key, &v.assessment_id, v.run_disk_scan)
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

pub(super) fn start_system_repair(call: &Call<'_>, v: v1::StartSystemRepairRequest) -> Routed {
    let ctx = call.ctx;
    let request_context = call.request_context;
    let principal_key = &call.principal_key;
    request_context.checkpoint().map_err(err)?;
    let lease = ctx
        .kernel
        .mutations()
        .try_acquire(MutationWorkload::SystemRepair, &v.plan_id, principal_key)
        .map_err(err)?;
    let value = ctx
        .repair
        .start_with_lease(principal_key, &v.plan_id, lease)
        .map_err(err)?;
    let proto = repair_status_proto(value);
    publish(
        ctx,
        principal_key,
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

pub(super) fn get_system_repair_status(
    call: &Call<'_>,
    v: v1::GetSystemRepairStatusRequest,
) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let id = (!v.plan_id.trim().is_empty()).then_some(v.plan_id.as_str());
    let status = ctx.repair.status(principal_key, id).map_err(err)?;
    Ok(Some(response::Payload::SystemRepairStatus(
        v1::SystemRepairStatusResponse {
            status: status.map(repair_status_proto),
        },
    )))
}
