//! Local Intelligence: listing, requesting and dismissing advisory insights.

use super::*;

pub(super) fn list_insights(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let request_context = call.request_context;
    let principal_key = &call.principal_key;
    request_context.checkpoint().map_err(err)?;
    let response = ctx.intelligence_core.list(principal_key);
    publish(
        ctx,
        principal_key,
        EventKind::Insights,
        "",
        Some(event_envelope::Payload::Insights(response.clone())),
    );
    Ok(Some(response::Payload::InsightsResponse(response)))
}

pub(super) fn request_insight(call: &Call<'_>, v: v1::RequestInsightRequest) -> Routed {
    let ctx = call.ctx;
    let request_context = call.request_context;
    let principal_key = &call.principal_key;
    request_context.checkpoint().map_err(err)?;
    // Observer-effect guard: no inference while any mutation or care run holds
    // the machine-wide lease. Kernel state is the single source of truth.
    let mutation_active = ctx.kernel.mutations().is_active();
    match ctx.intelligence_core.request(
        principal_key,
        mutation_active,
        if v.question.is_empty() {
            &v.question_key
        } else {
            &v.question
        },
    ) {
        Ok(response) => {
            publish(
                ctx,
                principal_key,
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

pub(super) fn dismiss_insight(call: &Call<'_>, v: v1::DismissInsightRequest) -> Routed {
    let ctx = call.ctx;
    let request_context = call.request_context;
    let principal_key = &call.principal_key;
    request_context.checkpoint().map_err(err)?;
    // A handle that is already gone is not an error: the set is
    // session-scoped and two windows can dismiss the same insight.
    // The list below is what settles it either way.
    let _ = ctx.intelligence_core.dismiss(principal_key, &v.insight_id);
    let response = ctx.intelligence_core.list(principal_key);
    publish(
        ctx,
        principal_key,
        EventKind::Insights,
        "",
        Some(event_envelope::Payload::Insights(response.clone())),
    );
    // The direct reply used to be an unconditional empty list, which
    // reads as "you have no insights" rather than "here is what is
    // left". It now carries the same set the event does.
    Ok(Some(response::Payload::InsightsResponse(response)))
}
