//! Phase 32 golden proofs (GD-1..GD-3, GD-5). GD-4 (live host audit) runs via
//! the phase32 scripts against this Mac; it is not expressible as a hermetic
//! unit test because it must cite the real host's real files.

use aethercore_security_audit as sec;
use sec::model::{AuditTarget, Confidence, Severity};
use std::path::PathBuf;

/// DBT-P48-003: same shape as the four sites DBT-P42-013 fixed and as
/// `crates/ipc/tests/unix_adversarial.rs` — cleaned at the start of the NEXT
/// run, which means a panicking run leaves it forever. Measured after one
/// `cargo test --workspace`: six `p32-gd-*` directories left behind. The guard
/// is returned so `TempDir`'s Drop removes it whichever way the test ends.
fn tempdir(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("p32-gd-{name}-"))
        .tempdir()
        .expect("tempdir")
}

// ---------------- GD-1: sshd lint, weak vs hardened ----------------

const WEAK_SSHD: &str = "\
# planted weak values
Port 22
PermitRootLogin yes
PasswordAuthentication yes
PubkeyAuthentication no
X11Forwarding yes
MaxAuthTries 12
ClientAliveInterval 0
";

const HARDENED_SSHD: &str = "\
Port 22
PermitRootLogin prohibit-password
PasswordAuthentication no
PubkeyAuthentication yes
X11Forwarding no
MaxAuthTries 4
ClientAliveInterval 300
";

#[test]
fn gd1_sshd_weak_fixture_yields_exact_findings() {
    let dir_guard = tempdir("gd1");
    let dir = dir_guard.path().to_path_buf();
    let cfg = dir.join("sshd_config_weak");
    std::fs::write(&cfg, WEAK_SSHD).unwrap();
    let findings = sec::sshd::audit_sshd_config(cfg.to_str().unwrap()).expect("lint runs");
    let codes: Vec<&str> = findings.iter().map(|f| f.code.as_str()).collect();
    // Deterministic order: severity DESC, then code ASC.
    assert_eq!(
        codes,
        vec![
            "ssh.permit_root_login",
            "ssh.password_authentication",
            "ssh.pubkey_authentication",
            "ssh.max_auth_tries",
            "ssh.x11_forwarding",
            "ssh.client_alive",
        ]
    );
    // Verbatim line citations with correct line numbers.
    let f1 = &findings[0];
    assert_eq!(f1.severity, Severity::High);
    assert_eq!(f1.cis_ref.as_deref(), Some("CIS L1 5.2.8"));
    let ev = &f1.evidence[0];
    assert_eq!(ev.fact, "PermitRootLogin yes");
    assert!(ev.source_location.ends_with("sshd_config_weak:3"));
    // First-occurrence-wins semantics: duplicate directive is ignored.
    let dup = format!("{WEAK_SSHD}PermitRootLogin no\n");
    std::fs::write(dir.join("sshd_dup"), dup).unwrap();
    let dfind = sec::sshd::audit_sshd_config(dir.join("sshd_dup").to_str().unwrap()).unwrap();
    let root = dfind
        .iter()
        .find(|f| f.code == "ssh.permit_root_login")
        .unwrap();
    assert_eq!(root.evidence[0].fact, "PermitRootLogin yes");
    let _ = dir;
}

#[test]
fn gd1_sshd_hardened_fixture_zero_findings() {
    let dir_guard = tempdir("gd1h");
    let dir = dir_guard.path().to_path_buf();
    let cfg = dir.join("sshd_config_hard");
    std::fs::write(&cfg, HARDENED_SSHD).unwrap();
    let findings = sec::sshd::audit_sshd_config(cfg.to_str().unwrap()).expect("lint runs");
    assert!(
        findings.is_empty(),
        "expected zero findings, got {findings:?}"
    );
}

// ---------------- GD-2: secrets scanner + redaction contract ----------------

const FAKE_AWS: &str = "AKIAIOSFODNN7EXAMPLE";
const FAKE_PEM_HEADER: &str = "-----BEGIN RSA PRIVATE KEY-----";
const CLEAN_BODY: &str = "password = correcthorsebatterystaple\nLOG_LEVEL=debug\n";

