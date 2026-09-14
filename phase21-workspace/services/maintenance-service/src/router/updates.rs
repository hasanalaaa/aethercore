//! The update surface, and the elevated broker gate the install claim runs behind.

use super::*;

fn require_update_broker(
    peer: &aethercore_security::PrincipalContext,
) -> std::result::Result<(), ServiceError> {
    let expected = expected_update_broker_path().map_err(|error| {
        ServiceError::internal("update", "update.error.brokerPath", error.to_string())
    })?;
    if !aethercore_security::is_expected_broker(peer, &expected) {
        return Err(ServiceError::forbidden(
            "update",
            "update.error.brokerRejected",
            format!(
                "update broker rejected (pid={}, elevated={})",
                peer.pid, peer.elevated
            ),
        ));
    }
    Ok(())
}
fn expected_update_broker_path() -> Result<PathBuf> {
    let service = std::env::current_exe().context("resolve service executable path")?;
    let dir = service
        .parent()
        .ok_or_else(|| anyhow::anyhow!("service executable has no parent directory"))?;
    Ok(dir.join("aethercore-update-broker.exe"))
}

pub(super) fn get_update_snapshot(call: &Call<'_>) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    ctx.updates.reap_expired_execution();
    Ok(Some(response::Payload::UpdateSnapshot(
        v1::UpdateSnapshotResponse {
            snapshot: Some(update_snapshot_proto(ctx.updates.snapshot(principal_key))),
        },
    )))
}

pub(super) fn get_update_check_descriptor(
    call: &Call<'_>,
    v: v1::GetUpdateCheckDescriptorRequest,
) -> Routed {
    let ctx = call.ctx;
    ctx.updates.reap_expired_execution();
    let channel = update_channel_from_proto(v.channel)?;
    let descriptor = ctx.updates.check_descriptor(channel).map_err(err)?;
    Ok(Some(response::Payload::UpdateCheckDescriptor(
        update_check_descriptor_proto(descriptor),
    )))
}

pub(super) fn submit_update_manifest(
    call: &Call<'_>,
    v: v1::SubmitUpdateManifestRequest,
) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    ctx.updates.reap_expired_execution();
    let channel = update_channel_from_proto(v.channel)?;
    let snapshot = ctx
        .updates
        .submit_manifest(
            principal_key,
            channel,
            &v.manifest_bytes,
            &v.signature_bytes,
        )
        .map_err(err)?;
    Ok(Some(response::Payload::UpdateSnapshot(
        v1::UpdateSnapshotResponse {
            snapshot: Some(update_snapshot_proto(snapshot)),
        },
    )))
}

pub(super) fn begin_update_stage_upload(
    call: &Call<'_>,
    v: v1::BeginUpdateStageUploadRequest,
) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    ctx.updates.reap_expired_execution();
    let descriptor = ctx
        .updates
        .begin_stage_upload(principal_key, &v.release_id)
        .map_err(err)?;
    Ok(Some(response::Payload::UpdateStageUploadDescriptor(
        update_stage_upload_descriptor_proto(descriptor),
    )))
}

pub(super) fn write_update_stage_chunk(
    call: &Call<'_>,
    v: v1::WriteUpdateStageChunkRequest,
) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    ctx.updates.reap_expired_execution();
    let snapshot = ctx
        .updates
        .write_stage_chunk(principal_key, &v.upload_id, v.offset, &v.data)
        .map_err(err)?;
    Ok(Some(response::Payload::UpdateSnapshot(
        v1::UpdateSnapshotResponse {
            snapshot: Some(update_snapshot_proto(snapshot)),
        },
    )))
}

pub(super) fn finalize_update_stage_upload(
    call: &Call<'_>,
    v: v1::FinalizeUpdateStageUploadRequest,
) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    ctx.updates.reap_expired_execution();
    let snapshot = ctx
        .updates
        .finalize_stage_upload(principal_key, &v.upload_id)
        .map_err(err)?;
    Ok(Some(response::Payload::UpdateSnapshot(
        v1::UpdateSnapshotResponse {
            snapshot: Some(update_snapshot_proto(snapshot)),
        },
    )))
}

