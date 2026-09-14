//! The grounded assistant: ask, cancel, and read the evidence pack.

use super::*;

pub(super) fn ask_assistant(call: &Call<'_>, v: v1::AskAssistantRequest) -> Routed {
    let ctx = call.ctx;
    let request_context = call.request_context;
    let principal_key = &call.principal_key;
    request_context.checkpoint().map_err(err)?;
    // Validated at the boundary, trusted internally. A turn id is a
    // handle the client will send back to cancel, so it is checked
    // for shape before it is ever used as a map key.
    if let Err(key) = crate::assistant::validate(&v.turn_id, &v.question) {
        return Err(ServiceError::invalid("assistant", key, key));
    }
    // Observer-effect guard: no inference while any mutation or care
    // run holds the machine-wide lease. Kernel state is the single
    // source of truth, exactly as the insight path reads it.
    let mutation_active = ctx.kernel.mutations().is_active();
    let turn = ctx.assistant.ask(
        principal_key,
        &v.turn_id,
        &v.question,
        mutation_active,
        ctx.kernel.events().clone(),
    );
    publish(
        ctx,
        principal_key,
        EventKind::AssistantTurn,
        "",
        Some(event_envelope::Payload::AssistantTurn(turn.clone())),
    );
    Ok(Some(response::Payload::AssistantTurn(
        v1::AssistantTurnResponse { turn: Some(turn) },
    )))
}

pub(super) fn cancel_assistant_turn(call: &Call<'_>, v: v1::CancelAssistantTurnRequest) -> Routed {
    let ctx = call.ctx;
    let request_context = call.request_context;
    let principal_key = &call.principal_key;
    request_context.checkpoint().map_err(err)?;
    if v.turn_id.is_empty() {
        return Err(ServiceError::invalid(
            "assistant",
            "assistant.invalid.turnId",
            "assistant.invalid.turnId",
        ));
    }
    // A turn that already finished cannot be cancelled, and that is a
    // STATE rather than a fault: two windows can press Escape on the
    // same turn, and the terminal envelope on the stream settles it
    // either way. The reply reports what was found.
    let in_flight = ctx.assistant.cancel(principal_key, &v.turn_id);
    Ok(Some(response::Payload::AssistantTurn(
        v1::AssistantTurnResponse {
            turn: Some(v1::AssistantTurn {
                turn_id: v.turn_id,
                schema_version: aethercore_intelligence_core::ASSISTANT_SCHEMA_V1,
                state: if in_flight {
                    v1::AssistantTurnState::Streaming as i32
                } else {
                    v1::AssistantTurnState::Cancelled as i32
                },
                engine_label: ctx.assistant.engine_label().into(),
                ..Default::default()
            }),
        },
    )))
}

pub(super) fn get_assistant_pack(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let request_context = call.request_context;
    let principal_key = &call.principal_key;
    request_context.checkpoint().map_err(err)?;
    let pack = ctx.assistant.evidence_pack(principal_key);
    Ok(Some(response::Payload::AssistantPack(
        v1::AssistantPackResponse {
            pack: crate::assistant::evidence_refs(&pack),
            engine_label: ctx.assistant.engine_label().into(),
        },
    )))
}
