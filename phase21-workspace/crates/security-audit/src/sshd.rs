//! Phase 32 — sshd_config lint provider (read-only).
//!
//! Cites matched lines VERBATIM. Where an option is absent, the finding cites
//! the effective OpenSSH default per this host's client version family and
//! marks confidence=Inferred. No process is spawned, nothing is written.

use crate::model::{
    sort_and_clamp, AuditTarget, Confidence, EvidenceRef, SecFinding, Severity, MAX_PARSE_BYTES,
};
use std::path::Path;

/// One lint rule: keyword, violating values, expected value, CIS ref.
struct SshRule {
    code: &'static str,
    id: &'static str,
    keyword: &'static str,
    /// Values that trigger the finding (lowercased compare).
    bad_values: &'static [&'static str],
    expected: &'static str,
    severity: Severity,
    cis: Option<&'static str>,
    summary_key: &'static str,
}

const SSH_RULES: &[SshRule] = &[
    SshRule {
        code: "ssh.permit_root_login",
        id: "SEC-SSH-001",
        keyword: "permitrootlogin",
        bad_values: &["yes"],
        expected: "no | prohibit-password",
        severity: Severity::High,
        cis: Some("CIS L1 5.2.8"),
        summary_key: "sec.ssh.permitRootLogin",
    },
    SshRule {
        code: "ssh.password_authentication",
        id: "SEC-SSH-002",
        keyword: "passwordauthentication",
        bad_values: &["yes"],
        expected: "no",
        severity: Severity::Medium,
        cis: Some("CIS L1 5.2.9"),
        summary_key: "sec.ssh.passwordAuthentication",
    },
    SshRule {
        code: "ssh.pubkey_authentication",
        id: "SEC-SSH-003",
        keyword: "pubkeyauthentication",
        bad_values: &["no"],
        expected: "yes",
        severity: Severity::Medium,
        cis: None,
        summary_key: "sec.ssh.pubkeyAuthentication",
    },
    SshRule {
        code: "ssh.x11_forwarding",
        id: "SEC-SSH-004",
        keyword: "x11forwarding",
        bad_values: &["yes"],
        expected: "no",
        severity: Severity::Low,
        cis: None,
        summary_key: "sec.ssh.x11Forwarding",
    },
    SshRule {
        code: "ssh.max_auth_tries",
        id: "SEC-SSH-005",
        keyword: "maxauthtries",
        bad_values: &[],
        expected: "<= 4",
        severity: Severity::Low,
        cis: Some("CIS L1 5.2.10"),
        summary_key: "sec.ssh.maxAuthTries",
    },
    SshRule {
        code: "ssh.client_alive",
        id: "SEC-SSH-006",
        keyword: "clientaliveinterval",
        bad_values: &[],
        expected: "1..900 seconds (and ClientAliveCountMax<=3)",
        severity: Severity::Advisory,
        cis: None,
        summary_key: "sec.ssh.clientAlive",
    },
];

