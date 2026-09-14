//! The read-only security-audit domain.

use super::*;

pub(super) fn run_security_audit(call: &Call<'_>, v: v1::RunSecurityAuditRequest) -> Routed {
    let ctx = call.ctx;
    let peer = call.peer;
    let principal_key = &call.principal_key;
    // Read-only RPC over the aethercore-security-audit domain: zero
    // system mutations, zero network. It is NOT zero-privilege — the
    // Windows host runs as LocalSystem and reverts impersonation before
    // dispatch, so a caller-named path is read with the service's authority
    // and not the caller's. Targets are therefore confined to the roots the
    // OS reported for THIS caller's own token, which is the owner-scoped
    // allowlist `operations.proto` has always promised here. Refusal is
    // typed and happens before any provider runs.
    let parsed: Result<Vec<aethercore_security_audit::model::AuditTarget>, String> =
        serde_json::from_slice::<Vec<serde_json::Value>>(&v.targets_json)
            .map_err(|e| format!("targets parse: {e}"))
            .and_then(|vals| {
                vals.into_iter()
                    .map(|val| {
                        serde_json::from_value::<aethercore_security_audit::model::AuditTarget>(val)
                            .map_err(|e| format!("target decode: {e}"))
                    })
                    .collect()
            });
    let targets = match parsed {
        Ok(t) if !t.is_empty() => t,
        Ok(_) => {
            // Typed rejection BEFORE any provider runs.
            return Err(ServiceError::invalid(
                "security",
                "sec.invalidTargets",
                "targets_json must be a non-empty AuditTarget array",
            ));
        }
        Err(detail) => {
            return Err(ServiceError::invalid(
                "security",
                "sec.invalidTargets",
                detail,
            ));
        }
    };
    let scope = aethercore_security_audit::OwnerScope::new(peer.owner_roots());
    if let Err(denial) = aethercore_security_audit::authorize_targets(&targets, &scope) {
        return Err(ServiceError::forbidden(
            "security",
            denial.code(),
            denial.to_string(),
        ));
    }
    let report = aethercore_security_audit::run_audit(&targets);
    let lanes = report
        .lanes
        .iter()
        .map(|l| v1::SecurityAuditReportLane {
            lane: l.lane.clone(),
            status: match l.status {
                aethercore_security_audit::LaneStatus::Ok { .. } => "ok".to_string(),
                aethercore_security_audit::LaneStatus::NotAvailable { .. } => {
                    "notAvailable".to_string()
                }
            },
            reason: match &l.status {
                aethercore_security_audit::LaneStatus::NotAvailable { reason } => reason.clone(),
                _ => String::new(),
            },
            finding_count: l.findings.len() as u64,
            findings_json: serde_json::to_vec(&l.findings).unwrap_or_default(),
        })
        .collect();
    aethercore_diagnostics::emit_structured(
        "info",
        aethercore_diagnostics::events::SECURITY_AUDIT_RAN,
        serde_json::json!({
            "digest": report.digest,
            "lanes": report.lanes.len(),
        }),
    );
    publish(
        ctx,
        principal_key,
        EventKind::SecurityAudit,
        "",
        Some(event_envelope::Payload::SecurityAudit(
            v1::SecurityAuditResponse {
                schema_version: report.schema_version,
                platform: report.platform.clone(),
                digest: report.digest.clone(),
                lanes,
            },
        )),
    );
    Ok(Some(response::Payload::SecurityAuditResponse(
        v1::SecurityAuditResponse {
            schema_version: report.schema_version,
            platform: report.platform.clone(),
            digest: report.digest,
            lanes: report
                .lanes
                .iter()
                .map(|l| v1::SecurityAuditReportLane {
                    lane: l.lane.clone(),
                    status: match l.status {
                        aethercore_security_audit::LaneStatus::Ok { .. } => "ok".to_string(),
                        aethercore_security_audit::LaneStatus::NotAvailable { .. } => {
                            "notAvailable".to_string()
                        }
                    },
                    reason: match &l.status {
                        aethercore_security_audit::LaneStatus::NotAvailable { reason } => {
                            reason.clone()
                        }
                        _ => String::new(),
                    },
                    finding_count: l.findings.len() as u64,
                    findings_json: serde_json::to_vec(&l.findings).unwrap_or_default(),
                })
                .collect(),
        },
    )))
}
