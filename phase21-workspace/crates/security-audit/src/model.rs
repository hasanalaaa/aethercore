//! Phase 32 — Security-audit domain model.
//!
//! House discipline reused from P23/P29/P30: typed findings with mandatory
//! verbatim evidence (`SecFinding::try_new` refuses zero-evidence findings),
//! deterministic digests over sorted findings, and hard clamps everywhere
//! (same bounds family as the P30/P31 read-only lanes).

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

/// Hard clamp: maximum parse size per config file, in bytes. Same clamp family
/// as P30 (`MAX_PARSE_BYTES`) so every offline lane degrades identically under
/// pathological inputs.
pub const MAX_PARSE_BYTES: usize = 1_048_576;

/// Hard clamp: maximum findings a single provider may return; extras are
/// dropped deterministically after severity-then-code ordering.
pub const MAX_FINDINGS: usize = 512;

/// Hard clamp: maximum files one filesystem/authlog/secrets scan may visit.
pub const MAX_FILES_PER_SCAN: usize = 4_096;

/// Hard clamp: wall-clock budget for one bounded scan (filesystem/secrets).
pub const MAX_SCAN_MILLIS: u64 = 2_000;

/// Maximum evidence refs carried by one finding (deterministic truncation).
pub const MAX_EVIDENCE_REFS: usize = 32;

/// Audit target selector — typed, unix-first. Every variant names exactly what
/// will be read; nothing here implies write access or elevation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum AuditTarget {
    /// sshd_config path (lint-only).
    SshdConfig { path: String },
    /// login.defs-style password policy file path.
    PasswordPolicy { path: String },
    /// sudoers file or includedir directory (parse-only; never evaluated via sudo).
    Sudoers { path: String },
    /// Filesystem posture scan over explicit paths (world-writable / SUID / .ssh).
    FilesystemPaths { paths: Vec<String> },
    /// auth.log/secure-style log files (failure-burst analysis).
    AuthLogs { paths: Vec<String> },
    /// Offline secrets scan over an EXPLICITLY named directory.
    SecretsDir { dir: String },
    /// Host firewall state via config-file presence only.
    FirewallState,
}

/// One verbatim fact backing a finding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceRef {
    /// The exact observed text (already privacy-redacted where applicable).
    pub fact: String,
    /// What was observed in machine terms (e.g. `PermitRootLogin=yes`).
    pub observed: String,
    /// The expected value/threshold that was violated.
    pub expected_or_threshold: String,
    /// Where it was seen (e.g. `/etc/ssh/sshd_config:47`).
    pub source_location: String,
}

/// Finding severity, ordered low → critical.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Advisory,
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Advisory => "advisory",
            Severity::Low => "low",
            Severity::Medium => "medium",
            Severity::High => "high",
            Severity::Critical => "critical",
        }
    }
}

/// How directly the rule proves the condition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Confidence {
    /// Verbatim line matched an unambiguous rule.
    Exact,
    /// Matched with interpretation (e.g. default-derived, include expansion).
    Inferred,
    /// Heuristic signal (entropy/burst), advisory only.
    Heuristic,
}

impl Confidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Confidence::Exact => "exact",
            Confidence::Inferred => "inferred",
            Confidence::Heuristic => "heuristic",
        }
    }
}

/// A security finding. Construction is guarded: no evidence, no finding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SecFinding {
    /// Stable rule identity, e.g. `SEC-SSH-001`.
    pub id: String,
    /// Rule code used in cis_map lookups, e.g. `ssh.permit_root_login`.
    pub code: String,
    pub severity: Severity,
    pub evidence: Vec<EvidenceRef>,
    /// CIS benchmark control reference when mapped (e.g. `CIS L1 5.2.8`).
    pub cis_ref: Option<String>,
    /// Localized-message key (renderer catalogs); never raw prose identity.
    pub summary_key: String,
    pub confidence: Confidence,
}

impl SecFinding {
    /// Guarded constructor: zero evidence → None (house anti-snake-oil rule).
    /// Enforces caps deterministically (evidence truncated to `MAX_EVIDENCE_REFS`).
    pub fn try_new(
        id: &str,
        code: &str,
        severity: Severity,
        evidence: Vec<EvidenceRef>,
        cis_ref: Option<&str>,
        summary_key: &str,
        confidence: Confidence,
    ) -> Option<SecFinding> {
        if evidence.is_empty() || id.is_empty() || code.is_empty() || summary_key.is_empty() {
            return None;
        }
        let mut evidence = evidence;
        if evidence.len() > MAX_EVIDENCE_REFS {
            evidence.truncate(MAX_EVIDENCE_REFS);
        }
        Some(SecFinding {
            id: id.to_string(),
            code: code.to_string(),
            severity,
            evidence,
            cis_ref: cis_ref.map(|s| s.to_string()),
            summary_key: summary_key.to_string(),
            confidence,
        })
    }
}

/// Deterministic report digest: findings sorted by (severity desc, code asc,
/// first-evidence location asc) then hashed over their canonical JSON.
pub fn report_digest(findings: &[SecFinding]) -> String {
    let mut sorted: Vec<&SecFinding> = findings.iter().collect();
    sorted.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| a.code.cmp(&b.code))
            .then_with(|| a.id.cmp(&b.id))
            .then_with(|| {
                let la = a
                    .evidence
                    .first()
                    .map(|e| e.source_location.as_str())
                    .unwrap_or("");
                let lb = b
                    .evidence
                    .first()
                    .map(|e| e.source_location.as_str())
                    .unwrap_or("");
                la.cmp(lb)
            })
    });
    let mut hasher = Sha256::new();
    for f in sorted {
        let canonical = serde_json::to_vec(f).unwrap_or_default();
        hasher.update((canonical.len() as u64).to_le_bytes());
        hasher.update(canonical);
    }
    format!("{:x}", hasher.finalize())
}

/// Sort + clamp helper shared by providers: severity desc, then code/id/location.
pub fn sort_and_clamp(mut findings: Vec<SecFinding>) -> Vec<SecFinding> {
    findings.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| a.code.cmp(&b.code))
            .then_with(|| a.id.cmp(&b.id))
            .then_with(|| {
                let la = a.evidence.first().map(|e| e.source_location.as_str());
                let lb = b.evidence.first().map(|e| e.source_location.as_str());
                la.unwrap_or("").cmp(lb.unwrap_or(""))
            })
    });
    if findings.len() > MAX_FINDINGS {
        findings.truncate(MAX_FINDINGS);
    }
    findings
}