fn first_token(v: &str) -> String {
    v.split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Parses `text` as sshd_config and returns (keyword_lc_lower, value_rest,
/// line_number, verbatim_line) tuples honoring only the FIRST occurrence of
/// each keyword (sshd semantics).
fn parse_directives(text: &str) -> Vec<(String, String, usize, String)> {
    let mut out = Vec::new();
    for (idx, raw_line) in text.lines().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut it = trimmed.splitn(2, ['=', ' ']);
        let kw = it.next().unwrap_or("").trim().to_ascii_lowercase();
        let val = it.next().unwrap_or("").trim();
        if kw.is_empty() {
            continue;
        }
        // First occurrence wins (matches sshd_config semantics).
        if out
            .iter()
            .any(|(k, _, _, _): &(String, String, usize, String)| *k == kw)
        {
            continue;
        }
        out.push((kw, val.to_string(), idx + 1, raw_line.to_string()));
    }
    out
}

/// Runs the sshd_config lint over the given path. Missing/oversize file is an
/// honest error string (caller degrades the lane).
pub fn audit_sshd_config(path_str: &str) -> Result<Vec<SecFinding>, String> {
    let path = Path::new(path_str);
    let meta = std::fs::metadata(path).map_err(|e| format!("stat: {e}"))?;
    if !meta.is_file() {
        return Err(format!("not a regular file: {path_str}"));
    }
    if meta.len() > MAX_PARSE_BYTES as u64 {
        return Err(format!("oversize: {} > {MAX_PARSE_BYTES}", meta.len()));
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("read: {e}"))?;
    let directives = parse_directives(&text);
    let loc = |n: usize| format!("{path_str}:{n}");
    let mut findings = Vec::new();
    for rule in SSH_RULES {
        match directives.iter().find(|(k, _, _, _)| *k == rule.keyword) {
            Some((_, val, line_no, verbatim)) => {
                let tok = first_token(val);
                // DBT-P46-B20: a directive present but unparseable (non-numeric, or
                // out of u32/u64 range) cannot be verified safe, so it fails CLOSED
                // like every other rule here — never silently read as compliant.
                let violates = if rule.code == "ssh.max_auth_tries" {
                    tok.parse::<u32>().map(|n| n > 4).unwrap_or(true)
                } else if rule.code == "ssh.client_alive" {
                    tok.parse::<u64>().map(|n| !(1..=900).contains(&n)).unwrap_or(true)
                } else {
                    rule.bad_values.contains(&tok.as_str())
                };
                if violates
                    && let Some(f) = SecFinding::try_new(
                        rule.id,
                        rule.code,
                        rule.severity,
                        vec![EvidenceRef {
                            fact: verbatim.clone(),
                            observed: format!("{}={}", rule.keyword, val),
                            expected_or_threshold: rule.expected.to_string(),
                            source_location: loc(*line_no),
                        }],
                        rule.cis,
                        rule.summary_key,
                        Confidence::Exact,
                    ) {
                        findings.push(f);
                }
            }
            None => {
                // Absence handling: only MaxAuthTries gets a default-derived
                // finding (OpenSSH default 6 violates the ≤4 expectation).
                // Every other rule's documented default is compliant, so an
                // absent directive is NOT a violation — honest silence.
                if rule.code == "ssh.max_auth_tries"
                    && let Some(f) = SecFinding::try_new(
                        rule.id,
                        rule.code,
                        Severity::Advisory,
                        vec![EvidenceRef {
                            fact: format!("# directive absent from {path_str}"),
                            observed: format!("{}=<absent>", rule.keyword),
                            expected_or_threshold: rule.expected.to_string(),
                            source_location: path_str.to_string(),
                        }],
                        rule.cis,
                        rule.summary_key,
                        Confidence::Inferred,
                    ) {
                        findings.push(f);
                }
                // All other rules' documented defaults are compliant ⇒ an absent
                // directive is honest silence (no violation claim).
            }
        }
    }
    Ok(sort_and_clamp(findings))
}

/// Dispatch by target (kept total so new targets fail honestly at compile time).
pub fn audit_target(target: &AuditTarget) -> Result<Vec<SecFinding>, String> {
    match target {
        AuditTarget::SshdConfig { path } => audit_sshd_config(path),
        _ => Err("provider mismatch: not an sshd target".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempConfig {
        path: std::path::PathBuf,
    }

    impl TempConfig {
        fn write(label: &str, contents: &str) -> TempConfig {
            let path = std::env::temp_dir().join(format!(
                "aethercore-sshd-test-{label}-{}-{:?}.conf",
                std::process::id(),
                std::thread::current().id()
            ));
            std::fs::write(&path, contents).expect("write temp sshd_config");
            TempConfig { path }
        }
    }

    impl Drop for TempConfig {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    fn codes(findings: &[SecFinding]) -> Vec<&str> {
        findings.iter().map(|f| f.code.as_str()).collect()
    }

    // DBT-P46-B20: a malformed (non-numeric) MaxAuthTries value must be flagged,
    // not silently read as compliant. Before the fix, `unwrap_or(false)` made an
    // unparseable value indistinguishable from "4 or fewer attempts".
    #[test]
    fn a_non_numeric_max_auth_tries_value_is_flagged_not_silently_compliant() {
        let cfg = TempConfig::write("bad-max-auth-tries", "MaxAuthTries abc\n");
        let findings =
            audit_sshd_config(cfg.path.to_str().unwrap()).expect("parse succeeds");
        assert!(
            codes(&findings).contains(&"ssh.max_auth_tries"),
            "a non-numeric MaxAuthTries must be flagged, got: {findings:?}"
        );
    }

    // The sibling rule already got this right by coincidence (0 fails the 1..=900
    // range check) — pin it explicitly so a future refactor can't regress it while
    // "fixing" the rule above.
    #[test]
    fn a_non_numeric_client_alive_interval_value_is_flagged() {
        let cfg = TempConfig::write("bad-client-alive", "ClientAliveInterval abc\n");
        let findings =
            audit_sshd_config(cfg.path.to_str().unwrap()).expect("parse succeeds");
        assert!(
            codes(&findings).contains(&"ssh.client_alive"),
            "a non-numeric ClientAliveInterval must be flagged, got: {findings:?}"
        );
    }

    #[test]
    fn a_compliant_numeric_max_auth_tries_value_is_not_flagged() {
        let cfg = TempConfig::write("ok-max-auth-tries", "MaxAuthTries 3\n");
        let findings =
            audit_sshd_config(cfg.path.to_str().unwrap()).expect("parse succeeds");
        assert!(
            !codes(&findings).contains(&"ssh.max_auth_tries"),
            "a compliant value must not be flagged, got: {findings:?}"
        );
    }

    #[test]
    fn an_out_of_range_numeric_max_auth_tries_value_is_still_flagged() {
        let cfg = TempConfig::write("high-max-auth-tries", "MaxAuthTries 999999999999\n");
        let findings =
            audit_sshd_config(cfg.path.to_str().unwrap()).expect("parse succeeds");
        assert!(
            codes(&findings).contains(&"ssh.max_auth_tries"),
            "an out-of-u32-range value overflows parse and must fail closed too, got: {findings:?}"
        );
    }
}
