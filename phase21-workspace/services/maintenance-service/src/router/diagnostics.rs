//! Diagnostics and the deep scan, including remediation sealing.

use super::*;

pub(super) fn start_diagnostics_scan(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let lease = ctx
        .kernel
        .reads()
        .try_acquire(ReadWorkload::Diagnostics)
        .map_err(err)?;
    let value = ctx
        .diagnostics
        .start_scan_with_lease(principal_key, lease)
        .map_err(err)?;
    let proto = diagnostics_snapshot_proto(value);
    publish(
        ctx,
        principal_key,
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

pub(super) fn start_deep_scan(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let request_context = call.request_context;
    let principal_key = &call.principal_key;
    request_context.checkpoint().map_err(err)?;
    let value = ctx.intelligence.start(principal_key).map_err(err)?;
    let _ = ctx
        .intelligence
        .record_stream_event(principal_key, &value.scan_id);
    let value = ctx
        .intelligence
        .snapshot_for_owner(principal_key)
        .unwrap_or(value);
    let proto = deep_scan_snapshot_proto(value);
    publish(
        ctx,
        principal_key,
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

pub(super) fn cancel_deep_scan(call: &Call<'_>, v: v1::CancelDeepScanRequest) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let value = ctx
        .intelligence
        .cancel(principal_key, &v.scan_id)
        .map_err(err)?;
    let _ = ctx
        .intelligence
        .record_stream_event(principal_key, &value.scan_id);
    let value = ctx
        .intelligence
        .snapshot_for_owner(principal_key)
        .unwrap_or(value);
    let proto = deep_scan_snapshot_proto(value);
    publish(
        ctx,
        principal_key,
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

pub(super) fn get_deep_scan_snapshot(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let value = ctx
        .intelligence
        .snapshot_for_owner(principal_key)
        .map_err(err)?;
    Ok(Some(response::Payload::DeepScanSnapshot(
        v1::DeepScanSnapshotResponse {
            snapshot: Some(deep_scan_snapshot_proto(value)),
        },
    )))
}

pub(super) fn get_deep_scan_history(call: &Call<'_>, v: v1::GetDeepScanHistoryRequest) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let entries = ctx
        .intelligence
        .history(principal_key, v.limit.clamp(1, 50) as usize)
        .map_err(err)?;
    Ok(Some(response::Payload::DeepScanHistory(
        v1::DeepScanHistoryResponse {
            entries: entries.into_iter().map(deep_scan_history_proto).collect(),
        },
    )))
}

pub(super) fn seal_remediation_plan(call: &Call<'_>, v: v1::SealRemediationPlanRequest) -> Routed {
    let ctx = call.ctx;
    let request_context = call.request_context;
    let principal_key = &call.principal_key;
    request_context.checkpoint().map_err(err)?;
    let plan = ctx
        .intelligence
        .seal_remediation_plan(principal_key, &v.scan_id, &v.selected_action_ids)
        .map_err(err)?;
    Ok(Some(response::Payload::PcRemediationPlan(
        v1::PcRemediationPlanResponse {
            plan: Some(remediation_plan_proto(plan)),
        },
    )))
}

pub(super) fn get_diagnostics_snapshot(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    Ok(Some(response::Payload::DiagnosticsSnapshot(
        v1::DiagnosticsSnapshotResponse {
            snapshot: Some(diagnostics_snapshot_proto(
                ctx.diagnostics
                    .snapshot_for_owner(principal_key)
                    .map_err(err)?,
            )),
        },
    )))
}

pub(super) fn get_diagnostics_history(
    call: &Call<'_>,
    v: v1::GetDiagnosticsHistoryRequest,
) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let entries = ctx
        .diagnostics
        .history(principal_key, v.limit.clamp(1, 100) as usize)
        .map_err(err)?;
    Ok(Some(response::Payload::DiagnosticsHistory(
        v1::DiagnosticsHistoryResponse {
            entries: entries.into_iter().map(diagnostic_history_proto).collect(),
        },
    )))
}
