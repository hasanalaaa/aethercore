//! Timeline Intelligence: read-only pages and recurrence patterns.

use super::*;

pub(super) fn get_timeline_page(call: &Call<'_>, v: v1::GetTimelinePageRequest) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let (page, _timeline) = ctx
        .timeline
        .page_for_owner(principal_key, v.page_size, v.before_sequence)
        .map_err(err)?;
    publish(
        ctx,
        principal_key,
        EventKind::TimelinePage,
        "",
        Some(event_envelope::Payload::TimelinePage(page.clone())),
    );
    Ok(Some(response::Payload::TimelinePage(page)))
}

pub(super) fn get_recurrence_patterns(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let (patterns, _timeline) = ctx
        .timeline
        .patterns_for_owner(principal_key)
        .map_err(err)?;
    Ok(Some(response::Payload::RecurrencePatterns(patterns)))
}
