//! The signed journal export, and the owner key seed it is signed with.

use super::*;

/// Phase 29 (T1): decodes the owner's 64-hex-char Ed25519 seed file. Returns None on
/// any malformed input — signing is then skipped and the export ships digest-only.
fn decode_key_seed(contents: &str) -> Option<[u8; 32]> {
    let hex = contents.trim();
    if hex.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    let bytes: Vec<u8> = hex
        .as_bytes()
        .chunks(2)
        .map(|c| {
            let hi = (c[0] as char).to_digit(16).ok_or(())? as u8;
            let lo = (c[1] as char).to_digit(16).ok_or(())? as u8;
            Ok::<u8, ()>((hi << 4) | lo)
        })
        .collect::<Result<Vec<u8>, ()>>()
        .ok()?;
    out.copy_from_slice(&bytes);
    Some(out)
}

pub(super) fn export_journal(call: &Call<'_>, v: v1::ExportJournalRequest) -> Routed {
    let ctx = call.ctx;
    let principal_key = &call.principal_key;
    // Read-only RPC: pulls through EXISTING persistence accessors only;
    // single-writer discipline untouched.
    // `operations.proto` documents empty as "the calling principal's own
    // records". A NON-empty value is a request to read someone else's, and
    // no authorization exists in this product that grants that: there is no
    // administrative scope, no delegation, no broker gate for it. Honouring
    // it let any caller holding another binding key read that owner's
    // executions, support journal and repair timeline out of a LocalSystem
    // service. The only correct answer is to refuse.
    let owner =
        if v.owner_principal_key.trim().is_empty() || v.owner_principal_key == *principal_key {
            principal_key.clone()
        } else {
            return Err(ServiceError::forbidden(
                "persistence",
                "journal.ownerScopeForbidden",
                "owner_principal_key must be empty or equal to the calling principal",
            ));
        };
    let executions = ctx
        .db
        .maintenance_executions_for_owner(&owner, 2000)
        .map_err(err)?;
    let events = ctx
        .db
        .support_journal_events_for_owner(&owner, 500)
        .map_err(err)?;
    let timeline = ctx
        .db
        .repair_timeline_events_for_owner(&owner, 2000)
        .map_err(err)?;
    let mut records: Vec<(String, i64, serde_json::Value)> = Vec::new();
    for e in &executions {
        records.push((
            "maintenance_execution".to_string(),
            e.updated_unix_ms,
            serde_json::json!({
                "planId": e.plan_id,
                "domain": e.domain,
                "stage": e.stage,
                "overallPercent": e.overall_percent,
                "outcome": e.outcome,
                "startedUnixMs": e.started_unix_ms,
                "completedUnixMs": e.completed_unix_ms,
            }),
        ));
    }
    for e in &events {
        records.push((
            "plan_event".to_string(),
            e.created_unix_ms,
            serde_json::json!({
                "seq": e.seq,
                "planId": e.plan_id,
                "fromState": e.from_state,
                "toState": e.to_state,
                "eventKind": e.event_kind,
                "detail": e.detail,
            }),
        ));
    }
    for t in &timeline {
        records.push((
            "repair_timeline".to_string(),
            t.created_unix_ms,
            serde_json::json!({
                "eventId": t.event_id,
                "planId": t.plan_id,
                "domain": t.domain,
                "actionId": t.action_id,
                "eventKind": t.event_kind,
                "outcome": t.outcome,
            }),
        ));
    }
    let now = chrono::Utc::now().timestamp_millis();
    let to_bound = if v.to_unix_ms > 0 {
        v.to_unix_ms
    } else {
        i64::MAX
    };
    let from_bound = v.from_unix_ms;
    records.retain(|(_, ordinal, _)| *ordinal >= from_bound && *ordinal < to_bound);
    // source_db_fingerprint: hash of the record count + latest ordinal per
    // class (deterministic over the same data set without reading the db file).
    let mut fp = sha2::Sha256::new();
    use sha2::Digest as _;
    fp.update(records.len().to_le_bytes());
    for (kind, ordinal, _) in &records {
        fp.update(kind.as_bytes());
        fp.update(ordinal.to_le_bytes());
    }
    let fingerprint = format!("{:x}", fp.finalize());
    // Phase 31 (W9): the request id IS the correlation id for this export;
    // it is clamped typed by CorrelationId::parse inside the crate.
    let correlation_id = aethercore_persistence::export::CorrelationId::parse(principal_key)
        .or_else(|| aethercore_persistence::export::CorrelationId::parse(&format!("export-{now}")));
    let mut envelope = aethercore_persistence::export::build_envelope_with_correlation(
        records,
        now,
        fingerprint,
        correlation_id,
    );
    // Signing happens ONLY with an explicitly provisioned owner key file
    // pointed at by AETHERCORE_EXPORT_KEY; otherwise digest-only honesty.
    if let Some(key_path) = std::env::var_os("AETHERCORE_EXPORT_KEY") {
        if let Ok(key_hex) = std::fs::read_to_string(std::path::Path::new(&key_path)) {
            let key_hex = key_hex.trim();
            if let Some(seed) = decode_key_seed(key_hex) {
                let signing = aethercore_persistence::export::signing_key_from_seed(&seed);
                aethercore_persistence::export::sign_envelope(&mut envelope, &signing);
            }
        }
    }
    let response_payload = v1::ExportJournalResponse {
        envelope_json: serde_json::to_vec(&envelope).unwrap_or_default(),
        record_count: envelope.records.len() as u64,
        signed: envelope.signed,
    };
    aethercore_diagnostics::emit_structured(
        "info",
        aethercore_diagnostics::events::EXPORT_PRODUCED,
        serde_json::json!({
            "records": response_payload.record_count,
            "signed": response_payload.signed,
        }),
    );
    Ok(Some(response::Payload::ExportJournalResponse(
        response_payload,
    )))
}
