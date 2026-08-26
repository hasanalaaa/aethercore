//! Phase 30 (T1) — typed finding model with evidence-cited discipline.

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

/// Severity ladder ordered by operational urgency.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

/// How strongly the cited evidence supports the finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Confidence {
    Measured,
    Derived,
    Assumed,
}

/// One measured fact backing a finding. A rule without its full matrix emits NOTHING.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceRef {
    /// Stable fact identifier (e.g. "pragma.integrity_check.row").
    pub fact: String,
    /// What was actually observed, verbatim where applicable.
    pub observed: String,
    /// The expected value or threshold that was violated.
    pub expected_or_threshold: String,
    /// Where the observation came from (file:line, PRAGMA name, …).
    pub source_location: String,
}

/// Evidence-cited advisory finding. Advisory only — remediation is NOT in scope here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbFinding {
    pub id: String,
    pub code: String,
    pub severity: Severity,
    pub evidence: Vec<EvidenceRef>,
    pub summary_key: String,
    #[serde(default)]
    pub message_args: Vec<String>,
    pub confidence: Confidence,
}

impl DbFinding {
    /// Guard required by the house rule: a finding must cite at least one measured
    /// fact; rules that cannot fill the matrix emit nothing instead of a hollow row.
    pub fn try_new(
        id: &str,
        code: &str,
        severity: Severity,
        mut evidence: Vec<EvidenceRef>,
        summary_key: &str,
        confidence: Confidence,
    ) -> Option<Self> {
        if evidence.is_empty() || id.is_empty() || code.is_empty() {
            return None;
        }
        // Deterministic evidence order → deterministic report digest downstream.
        evidence.sort_by(|a, b| {
            a.fact
                .cmp(&b.fact)
                .then(a.source_location.cmp(&b.source_location))
        });
        Some(Self {
            id: id.to_string(),
            code: code.to_string(),
            severity,
            evidence,
            summary_key: summary_key.to_string(),
            message_args: Vec::new(),
            confidence,
        })
    }
}

/// Typed diagnostic target surface (read-only by construction).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DiagnosticTarget {
    SqliteFile { path: String },
    PostgresConfigDir { path: String },
    MysqlConfigDir { path: String },
    PostgresSlowLog { path: String },
    MysqlSlowLog { path: String },
    DataDirectory { path: String },
}

/// Final report: sorted findings + byte-stable digest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticReport {
    pub target_kind: String,
    pub target_path: String,
    pub findings: Vec<DbFinding>,
    /// sha256 over the canonical serialization of sorted findings.
    pub digest: String,
    /// Honest lane availability (engine-live lanes report NotAvailable here).
    pub lanes_not_available: Vec<String>,
}

/// Sorts findings deterministically (severity desc → code → id) and seals a digest.
pub fn seal_report(
    target_kind: &str,
    target_path: &str,
    mut findings: Vec<DbFinding>,
    lanes_not_available: Vec<String>,
) -> DiagnosticReport {
    findings.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| a.code.cmp(&b.code))
            .then_with(|| a.id.cmp(&b.id))
    });
    let canonical = serde_json::to_string(&findings).unwrap_or_default();
    let digest = hex_digest(canonical.as_bytes());
    DiagnosticReport {
        target_kind: target_kind.to_string(),
        target_path: target_path.to_string(),
        findings,
        digest,
        lanes_not_available,
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let out = hasher.finalize();
    let mut s = String::with_capacity(out.len() * 2);
    for b in out {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