pub(super) fn cancel_update_stage_upload(
    call: &Call<'_>,
    v: v1::CancelUpdateStageUploadRequest,
) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    ctx.updates.reap_expired_execution();
    let snapshot = ctx
        .updates
        .cancel_stage_upload(principal_key, &v.upload_id)
        .map_err(err)?;
    Ok(Some(response::Payload::UpdateSnapshot(
        v1::UpdateSnapshotResponse {
            snapshot: Some(update_snapshot_proto(snapshot)),
        },
    )))
}

pub(super) fn begin_update_install_intent(
    call: &Call<'_>,
    v: v1::BeginUpdateInstallIntentRequest,
) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    ctx.updates.reap_expired_execution();
    let intent = ctx
        .updates
        .begin_install_intent(principal_key, &v.release_id)
        .map_err(err)?;
    Ok(Some(response::Payload::UpdateInstallIntent(
        v1::UpdateInstallIntentResponse {
            intent_id: intent.intent_id,
            release: Some(update_release_proto(intent.release)),
            expires_unix_ms: intent.expires_unix_ms,
        },
    )))
}

pub(super) fn get_update_install_intent(
    call: &Call<'_>,
    v: v1::GetUpdateInstallIntentRequest,
) -> Routed {
    let ctx = call.ctx;
    let peer = call.peer;
    let principal_key = &call.principal_key;
    require_update_broker(peer)?;
    ctx.updates.reap_expired_execution();
    let intent = ctx
        .updates
        .intent_for_broker(principal_key, &v.intent_id)
        .map_err(err)?;
    Ok(Some(response::Payload::UpdateInstallIntent(
        v1::UpdateInstallIntentResponse {
            intent_id: intent.intent_id,
            release: Some(update_release_proto(intent.release)),
            expires_unix_ms: intent.expires_unix_ms,
        },
    )))
}

pub(super) fn claim_update_install(call: &Call<'_>, v: v1::ClaimUpdateInstallRequest) -> Routed {
    let ctx = call.ctx;
    let peer = call.peer;
    let principal_key = &call.principal_key;
    require_update_broker(peer)?;
    ctx.updates.reap_expired_execution();
    let ticket = ctx
        .updates
        .claim_install(principal_key, &v.intent_id)
        .map_err(err)?;
    Ok(Some(response::Payload::UpdateExecutionTicket(
        v1::UpdateExecutionTicketResponse {
            ticket_id: ticket.ticket_id,
            release: Some(update_release_proto(ticket.release)),
            staged_path: ticket.staged_path.to_string_lossy().into_owned(),
            expected_sha256: ticket.expected_sha256,
            expected_size: ticket.expected_size,
            expires_unix_ms: ticket.expires_unix_ms,
        },
    )))
}

pub(super) fn complete_update_install(
    call: &Call<'_>,
    v: v1::CompleteUpdateInstallRequest,
) -> Routed {
    let ctx = call.ctx;
    let peer = call.peer;
    let principal_key = &call.principal_key;
    require_update_broker(peer)?;
    let completion = ctx
        .updates
        .complete_install(principal_key, &v.ticket_id, v.exit_code)
        .map_err(err)?;
    Ok(Some(response::Payload::UpdateCompletion(
        v1::UpdateCompletionResponse {
            ticket_id: completion.ticket_id,
            release_id: completion.release_id,
            exit_code: completion.exit_code,
            succeeded: completion.succeeded,
            reboot_recommended: completion.reboot_recommended,
        },
    )))
}

pub(super) fn cancel_update_install_intent(
    call: &Call<'_>,
    v: v1::CancelUpdateInstallIntentRequest,
) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let snapshot = ctx
        .updates
        .cancel_install_intent(principal_key, &v.intent_id)
        .map_err(err)?;
    Ok(Some(response::Payload::UpdateSnapshot(
        v1::UpdateSnapshotResponse {
            snapshot: Some(update_snapshot_proto(snapshot)),
        },
    )))
}
