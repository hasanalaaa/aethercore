use aethercore_security_audit::compliance::{
    self, ComplianceError, ControlStatus, IMPLEMENTED_RULE_CODES, Profile, ScorePct,
};
use aethercore_security_audit::model::{Confidence, EvidenceRef, SecFinding, Severity};
use aethercore_security_audit::{LaneReport, LaneStatus, SecurityAuditReport};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[test]
fn compliance_profile_rejects_unknown_fields() {
    let err = Profile::from_bytes(br#"{"schema":"COMPLIANCE_PROFILE_V1","profile_id":"cis-l1","title_key":"x","unknown":1,"control_families":[]}"#).unwrap_err();
    assert!(matches!(err, ComplianceError::MalformedProfile(_)));
    println!("GD-5 unknown JSON field => {err}");
}

#[test]
fn not_verified_is_not_pass() {
    assert_ne!(ControlStatus::NotVerified, ControlStatus::Pass);
}

#[test]
fn profile_rejects_invalid_rule_and_control_shape() {
    let base = r#"{"schema":"COMPLIANCE_PROFILE_V1","profile_id":"cis-l1","title_key":"x","control_families":[{"family_id":"f","title_key":"f","controls":[{"control_id":"CIS L1 1.1","rule_codes":["no.such.rule"],"applicability":"all"}]}]}"#;
    let unknown_rule = Profile::from_bytes(base.as_bytes()).unwrap_err();
    assert_eq!(
        unknown_rule,
        ComplianceError::UnknownRule("no.such.rule".into())
    );
    let empty = base.replace("no.such.rule", "");
    let empty_rule = Profile::from_bytes(empty.as_bytes()).unwrap_err();
    assert_eq!(empty_rule, ComplianceError::UnknownRule(String::new()));
    println!("GD-5 nonexistent rule_code => {unknown_rule}");
    println!("GD-5 empty rule_code => {empty_rule}");
}

#[test]
fn bundled_profiles_are_strict_and_cover_every_implemented_rule() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/compliance/profiles");
    let expected: BTreeSet<_> = IMPLEMENTED_RULE_CODES.iter().copied().collect();
    for name in ["cis-l1.json", "cis-l2.json"] {
        let profile = Profile::load(&root.join(name)).unwrap();
        let actual: BTreeSet<_> = profile
            .controls()
            .flat_map(|control| control.rule_codes.iter().map(String::as_str))
            .collect();
        assert_eq!(actual, expected, "incomplete P32 rule coverage in {name}");
    }
}

