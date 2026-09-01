//! Phase 39 — the attack, run against the INSTALLED Windows service over its real
//! named pipe.
//!
//! The unit and unix-socket tests exercise the shared router. They cannot exercise the
//! thing that makes this a privilege defect: a genuinely unprivileged local user talking
//! to a LocalSystem service through a pipe whose DACL grants Authenticated Users by
//! design. This probe is that caller. Run it under a standard-user token.
//!
//! It lives in `tools/`, NOT in a shipping crate. It was written as an
//! `apps/aetherctl/examples/` file on the theory that "examples are not packaged";
//! DBT-P36-008 records an example binary that nonetheless reached
//! `C:\Program Files\AetherCore`, so the theory is not load-bearing any more and the
//! file was moved (DBT-P40-001). `scripts/check_msi_payload.py` now fails the build if
//! any developer binary appears in the MSI File table.
//!
//! It adds no product API: it uses `aethercore_ipc::SessionClient` exactly as `aetherctl`
//! already does — the mistake DBT-P36-001/002 recorded, when the last probes widened a
//! product crate's public surface to serve them.
//!
//! Usage (as the unprivileged user):
//!   p39_pipe_attack.exe <victim-path>
//!
//! Prints one `KEY=value` line per probe and exits 0 when every attack was REFUSED and
//! every legitimate call still WORKED; non-zero otherwise, so a harness can gate on it.

#[cfg(not(windows))]
fn main() {
    eprintln!("p39_pipe_attack is Windows-only: it probes the named-pipe service");
    std::process::exit(2);
}

#[cfg(windows)]
fn main() {
    use std::{sync::Arc, time::Duration};

    use aethercore_contracts::{
        PROTOCOL_VERSION,
        v1::{self, Request, RequestHeader, request},
    };

    let victim = std::env::args().nth(1).unwrap_or_else(|| {
        // Another user's profile is the point; fall back to the well-known system
        // profile, which an unprivileged user must also never be able to read through us.
        r"C:\Windows\System32\config\systemprofile".to_string()
    });
    let own = std::env::var("USERPROFILE").unwrap_or_default();

    let noop_event =
        Arc::new(|_: v1::EventEnvelope| {}) as Arc<dyn Fn(v1::EventEnvelope) + Send + Sync>;
    let noop_reset =
        Arc::new(|_: v1::StreamReset| {}) as Arc<dyn Fn(v1::StreamReset) + Send + Sync>;
    let noop_disconnect = Arc::new(|| {}) as Arc<dyn Fn() + Send + Sync>;
    let client = match aethercore_ipc::SessionClient::connect(
        "p39-pipe-attack",
        env!("CARGO_PKG_VERSION"),
        0,
        noop_event,
        noop_reset,
        noop_disconnect,
    ) {
        Ok(client) => client,
        Err(error) => {
            println!("CONNECTED=no ERROR={error}");
            std::process::exit(3);
        }
    };
    println!("CONNECTED=yes");
    println!("VICTIM_PATH={victim}");
    println!("OWN_PATH={own}");

    let mut sequence = 0u32;
    let mut call = |payload: request::Payload| -> (u32, String, String) {
        sequence += 1;
        let request = Request {
            header: Some(RequestHeader {
                protocol_version: PROTOCOL_VERSION,
                request_id: format!("p39-attack-{sequence:06}"),
            }),
            payload: Some(payload),
        };
        match client.request(request, Duration::from_secs(30)) {
            Ok(response) => {
                let key = response
                    .error
                    .as_ref()
                    .map(|e| format!("{} [{}]", e.message_key, e.technical_detail))
                    .unwrap_or_default();
                let body = match response.payload.as_ref() {
                    Some(v1::response::Payload::SecurityAuditResponse(report)) => report
                        .lanes
                        .iter()
                        .map(|lane| {
                            format!(
                                "{}:{}:{}:{}",
                                lane.lane,
                                lane.status,
                                lane.finding_count,
                                String::from_utf8_lossy(&lane.findings_json)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(" | "),
                    Some(v1::response::Payload::ExportJournalResponse(export)) => {
                        format!("records={} signed={}", export.record_count, export.signed)
                    }
                    Some(_) => "other-payload".to_string(),
                    None => String::new(),
                };
                (response.status_code, key, body)
            }
            Err(error) => (u32::MAX, format!("transport:{error}"), String::new()),
        }
    };

    let audit = |dir: &str| {
        request::Payload::RunSecurityAudit(v1::RunSecurityAuditRequest {
            targets_json: format!(
                r#"[{{"kind":"secretsDir","dir":{}}}]"#,
                serde_json::to_string(dir).unwrap_or_else(|_| "\"\"".to_string())
            )
            .into_bytes(),
        })
    };
    let journal = |owner: &str| {
        request::Payload::ExportJournal(v1::ExportJournalRequest {
            from_unix_ms: 0,
            to_unix_ms: 0,
            owner_principal_key: owner.to_string(),
        })
    };

    // Returns true when the service answered as it must. Kept a free function so the
    // failure counter is not borrowed by a closure that also has to be called.
    fn record(name: &str, expected_refused: bool, outcome: (u32, String, String)) -> bool {
        let (status, key, body) = outcome;
        let refused = status == 403;
        let ok = refused == expected_refused && (expected_refused || status == 0);
        println!(
            "{name}=STATUS:{status} KEY:{key} VERDICT:{} BODY:{body}",
            if ok { "AS_EXPECTED" } else { "UNEXPECTED" }
        );
        ok
    }

    let mut failures = 0usize;
    let mut check = |ok: bool| {
        if !ok {
            failures += 1;
        }
    };

    check(record(
        "ATTACK_AUDIT_FOREIGN_PATH",
        true,
        call(audit(&victim)),
    ));
    check(record(
        "ATTACK_JOURNAL_FOREIGN_OWNER",
        true,
        call(journal(&"f".repeat(64))),
    ));
    if own.is_empty() {
        println!("LEGIT_AUDIT_OWN_SCOPE=SKIPPED no USERPROFILE");
        check(false);
    } else {
        check(record("LEGIT_AUDIT_OWN_SCOPE", false, call(audit(&own))));
    }
    check(record("LEGIT_JOURNAL_OWN_SCOPE", false, call(journal(""))));
    drop(check);

    println!("FAILURES={failures}");
    std::process::exit(if failures == 0 { 0 } else { 1 });
}
