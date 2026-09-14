//! The consent intent lifecycle, and the broker trust gate it runs behind.

use super::*;

// P36 (Hermes): broker authorization gates are platform-neutral trust checks over
// PrincipalContext; the named-pipe transport enforces the same broker contract.
fn require_broker(
    peer: &aethercore_security::PrincipalContext,
) -> std::result::Result<(), ServiceError> {
    let expected = expected_broker_path().map_err(|error| {
        ServiceError::internal(
            "authorization",
            "authorization.brokerPathUnavailable",
            error.to_string(),
        )
    })?;
    if !aethercore_security::is_expected_broker(peer, &expected) {
        return Err(ServiceError::forbidden(
            "authorization",
            "authorization.brokerRejected",
            format!(
                "consent broker rejected (pid={}, elevated={})",
                peer.pid, peer.elevated
            ),
        ));
    }
    Ok(())
}

fn expected_broker_path() -> Result<PathBuf> {
    let service = std::env::current_exe().context("resolve service executable path")?;
    let dir = service
        .parent()
        .ok_or_else(|| anyhow::anyhow!("service executable has no parent directory"))?;
    Ok(dir.join("aethercore-consent-broker.exe"))
}

pub(super) fn begin_consent_intent(call: &Call<'_>, v: v1::BeginConsentIntentRequest) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let intent = ctx
        .kernel
        .authorization()
        .begin(&v.plan_id, principal_key)
        .map_err(err)?;
    let proto = consent_intent_proto(intent);
    publish(
        ctx,
        principal_key,
        EventKind::ConsentChanged,
        &proto.plan_id,
        Some(event_envelope::Payload::ConsentIntent(proto.clone())),
    );
    Ok(Some(response::Payload::ConsentIntent(proto)))
}

pub(super) fn get_consent_intent(call: &Call<'_>, v: v1::GetConsentIntentRequest) -> Routed {
    let ctx = call.ctx;
    let peer = call.peer;
    let principal_key = &call.principal_key;
    require_broker(peer)?;
    let intent = ctx
        .kernel
        .authorization()
        .for_broker(&v.intent_id, principal_key)
        .map_err(err)?;
    Ok(Some(response::Payload::ConsentIntent(
        consent_intent_proto(intent),
    )))
}

pub(super) fn approve_consent_intent(
    call: &Call<'_>,
    v: v1::ApproveConsentIntentRequest,
) -> Routed {
    let ctx = call.ctx;
    let peer = call.peer;
    let principal_key = &call.principal_key;
    require_broker(peer)?;
    let (intent, approved_unix_ms) = ctx
        .kernel
        .authorization()
        .approve(&v.intent_id, principal_key, peer.pid)
        .map_err(err)?;
    let proto = consent_intent_proto(intent.clone());
    publish(
        ctx,
        principal_key,
        EventKind::ConsentChanged,
        &intent.plan_id,
        Some(event_envelope::Payload::ConsentIntent(proto)),
    );
    publish_latest_plan(ctx, principal_key);
    Ok(Some(response::Payload::ConsentApproval(
        v1::ConsentApprovalResponse {
            intent_id: intent.intent_id,
            plan_id: intent.plan_id,
            approved_unix_ms,
        },
    )))
}
