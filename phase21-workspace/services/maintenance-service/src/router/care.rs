//! One-Click Care, and the error mapping that keeps an unreadable plan a 500.

use super::*;

/// DBT-P46-B33: "the plans could not be read" is not a conflict and must not
/// be reported as one — the request failed because the service could not tell
/// what is due, which is a 500 the caller can distinguish from a refusal.
fn care_err(error: aethercore_care_orchestrator::CareError) -> ServiceError {
    match error {
        aethercore_care_orchestrator::CareError::PlanSourcesUnavailable(_) => {
            ServiceError::internal(
                "care",
                "care.error.planSourcesUnavailable",
                error.to_string(),
            )
        }
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

pub(super) fn grant_care_session_consent(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let status = ctx
        .care
        .grant_session_consent(principal_key)
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
