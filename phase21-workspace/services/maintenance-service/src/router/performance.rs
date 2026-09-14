//! Performance sampling, bottleneck analysis and optimization governance.

use super::*;

pub(super) fn start_perf_sampling(call: &Call<'_>, v: v1::StartPerfSamplingRequest) -> Routed {
    let ctx = call.ctx;
    let request_context = call.request_context;
    let principal_key = &call.principal_key;
    request_context.checkpoint().map_err(err)?;
    ctx.performance
        .start_sampling(principal_key, v.interval_ms)
        .map_err(|detail| {
            ServiceError::internal("performance", "perf.error.startSampling", detail)
        })?;
    Ok(None)
}

pub(super) fn stop_perf_sampling(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    ctx.performance.stop_sampling();
    Ok(None)
}

pub(super) fn get_performance_snapshot(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let interval = 1000u32;
    ctx.performance.ensure_sample(principal_key, interval);
    let snapshot = ctx.performance.latest(principal_key).ok_or_else(|| {
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
        principal_key,
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

pub(super) fn get_performance_window(
    call: &Call<'_>,
    v: v1::GetPerformanceWindowRequest,
) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let interval = 1000u32;
    ctx.performance.ensure_sample(principal_key, interval);
    let response = ctx.performance.window(principal_key, v.max_samples);
    Ok(Some(response::Payload::PerformanceWindow(response)))
}

pub(super) fn get_bottleneck_report(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let (report, _aggregate) = ctx.performance.analyze(principal_key).ok_or_else(|| {
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
        principal_key,
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

pub(super) fn create_optimization_plan(
    call: &Call<'_>,
    v: v1::CreateOptimizationPlanRequest,
) -> Routed {
    let ctx = call.ctx;
    let request_context = call.request_context;
    let principal_key = &call.principal_key;
    request_context.checkpoint().map_err(err)?;
    let (report, _aggregate) = ctx.performance.analyze(principal_key).ok_or_else(|| {
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
        principal_key,
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

pub(super) fn start_optimization(call: &Call<'_>, v: v1::StartOptimizationRequest) -> Routed {
    let request_context = call.request_context;
    request_context.checkpoint().map_err(err)?;
    Err(ServiceError::forbidden(
        "performance",
        "perf.error.executionRequiresConsent",
        "optimization execution is driven through the consent broker after plan review",
    ))
}

pub(super) fn get_optimization_status() -> Routed {
    Ok(Some(response::Payload::OptimizationStatus(
        v1::OptimizationStatusResponse { status: None },
    )))
}
