//! One-Click Care, and the error mapping that keeps an unreadable plan a 500.

use super::*;

/// DBT-P46-B33: "the plans could not be read" is not a conflict and must not
/// be reported as one — the request failed because the service could not tell
/// what is due, which is a 500 the caller can distinguish from a refusal.
fn care_err(error: aethercore_care_orchestrator::CareError) -> ServiceError {
    match error {
        aethercore_care_orchestrator::CareError::SourceLimit => ServiceError::new(
            429,
            v1::ErrorCode::Busy,
            "care",
            "care.error.sourceLimit",
            error.to_string(),
            false,
        ),
        aethercore_care_orchestrator::CareError::PlanSourcesUnavailable(_) => {
            ServiceError::internal(
                "care",
                "care.error.planSourcesUnavailable",
                error.to_string(),
            )
        }
        // The plan changed between what the owner was shown and what would be approved.
        aethercore_care_orchestrator::CareError::DigestChanged => ServiceError::new(
            6,
            v1::ErrorCode::Conflict,
            "care",
            "care.error.planChanged",
            error.to_string(),
            false,
        ),
        other => ServiceError::new(
            6,
            v1::ErrorCode::Conflict,
            "care",
            "care.error.startFailed",
            other.to_string(),
            false,
        ),
    }
}

pub(super) fn get_care_status(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let status = ctx.care.plan_preview(principal_key).map_err(care_err)?;
    publish(
        ctx,
        principal_key,
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

/// P79-04A (D5): prepares what care could run and says why when it cannot. It reads and prepares
/// only - the cleanup scan is local, the plan is unapproved, nothing is granted or deleted.
pub(super) fn prepare_care_preview(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64);
    let start_scan = || -> Result<(), String> {
        let lease = ctx
            .kernel
            .reads()
            .try_acquire(ReadWorkload::CleanupDiscovery)
            .map_err(|error| error.to_string())?;
        let value = ctx
            .cleaner
            .start_scan_with_lease(principal_key, lease)
            .map_err(|error| error.to_string())?;
        publish(
            ctx,
            principal_key,
            EventKind::CleanupDiscovery,
            "",
            Some(event_envelope::Payload::CleanupSnapshot(
                cleanup_snapshot_proto(value),
            )),
        );
        watch_cleanup_scan(ctx.clone(), principal_key.clone());
        Ok(())
    };
    let prepared = crate::care::prepare_preview(
        &ctx.cleaner,
        &|| ctx.care.plan_preview(principal_key),
        principal_key,
        now_ms,
        &start_scan,
    )
    .map_err(care_err)?;
    publish(
        ctx,
        principal_key,
        EventKind::CareRun,
        "",
        Some(event_envelope::Payload::CareStatus(prepared.status.clone())),
    );
    Ok(Some(response::Payload::CarePreview(
        v1::CarePreviewResponse {
            status: Some(prepared.status),
            domains: prepared.domains,
        },
    )))
}

pub(super) fn grant_consent(call: &Call<'_>, v: v1::GrantCareSessionConsentRequest) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let status = ctx
        .care
        .grant_session_consent(principal_key, &v.plan_digest_sha256)
        .map_err(care_err)?;
    publish(
        ctx,
        principal_key,
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

pub(super) fn start_care_run(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let run_id = format!("care-{}", chrono::Utc::now().timestamp_millis());
    let status = ctx
        .care
        .start_run(principal_key, &run_id)
        .map_err(care_err)?;
    publish(
        ctx,
        principal_key,
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

pub(super) fn cancel_care_run(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    ctx.care.cancel();
    let status = ctx.care.plan_preview(principal_key).map_err(care_err)?;
    publish(
        ctx,
        principal_key,
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
