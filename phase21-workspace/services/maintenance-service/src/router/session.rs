//! Liveness, the composed snapshot, and renderer rehydration.

use super::*;

pub(super) fn ping() -> Routed {
    Ok(Some(response::Payload::Pong(v1::PongResponse {
        service_version: VERSION.into(),
    })))
}

pub(super) fn get_snapshot(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    Ok(Some(response::Payload::Snapshot(v1::SnapshotResponse {
        snapshot: Some(snapshot(&ctx.engine, principal_key).map_err(err)?),
    })))
}

pub(super) fn hydrate_session(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let request_context = call.request_context;
    let principal_key = &call.principal_key;
    request_context.checkpoint().map_err(err)?;
    publish_hydration(ctx, principal_key);
    Ok(None)
}
