//! GD-5 — remote compliance report honesty proofs (Phase 34).
//!
//! A remote `fleet compliance` collection returns report JSON bytes; the
//! controlling host verifies them with the ESTABLISHED P33 path
//! (parse_report_bytes + verify_integrity + Phase29 signature primitives).
//! This suite proves:
//! - valid report accepted;
//! - signed report verified (signature valid);
//! - content tamper rejected (digest mismatch);
//! - signature tamper rejected;
//! - unsigned report remains explicitly unsigned (never "authenticated").

use aethercore_security_audit::compliance::{
    ControlStatus, Profile, parse_report_bytes, verify_integrity,
};
use aethercore_security_audit::{LaneReport, LaneStatus, SecurityAuditReport, compliance};
use std::path::PathBuf;

fn profile() -> Profile {
    Profile::load(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/compliance/profiles/cis-l1.json"),
    )
    .unwrap()
}

fn sample_audit() -> SecurityAuditReport {
    SecurityAuditReport {
        schema_version: 1,
        platform: "macos".into(),
        lanes: vec![
            LaneReport {
                lane: "sshd".into(),
                status: LaneStatus::ok(0),
                findings: vec![],
            },
            LaneReport {
                lane: "password".into(),
                status: LaneStatus::ok(0),
                findings: vec![],
            },
            LaneReport {
                lane: "sudoers".into(),
                status: LaneStatus::ok(0),
                findings: vec![],
            },
            LaneReport {
                lane: "filesystem".into(),
                status: LaneStatus::ok(0),
                findings: vec![],
            },
            LaneReport {
                lane: "secrets".into(),
                status: LaneStatus::ok(0),
                findings: vec![],
            },
            LaneReport {
                lane: "firewall".into(),
                status: LaneStatus::ok(0),
                findings: vec![],
            },
            LaneReport {
                lane: "authlog[0]".into(),
                status: LaneStatus::ok(0),
                findings: vec![],
            },
            LaneReport {
                lane: "cve".into(),
                status: LaneStatus::ok(0),
                findings: vec![],
            },
        ],
        digest: "fleet-sample-host-digest".into(),
    }
}

fn report_bytes(signed: bool) -> Vec<u8> {
    let audit = sample_audit();
    let mut report =
        compliance::evaluate(&profile(), &audit, "fleet-host-1", 1_700_000_000_000).unwrap();
    if signed {
        let seed = [7u8; 32]; // test-only deterministic seed
        let signing_key = aethercore_persistence::export::signing_key_from_seed(&seed);
        let signature = aethercore_persistence::export::sign_digest(&report.digest, &signing_key);
        report.signed = true;
        report.signature = Some(compliance::ReportSignature {
            public_key_hex: signature.public_key_hex,
            signature_hex: signature.signature_hex,
        });
    }
    serde_json::to_vec_pretty(&report).unwrap()
}

/// The remote-verification pipeline a `fleet compliance` run uses locally.
fn verify_remote_collection(raw: &[u8]) -> Result<compliance::ComplianceReport, String> {
    let report = parse_report_bytes(raw).map_err(|error| error.to_string())?;
    verify_integrity(&report).map_err(|error| error.to_string())?;
    if let Some(signature) = &report.signature {
        let export_signature = aethercore_persistence::export::ExportSignature {
            public_key_hex: signature.public_key_hex.clone(),
            signature_hex: signature.signature_hex.clone(),
        };
        aethercore_persistence::export::verify_digest_signature(&report.digest, &export_signature)
            .map_err(|_| "signature mismatch".to_string())?;
    }
    Ok(report)
}

#[test]
fn gd5_valid_unsigned_report_accepted_and_stays_unsigned() {
    let report = verify_remote_collection(&report_bytes(false)).expect("valid report verifies");
    assert!(!report.signed);
    assert!(report.signature.is_none());
    // unsigned honesty: score present, no authentication implied
    assert_eq!(report.profile_id, "cis-l1");
}

#[test]
fn gd5_signed_report_verifies() {
    let report = verify_remote_collection(&report_bytes(true)).expect("signed report verifies");
    assert!(report.signed);
    assert!(report.signature.is_some());
}

#[test]
fn gd5_content_tamper_rejected() {
    let mut value: serde_json::Value = serde_json::from_slice(&report_bytes(false)).unwrap();
    // alter control content without touching the score: rewrite one
    // evidence reference (digest covers controls; totals unchanged)
    let controls = value.get_mut("controls").unwrap().as_array_mut().unwrap();
    let mut flipped = false;
    for control in controls.iter_mut() {
        if let Some(evidence) = control
            .get_mut("evidence_refs")
            .and_then(|e| e.as_array_mut())
        {
            evidence.push(serde_json::json!("tampered-evidence-ref"));
            flipped = true;
            break;
        }
    }
    assert!(flipped);
    let tampered = serde_json::to_vec(&value).unwrap();
    // exact error family: DigestMismatch (after ScoreMismatch is impossible
    // here because pass/fail swap preserves totals — the digest check fires)
    let error = verify_remote_collection(&tampered).unwrap_err();
    assert!(
        error.contains("digest"),
        "typed digest rejection expected, got: {error}"
    );
}

#[test]
fn gd5_signature_tamper_rejected() {
    let mut value: serde_json::Value = serde_json::from_slice(&report_bytes(true)).unwrap();
    let signature_hex = value["signature"]["signature_hex"]
        .as_str()
        .unwrap()
        .to_string();
    // flip one hex character of the signature
    let flipped: String = signature_hex
        .char_indices()
        .map(|(i, c)| {
            if i == 0 && c == 'a' {
                'b'
            } else if i == 0 {
                'a'
            } else {
                c
            }
        })
        .collect();
    assert_ne!(flipped, signature_hex);
    value["signature"]["signature_hex"] = serde_json::json!(flipped);
    let tampered = serde_json::to_vec(&value).unwrap();
    let error = verify_remote_collection(&tampered).unwrap_err();
    assert!(
        error.contains("signature"),
        "typed signature rejection expected, got: {error}"
    );
}

#[test]
fn gd5_status_semantics_survive_transport() {
    // NotVerified is first-class on the remote side too — never averaged into pass.
    let report = verify_remote_collection(&report_bytes(false)).unwrap();
    let statuses: Vec<ControlStatus> = report.controls.iter().map(|c| c.status).collect();
    assert!(
        statuses.contains(&ControlStatus::Pass) || statuses.contains(&ControlStatus::NotVerified)
    );
    // score total invariant intact
    assert_eq!(report.score.total(), report.controls.len());
}