#[test]
fn gd2_secrets_planted_are_found_redacted() {
    let dir_guard = tempdir("gd2");
    let dir = dir_guard.path().to_path_buf();
    let f = dir.join("leaky.env");
    std::fs::write(
        &f,
        format!("AWS_KEY={FAKE_AWS}\n{FAKE_PEM_HEADER}\n{CLEAN_BODY}"),
    )
    .unwrap();
    let findings = sec::secrets::scan_secrets(dir.to_str().unwrap()).expect("scan");
    let codes: Vec<&str> = findings.iter().map(|x| x.code.as_str()).collect();
    assert!(codes.contains(&"secrets.aws_key"), "{codes:?}");
    assert!(codes.contains(&"secrets.private_key_block"), "{codes:?}");
    assert!(codes.contains(&"secrets.dotenv"), "{codes:?}");
    // Redaction contract: raw secret NEVER appears; first-4-chars form does.
    let blob = format!("{findings:#?}");
    assert!(!blob.contains(FAKE_AWS), "RAW SECRET LEAKED: {blob}");
    assert!(
        blob.contains(&sec::secrets::redact(FAKE_AWS)),
        "redacted form missing"
    );
    assert_eq!(sec::secrets::redact(FAKE_AWS), "AKIA****************");
    let aws = findings
        .iter()
        .find(|x| x.code == "secrets.aws_key")
        .unwrap();
    assert_eq!(aws.severity, Severity::Critical);
    assert!(aws.evidence[0].source_location.ends_with("leaky.env:1"));
}

#[test]
fn gd2_clean_dir_zero_false_positives() {
    let dir_guard = tempdir("gd2c");
    let dir = dir_guard.path().to_path_buf();
    std::fs::write(dir.join("clean.txt"), CLEAN_BODY).unwrap();
    std::fs::write(dir.join("notes.md"), "# notes\nno secrets here\n").unwrap();
    let findings = sec::secrets::scan_secrets(dir.to_str().unwrap()).expect("scan");
    let secretish: Vec<_> = findings
        .iter()
        .filter(|f| f.code.starts_with("secrets."))
        .collect();
    assert!(
        secretish.is_empty(),
        "false positives on clean dir: {secretish:?}"
    );
}

// ---------------- GD-3: CVE join against seeded fixture DB ----------------

#[test]
fn gd3_cve_join_exact_match_set() {
    use sec::vulndb::VulnEntry;
    use sec::vulnjoin::{compare_versions, join, InstalledPackage};
    let db = vec![
        VulnEntry {
            cve_id: "CVE-2026-0001".into(),
            package: "openssl".into(),
            introduced: "".into(),
            fixed: "3.0.14".into(),
            summary: "fixture".into(),
        },
        VulnEntry {
            cve_id: "CVE-2026-0002".into(),
            package: "openssl".into(),
            introduced: "3.0.0".into(),
            fixed: "3.0.14".into(),
            summary: "fixture range".into(),
        },
        VulnEntry {
            cve_id: "CVE-2026-0003".into(),
            package: "zlib".into(),
            introduced: "1.0".into(),
            fixed: "1.3.1".into(),
            summary: "not installed".into(),
        },
    ];
    let installed = vec![
        InstalledPackage {
            name: "openssl".into(),
            version: "3.0.13".into(),
        },
        InstalledPackage {
            name: "totally-unknown".into(),
            version: "9.9.9".into(),
        },
    ];
    let m = join(&installed, &db);
    let ids: Vec<&str> = m.iter().map(|x| x.cve_id.as_str()).collect();
    assert_eq!(ids, vec!["CVE-2026-0001", "CVE-2026-0002"]);
    assert_eq!(m[0].installed_version, "3.0.13");
    assert_eq!(m[0].fixed, "3.0.14");
    // Unknown packages are ignored entirely.
    assert!(m.iter().all(|x| x.package == "openssl"));
    // Version comparator sanity used by the join.
    assert_eq!(
        compare_versions("3.0.13", "3.0.14"),
        std::cmp::Ordering::Less
    );
    assert_eq!(
        compare_versions("3.0.14", "3.0.13"),
        std::cmp::Ordering::Greater
    );
}

// ---------------- GD-5: vulndb tamper ⇒ fail-closed typed error ----------------

