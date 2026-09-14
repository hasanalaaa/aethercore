//! Support bundles: preview, prepare, chunked read, discard, export.

use super::*;

pub(super) fn create_support_bundle_preview(
    call: &Call<'_>,
    v: v1::CreateSupportBundlePreviewRequest,
) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let sections = crate::support::build_sections(
        ctx,
        principal_key,
        v.include_hardware,
        v.include_crash_metadata,
        v.include_operation_history,
        v.include_scheduler_activity,
    )?;
    let preview = ctx
        .support
        .create_preview(principal_key, sections)
        .map_err(err)?;
    publish(
        ctx,
        principal_key,
        EventKind::SupportBundle,
        "",
        Some(event_envelope::Payload::SupportBundle(
            v1::SupportBundleEvent {
                state: v1::SupportBundleEventState::PreviewCreated as i32,
                preview_id: preview.preview_id.clone(),
                bundle_id: String::new(),
                size_bytes: preview.estimated_size_bytes,
                sha256: String::new(),
                changed_unix_ms: chrono::Utc::now().timestamp_millis(),
            },
        )),
    );
    Ok(Some(response::Payload::SupportBundlePreview(
        v1::SupportBundlePreviewResponse {
            preview: Some(support_preview_proto(preview)),
        },
    )))
}

pub(super) fn prepare_support_bundle(
    call: &Call<'_>,
    v: v1::PrepareSupportBundleRequest,
) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let ready = ctx
        .support
        .prepare(principal_key, &v.preview_id)
        .map_err(err)?;
    publish(
        ctx,
        principal_key,
        EventKind::SupportBundle,
        "",
        Some(event_envelope::Payload::SupportBundle(
            v1::SupportBundleEvent {
                state: v1::SupportBundleEventState::Prepared as i32,
                preview_id: v.preview_id,
                bundle_id: ready.bundle_id.clone(),
                size_bytes: ready.size_bytes,
                sha256: ready.sha256.clone(),
                changed_unix_ms: chrono::Utc::now().timestamp_millis(),
            },
        )),
    );
    Ok(Some(response::Payload::SupportBundleReady(
        v1::SupportBundleReadyResponse {
            bundle: Some(support_ready_proto(ready)),
        },
    )))
}

pub(super) fn read_support_bundle_chunk(
    call: &Call<'_>,
    v: v1::ReadSupportBundleChunkRequest,
) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let chunk = ctx
        .support
        .read_chunk(principal_key, &v.bundle_id, v.offset, v.max_bytes)
        .map_err(err)?;
    Ok(Some(response::Payload::SupportBundleChunk(
        v1::SupportBundleChunkResponse {
            bundle_id: v.bundle_id,
            offset: chunk.offset,
            data: chunk.bytes,
            eof: chunk.eof,
            total_size: chunk.total_size,
        },
    )))
}

pub(super) fn discard_support_bundle(
    call: &Call<'_>,
    v: v1::DiscardSupportBundleRequest,
) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    ctx.support
        .discard(principal_key, &v.bundle_id)
        .map_err(err)?;
    publish(
        ctx,
        principal_key,
        EventKind::SupportBundle,
        "",
        Some(event_envelope::Payload::SupportBundle(
            v1::SupportBundleEvent {
                state: v1::SupportBundleEventState::Discarded as i32,
                preview_id: String::new(),
                bundle_id: v.bundle_id,
                size_bytes: 0,
                sha256: String::new(),
                changed_unix_ms: chrono::Utc::now().timestamp_millis(),
            },
        )),
    );
    Ok(None)
}

pub(super) fn mark_support_bundle_exported(
    call: &Call<'_>,
    v: v1::MarkSupportBundleExportedRequest,
) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    let ready = ctx
        .support
        .ready_for_owner(principal_key, &v.bundle_id)
        .map_err(err)?;
    publish(
        ctx,
        principal_key,
        EventKind::SupportBundle,
        "",
        Some(event_envelope::Payload::SupportBundle(
            v1::SupportBundleEvent {
                state: v1::SupportBundleEventState::Exported as i32,
                preview_id: String::new(),
                bundle_id: v.bundle_id,
                size_bytes: ready.size_bytes,
                sha256: ready.sha256,
                changed_unix_ms: chrono::Utc::now().timestamp_millis(),
            },
        )),
    );
    Ok(None)
}