#[test]
fn gd5_rejects_duplicate_control_malformed_id_and_zero_rule_control() {
    let valid = control("CIS L1 1.1", &["ssh.password_authentication"], "unix");
    let duplicate = [valid.clone(), valid].join(",");
    let duplicate_error = Profile::from_bytes(
        format!(r#"{{"schema":"COMPLIANCE_PROFILE_V1","profile_id":"cis-l1","title_key":"x","control_families":[{{"family_id":"f","title_key":"f","controls":[{duplicate}]}}]}}"#).as_bytes(),
    ).unwrap_err();
    assert_eq!(
        duplicate_error,
        ComplianceError::DuplicateControl("CIS L1 1.1".into())
    );

    let malformed_control = control(
        "CIS L1 control-one",
        &["ssh.password_authentication"],
        "unix",
    );
    let malformed = Profile::from_bytes(
        format!(r#"{{"schema":"COMPLIANCE_PROFILE_V1","profile_id":"cis-l1","title_key":"x","control_families":[{{"family_id":"f","title_key":"f","controls":[{malformed_control}]}}]}}"#).as_bytes(),
    ).unwrap_err();
    assert_eq!(
        malformed,
        ComplianceError::MalformedControlId("CIS L1 control-one".into())
    );

    let zero_rules = serde_json::json!({
        "schema": "COMPLIANCE_PROFILE_V1",
        "profile_id": "cis-l1",
        "title_key": "x",
        "control_families": [{
            "family_id": "f",
            "title_key": "f",
            "controls": [{
                "control_id": "CIS L1 1.1",
                "rule_codes": [],
                "applicability": "all"
            }]
        }]
    });
    let zero_error = Profile::from_bytes(&serde_json::to_vec(&zero_rules).unwrap()).unwrap_err();
    assert_eq!(
        zero_error,
        ComplianceError::EmptyControl("CIS L1 1.1".into())
    );

    println!("GD-5 duplicate control ID => {duplicate_error}");
    println!("GD-5 malformed CIS control ID => {malformed}");
    println!("GD-5 zero-rule control => {zero_error}");
}

fn profile(controls: &str) -> Profile {
    let raw = format!(
        r#"{{"schema":"COMPLIANCE_PROFILE_V1","profile_id":"cis-l1","title_key":"compliance.profile.cisL1","control_families":[{{"family_id":"f","title_key":"compliance.family.general","controls":[{controls}]}}]}}"#
    );
    Profile::from_bytes(raw.as_bytes()).unwrap()
}

fn control(id: &str, rules: &[&str], applicability: &str) -> String {
    serde_json::json!({
        "control_id": id,
        "rule_codes": rules,
        "applicability": applicability,
    })
    .to_string()
}

fn finding(code: &str) -> SecFinding {
    SecFinding::try_new(
        "SEC-TEST-001",
        code,
        Severity::High,
        vec![EvidenceRef {
            fact: "sensitive raw fact must not enter compliance refs".into(),
            observed: "bad".into(),
            expected_or_threshold: "good".into(),
            source_location: "/fixture:1".into(),
        }],
        None,
        "sec.test",
        Confidence::Exact,
    )
    .unwrap()
}

#[test]
fn profile_rejects_duplicate_rule_mapping() {
    let controls = [
        control("CIS L1 1.1", &["ssh.password_authentication"], "unix"),
        control("CIS L1 1.2", &["ssh.password_authentication"], "unix"),
    ]
    .join(",");
    let raw = format!(
        r#"{{"schema":"COMPLIANCE_PROFILE_V1","profile_id":"cis-l1","title_key":"x","control_families":[{{"family_id":"f","title_key":"f","controls":[{controls}]}}]}}"#
    );
    assert!(
        Profile::from_bytes(raw.as_bytes())
            .unwrap_err()
            .to_string()
            .contains("duplicate rule mapping")
    );
}

#[test]
fn gd1_control_matrix_is_honest_and_totals_exact() {
    let profile = Profile::load(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/compliance/profiles/cis-l1.json"),
    )
    .unwrap();
    let audit = SecurityAuditReport {
        schema_version: 1,
        platform: "macos".into(),
        lanes: vec![
            LaneReport {
                lane: "sshd".into(),
                status: LaneStatus::ok(2),
                findings: vec![
                    finding("ssh.permit_root_login"),
                    finding("ssh.password_authentication"),
                ],
            },
            LaneReport {
                lane: "password".into(),
                status: LaneStatus::not_available("permission denied"),
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
        digest: "host-audit-digest".into(),
    };
    let report = compliance::evaluate(&profile, &audit, "host", 7).unwrap();
    let statuses: Vec<_> = report.controls.iter().map(|c| c.status).collect();
    assert_eq!(
        statuses,
        vec![
            ControlStatus::Pass,
            ControlStatus::Pass,
            ControlStatus::Pass,
            ControlStatus::Pass,
            ControlStatus::Fail,
            ControlStatus::Pass,
            ControlStatus::NotVerified,
            ControlStatus::Pass,
        ]
    );
    assert_eq!(
        (
            report.score.pass,
            report.score.fail,
            report.score.na,
            report.score.not_verified
        ),
        (6, 1, 0, 1)
    );
    assert_eq!(
        report.score.score_pct,
        ScorePct::Calculated {
            value: 6.0 * 100.0 / 7.0
        }
    );
    assert_eq!(report.controls.len(), report.score.total());
    assert!(
        !serde_json::to_string(&report)
            .unwrap()
            .contains("sensitive raw fact")
    );
    println!(
        "GD-1 control matrix: {}",
        serde_json::to_string(&report.controls).unwrap()
    );
    println!(
        "GD-1 score: {}",
        serde_json::to_string(&report.score).unwrap()
    );
}

#[test]
fn zero_verifiable_controls_has_typed_state() {
    let profile = profile(&control("CIS L1 1.1", &["pass.min_len"], "all"));
    let audit = SecurityAuditReport {
        schema_version: 1,
        platform: "macos".into(),
        lanes: vec![LaneReport {
            lane: "password".into(),
            status: LaneStatus::not_available("unavailable"),
            findings: vec![],
        }],
        digest: "d".into(),
    };
    let report = compliance::evaluate(&profile, &audit, "host", 1).unwrap();
    assert_eq!(report.score.score_pct, ScorePct::NoVerifiableControls);
}

#[test]
fn digest_sorts_semantically_and_excludes_timestamp() {
    let controls_a = [
        control("CIS L1 1.1", &["ssh.password_authentication"], "unix"),
        control("CIS L1 1.2", &["ssh.pubkey_authentication"], "unix"),
    ]
    .join(",");
    let controls_b = [
        control("CIS L1 1.2", &["ssh.pubkey_authentication"], "unix"),
        control("CIS L1 1.1", &["ssh.password_authentication"], "unix"),
    ]
    .join(",");
    let audit = SecurityAuditReport {
        schema_version: 1,
        platform: "macos".into(),
        lanes: vec![LaneReport {
            lane: "sshd".into(),
            status: LaneStatus::ok(0),
            findings: vec![],
        }],
        digest: "d".into(),
    };
    let a = compliance::evaluate(&profile(&controls_a), &audit, "host", 1).unwrap();
    let mut b = compliance::evaluate(&profile(&controls_b), &audit, "host", 2).unwrap();
    assert_eq!(a.digest, b.digest);
    assert_eq!(
        serde_json::to_vec(&a.controls).unwrap(),
        serde_json::to_vec(&b.controls).unwrap()
    );
    assert_eq!(compliance::render_html(&a), compliance::render_html(&b));
    b.generated_unix_ms = a.generated_unix_ms;
    assert_eq!(
        serde_json::to_vec(&a).unwrap(),
        serde_json::to_vec(&b).unwrap()
    );
    println!("GD-2 digest identical: {}", a.digest);
    println!("GD-2 JSON identical after timestamp allowlist normalization: true");
    println!("GD-2 HTML identical: true");
}

#[test]
fn html_is_bilingual_rtl_and_air_gapped() {
    let profile = profile(&control(
        "CIS L1 1.1",
        &["ssh.pubkey_authentication"],
        "unix",
    ));
    let audit = SecurityAuditReport {
        schema_version: 1,
        platform: "macos".into(),
        lanes: vec![LaneReport {
            lane: "sshd".into(),
            status: LaneStatus::ok(0),
            findings: vec![],
        }],
        digest: "d".into(),
    };
    let report = compliance::evaluate(&profile, &audit, "host", 1).unwrap();
    let html = compliance::render_html(&report);
    assert!(html.contains("Compliance report"));
    assert!(html.contains("تقرير الامتثال"));
    assert!(html.contains("dir=\"rtl\""));
    assert!(!html.contains("http://"));
    assert!(!html.contains("https://"));
}
