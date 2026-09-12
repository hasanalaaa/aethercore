//! Phase 32 — Password-policy provider (read-only).
//!
//! Parses login.defs-style files (`PASS_MAX_DAYS`, `PASS_MIN_DAYS`,
//! `PASS_WARN_AGE`, `PASS_MIN_LEN`). On macOS these paths differ by
//! construction; callers must surface the typed NotAvailable reason instead of
//! pretending a scan happened (QD-032-002).

use crate::model::{
    Confidence, EvidenceRef, MAX_PARSE_BYTES, SecFinding, Severity, sort_and_clamp,
};
use std::path::Path;

struct PassRule {
    keyword: &'static str,
    code: &'static str,
    id: &'static str,
    /// Violates when value > bound (for max-style) or < bound (min-style).
    max_violation: Option<u64>,
    min_violation: Option<u64>,
    expected: &'static str,
    severity: Severity,
    cis: Option<&'static str>,
}

const PASS_RULES: &[PassRule] = &[
    PassRule {
        keyword: "PASS_MAX_DAYS",
        code: "pass.max_days",
        id: "SEC-PASS-001",
        max_violation: Some(365),
        min_violation: None,
        expected: "<= 365",
        severity: Severity::Medium,
        cis: Some("CIS L1 5.5.1"),
    },
    PassRule {
        keyword: "PASS_MIN_DAYS",
        code: "pass.min_days",
        id: "SEC-PASS-002",
        max_violation: None,
        min_violation: Some(1),
        expected: ">= 1",
        severity: Severity::Low,
        cis: Some("CIS L1 5.5.2"),
    },
    PassRule {
        keyword: "PASS_MIN_LEN",
        code: "pass.min_len",
        id: "SEC-PASS-003",
        max_violation: None,
        min_violation: Some(14),
        expected: ">= 14 (pam_pwquality minlen otherwise)",
        severity: Severity::Medium,
        cis: None,
    },
];

/// Typed reason the password-policy lane cannot run on this host.
pub fn platform_note() -> &'static str {
    if cfg!(target_os = "macos") {
        "macOS stores password policy outside login.defs (OpenDirectory pwpolicy); \
         file-based lint is NotAvailable on this platform"
    } else {
        ""
    }
}

pub fn audit_password_policy(path_str: &str) -> Result<Vec<SecFinding>, String> {
    let path = Path::new(path_str);
    let meta = std::fs::metadata(path).map_err(|e| format!("stat: {e}"))?;
    if !meta.is_file() {
        return Err(format!("not a regular file: {path_str}"));
    }
    if meta.len() > MAX_PARSE_BYTES as u64 {
        return Err(format!("oversize: {} > {MAX_PARSE_BYTES}", meta.len()));
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("read: {e}"))?;
    let mut findings = Vec::new();
    for rule in PASS_RULES {
        let mut seen: Option<(String, usize, String)> = None;
        for (idx, raw) in text.lines().enumerate() {
            let t = raw.trim();
            if t.is_empty() || t.starts_with('#') || !t.starts_with(rule.keyword) {
                continue;
            }
            let rest = t[rule.keyword.len()..].trim();
            // First occurrence wins; verbatim citation.
            seen = Some((rest.to_string(), idx + 1, raw.trim_end().to_string()));
            break;
        }
        if let Some((val_s, line_no, verbatim)) = seen
            && let Ok(val) = val_s.split_whitespace().next().unwrap_or("").parse::<u64>()
        {
            let violates = rule
                .max_violation
                .map(|m| val > m)
                .or_else(|| rule.min_violation.map(|m| val < m))
                .unwrap_or(false);
            if violates
                && let Some(f) = SecFinding::try_new(
                    rule.id,
                    rule.code,
                    rule.severity,
                    vec![EvidenceRef {
                        fact: verbatim,
                        observed: format!("{}={val}", rule.keyword),
                        expected_or_threshold: rule.expected.to_string(),
                        source_location: format!("{path_str}:{line_no}"),
                    }],
                    rule.cis,
                    match rule.code {
                        "pass.max_days" => "sec.pass.maxDays",
                        "pass.min_days" => "sec.pass.minDays",
                        _ => "sec.pass.minLen",
                    },
                    Confidence::Exact,
                )
            {
                findings.push(f);
            }
        }
    }
    Ok(sort_and_clamp(findings))
}