#[test]
fn gd5_tampered_db_refused_fail_closed() {
    use sec::vulndb::{load_verified, VulnDbError};
    use sha2::{Digest as _, Sha256};
    let dir_guard = tempdir("gd5");
    let dir = dir_guard.path().to_path_buf();
    let db = dir.join("vulndb.json");
    let mf = dir.join("vulndb.manifest.json");
    let entries = r#"[{"cve_id":"CVE-2026-0001","package":"openssl","introduced":"","fixed":"3.0.14","summary":"x"}]"#;
    std::fs::write(&db, entries).unwrap();
    // Manifest pin built inline — the crate itself never writes.
    let digest = format!("{:x}", Sha256::digest(entries.as_bytes()));
    let manifest = format!(
        "{{\"schema\":\"aethercore.vulndb.manifest.v1\",\"entries\":1,\"sha256\":\"{digest}\"}}"
    );
    std::fs::write(&mf, manifest).unwrap();
    // Honest load succeeds.
    assert!(load_verified(&db, &mf).is_ok());
    // Flip ONE byte.
    let tampered = entries.replace("openssl", "openssM");
    std::fs::write(&db, tampered).unwrap();
    match load_verified(&db, &mf) {
        Err(VulnDbError::HashMismatch {
            expected, actual, ..
        }) => {
            assert_ne!(expected, actual);
        }
        other => panic!("expected HashMismatch, got {other:?}"),
    }
    // Entry-count drift alone is also refused.
    let more = format!(
        "[{},{{\"cve_id\":\"CVE-2026-0002\",\"package\":\"z\",\"introduced\":\"\",\"fixed\":\"1\",\"summary\":\"y\"}}]",
        entries
    );
    std::fs::write(&db, more).unwrap();
    assert!(matches!(
        load_verified(&db, &mf),
        Err(VulnDbError::HashMismatch { .. })
    ));
}

// ---------------- Auth-log failure bursts (fixture log) ----------------

#[test]
fn gd_authlog_burst_detected_and_quiet_log_clean() {
    let dir_guard = tempdir("gd-auth");
    let dir = dir_guard.path().to_path_buf();
    // 12 failures from 203.0.113.9 inside a minute ⇒ above threshold.
    let mut noisy = String::new();
    for i in 0..12 {
        noisy.push_str(&format!(
            "Aug 26 10:00:{:02} host sshd[1{}]: Failed password for invalid user admin from 203.0.113.9 port 220{} ssh2\n",
            i,
            i,
            i % 10
        ));
    }
    // One lonely failure from another source stays under threshold.
    noisy.push_str(
        "Aug 26 10:05:00 host sshd[199]: Failed password for root from 198.51.100.7 port 22099 ssh2\n",
    );
    std::fs::write(dir.join("auth.log"), &noisy).unwrap();
    let findings =
        sec::authlog::audit_auth_log(dir.join("auth.log").to_str().unwrap()).expect("parse");
    assert_eq!(findings.len(), 1);
    let f = &findings[0];
    assert_eq!(f.code, "auth.failure_burst");
    assert_eq!(f.confidence, Confidence::Heuristic);
    assert_eq!(f.severity, Severity::Advisory);
    let obs = &f.evidence[0].observed;
    assert!(obs.starts_with("12 failures"), "{obs}");
    // Quiet log ⇒ zero findings.
    std::fs::write(
        dir.join("quiet.log"),
        "Aug 26 11:00:00 host sshd[7]: Accepted public-key for ops from 10.0.0.1\n",
    )
    .unwrap();
    let quiet = sec::authlog::audit_auth_log(dir.join("quiet.log").to_str().unwrap()).unwrap();
    assert!(quiet.is_empty());
}

// ---------------- Model guards (house discipline) ----------------

#[test]
fn targets_serialize_stably() {
    let t = AuditTarget::SshdConfig {
        path: "/etc/ssh/sshd_config".into(),
    };
    let j = serde_json::to_string(&t).unwrap();
    assert!(j.contains("\"kind\""));
    let _back: AuditTarget = serde_json::from_str(&j).unwrap();
}

#[test]
fn confidence_ordering_is_total() {
    use Confidence::*;
    let mut v = vec![Heuristic, Exact, Inferred];
    v.sort();
    assert_eq!(v, vec![Exact, Inferred, Heuristic]);
}
