//! Phase 33 — strict offline compliance profiles and signed point-in-time reports.
//!
//! Profiles carry identifiers and local rule mappings only. They do not
//! republish CIS Benchmark prose. Evaluation is read-only and treats missing,
//! unavailable, permission-denied, or partial evidence as `NotVerified`.

use crate::{LaneStatus, SecurityAuditReport};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeSet;
use std::path::Path;

pub const PROFILE_SCHEMA: &str = "COMPLIANCE_PROFILE_V1";
pub const REPORT_SCHEMA: &str = "aethercore.compliance.v1";

pub const IMPLEMENTED_RULE_CODES: &[&str] = &[
    "auth.failure_burst",
    "cve.vulnerable_package",
    "fs.scan_truncated",
    "fs.ssh_dir_perms",
    "fs.ssh_key_perms",
    "fs.suid_inventory",
    "fs.world_writable",
    "fw.config_present",
    "fw.state_not_available",
    "pass.max_days",
    "pass.min_days",
    "pass.min_len",
    "secrets.aws_key",
    "secrets.dotenv",
    "secrets.generic_assignment",
    "secrets.private_key_block",
    "secrets.scan_truncated",
    "ssh.client_alive",
    "ssh.max_auth_tries",
    "ssh.password_authentication",
    "ssh.permit_root_login",
    "ssh.pubkey_authentication",
    "ssh.x11_forwarding",
    "sudo.nopasswd",
    "sudo.wildcard_all",
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema: String,
    pub profile_id: String,
    pub title_key: String,
    pub control_families: Vec<ControlFamily>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ControlFamily {
    pub family_id: String,
    pub title_key: String,
    pub controls: Vec<Control>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub control_id: String,
    pub rule_codes: Vec<String>,
    pub applicability: Applicability,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Applicability {
    Unix,
    Windows,
    All,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ControlStatus {
    Pass,
    Fail,
    NotApplicable,
    NotVerified,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ControlResult {
    pub control_id: String,
    pub status: ControlStatus,
    pub evidence_refs: Vec<String>,
    pub reason_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "state", rename_all = "camelCase", deny_unknown_fields)]
pub enum ScorePct {
    Calculated { value: f64 },
    NoVerifiableControls,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Score {
    pub pass: usize,
    pub fail: usize,
    pub na: usize,
    pub not_verified: usize,
    pub score_pct: ScorePct,
}

impl Score {
    pub fn total(&self) -> usize {
        self.pass + self.fail + self.na + self.not_verified
    }

    fn from_controls(controls: &[ControlResult]) -> Self {
        let (mut pass, mut fail, mut na, mut not_verified) = (0, 0, 0, 0);
        for control in controls {
            match control.status {
                ControlStatus::Pass => pass += 1,
                ControlStatus::Fail => fail += 1,
                ControlStatus::NotApplicable => na += 1,
                ControlStatus::NotVerified => not_verified += 1,
            }
        }
        let score_pct = if pass + fail == 0 {
            ScorePct::NoVerifiableControls
        } else {
            ScorePct::Calculated {
                value: pass as f64 * 100.0 / (pass + fail) as f64,
            }
        };
        Self {
            pass,
            fail,
            na,
            not_verified,
            score_pct,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReportSignature {
    pub public_key_hex: String,
    pub signature_hex: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ComplianceReport {
    pub schema: String,
    pub profile_id: String,
    pub host_fingerprint: String,
    pub generated_unix_ms: i64,
    pub controls: Vec<ControlResult>,
    pub score: Score,
    pub digest: String,
    pub signed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<ReportSignature>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ComplianceError {
    #[error("malformed profile: {0}")]
    MalformedProfile(String),
    #[error("unknown profile schema: {0}")]
    UnknownProfileSchema(String),
    #[error("unknown profile: {0}")]
    UnknownProfile(String),
    #[error("duplicate family id: {0}")]
    DuplicateFamily(String),
    #[error("duplicate control id: {0}")]
    DuplicateControl(String),
    #[error("duplicate rule mapping: {0}")]
    DuplicateRuleMapping(String),
    #[error("empty control: {0}")]
    EmptyControl(String),
    #[error("unknown rule code: {0}")]
    UnknownRule(String),
    #[error("malformed CIS control id: {0}")]
    MalformedControlId(String),
    #[error("malformed report: {0}")]
    MalformedReport(String),
    #[error("report schema mismatch: {0}")]
    ReportSchema(String),
    #[error("report profile mismatch: {0}")]
    ReportProfile(String),
    #[error("report score totals are inconsistent")]
    ScoreMismatch,
    #[error("report contains duplicate control id: {0}")]
    DuplicateReportControl(String),
    #[error("digest mismatch")]
    DigestMismatch,
    #[error("signed flag inconsistent with signature presence")]
    SignatureFlagInconsistent,
    #[error("canonical report serialization failed: {0}")]
    Canonicalization(String),
}

impl Profile {
    pub fn from_bytes(raw: &[u8]) -> Result<Self, ComplianceError> {
        let profile: Self = serde_json::from_slice(raw)
            .map_err(|error| ComplianceError::MalformedProfile(error.to_string()))?;
        profile.validate()?;
        Ok(profile)
    }

    pub fn load(path: &Path) -> Result<Self, ComplianceError> {
        let raw = std::fs::read(path)
            .map_err(|error| ComplianceError::MalformedProfile(error.to_string()))?;
        Self::from_bytes(&raw)
    }

    pub fn validate(&self) -> Result<(), ComplianceError> {
        if self.schema != PROFILE_SCHEMA {
            return Err(ComplianceError::UnknownProfileSchema(self.schema.clone()));
        }
        if !matches!(self.profile_id.as_str(), "cis-l1" | "cis-l2") {
            return Err(ComplianceError::UnknownProfile(self.profile_id.clone()));
        }
        if self.title_key.trim().is_empty() || self.control_families.is_empty() {
            return Err(ComplianceError::MalformedProfile(
                "title_key and control_families must be non-empty".to_string(),
            ));
        }
        let mut family_ids = BTreeSet::new();
        let mut control_ids = BTreeSet::new();
        let mut mapped_rules = BTreeSet::new();
        for family in &self.control_families {
            if family.family_id.trim().is_empty()
                || family.title_key.trim().is_empty()
                || family.controls.is_empty()
            {
                return Err(ComplianceError::MalformedProfile(format!(
                    "family '{}' is empty",
                    family.family_id
                )));
            }
            if !family_ids.insert(family.family_id.clone()) {
                return Err(ComplianceError::DuplicateFamily(family.family_id.clone()));
            }
            for control in &family.controls {
                if !control_ids.insert(control.control_id.clone()) {
                    return Err(ComplianceError::DuplicateControl(
                        control.control_id.clone(),
                    ));
                }
                if control.rule_codes.is_empty() {
                    return Err(ComplianceError::EmptyControl(control.control_id.clone()));
                }
                if !valid_control_id(&control.control_id, &self.profile_id) {
                    return Err(ComplianceError::MalformedControlId(
                        control.control_id.clone(),
                    ));
                }
                for code in &control.rule_codes {
                    if !IMPLEMENTED_RULE_CODES.contains(&code.as_str()) {
                        return Err(ComplianceError::UnknownRule(code.clone()));
                    }
                    if !mapped_rules.insert(code.clone()) {
                        return Err(ComplianceError::DuplicateRuleMapping(code.clone()));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn controls(&self) -> impl Iterator<Item = &Control> {
        self.control_families
            .iter()
            .flat_map(|family| family.controls.iter())
    }
}

fn valid_control_id(control_id: &str, profile_id: &str) -> bool {
    let prefix = match profile_id {
        "cis-l1" => "CIS L1 ",
        "cis-l2" => "CIS L2 ",
        _ => return false,
    };
    let Some(rest) = control_id.strip_prefix(prefix) else {
        return false;
    };
    let parts: Vec<_> = rest.split('.').collect();
    (2..=3).contains(&parts.len())
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

fn lane_matches(lane: &str, rule_code: &str) -> bool {
    let prefix = rule_code.split('.').next().unwrap_or(rule_code);
    match prefix {
        "ssh" => lane == "sshd",
        "sudo" => lane == "sudoers",
        "pass" => lane == "password",
        "fs" => lane == "filesystem",
        "fw" => lane == "firewall",
        _ => lane.starts_with(prefix),
    }
}

fn applicable(applicability: Applicability, platform: &str) -> bool {
    match applicability {
        Applicability::All => true,
        Applicability::Unix => platform != "windows",
        Applicability::Windows => platform == "windows",
    }
}

fn is_unavailable_marker(rule_code: &str) -> bool {
    rule_code == "fw.state_not_available" || rule_code.ends_with(".scan_truncated")
}

fn is_positive_observation(rule_code: &str) -> bool {
    rule_code == "fw.config_present"
}

pub fn evaluate(
    profile: &Profile,
    audit: &SecurityAuditReport,
    host_fingerprint: impl Into<String>,
    generated_unix_ms: i64,
) -> Result<ComplianceReport, ComplianceError> {
    profile.validate()?;
    let mut controls = Vec::new();
    for control in profile.controls() {
        if !applicable(control.applicability, &audit.platform) {
            controls.push(ControlResult {
                control_id: control.control_id.clone(),
                status: ControlStatus::NotApplicable,
                evidence_refs: Vec::new(),
                reason_key: "compliance.not_applicable".to_string(),
            });
            continue;
        }
        let mut failed = false;
        let mut verified = true;
        let mut evidence_refs = Vec::new();
        for rule_code in &control.rule_codes {
            let matching_lanes: Vec<_> = audit
                .lanes
                .iter()
                .filter(|lane| lane_matches(&lane.lane, rule_code))
                .collect();
            if matching_lanes.is_empty() {
                verified = false;
                continue;
            }
            for lane in matching_lanes {
                if matches!(lane.status, LaneStatus::NotAvailable { .. }) {
                    verified = false;
                    continue;
                }
                for finding in lane
                    .findings
                    .iter()
                    .filter(|finding| finding.code == *rule_code)
                {
                    evidence_refs.push(finding.id.clone());
                    if is_unavailable_marker(rule_code) {
                        verified = false;
                    } else if !is_positive_observation(rule_code) {
                        failed = true;
                    }
                }
            }
        }
        evidence_refs.sort();
        evidence_refs.dedup();
        let status = if failed {
            ControlStatus::Fail
        } else if !verified {
            ControlStatus::NotVerified
        } else {
            ControlStatus::Pass
        };
        let reason_key = match status {
            ControlStatus::Pass => "compliance.verified",
            ControlStatus::Fail => "compliance.finding",
            ControlStatus::NotApplicable => "compliance.not_applicable",
            ControlStatus::NotVerified => "compliance.not_verified",
        };
        controls.push(ControlResult {
            control_id: control.control_id.clone(),
            status,
            evidence_refs,
            reason_key: reason_key.to_string(),
        });
    }
    controls.sort_by(|left, right| left.control_id.cmp(&right.control_id));
    let score = Score::from_controls(&controls);
    if score.total() != controls.len() {
        return Err(ComplianceError::ScoreMismatch);
    }
    let mut report = ComplianceReport {
        schema: REPORT_SCHEMA.to_string(),
        profile_id: profile.profile_id.clone(),
        host_fingerprint: host_fingerprint.into(),
        generated_unix_ms,
        controls,
        score,
        digest: String::new(),
        signed: false,
        signature: None,
    };
    report.digest = digest(&report)?;
    Ok(report)
}

/// Digest rule: timestamp, digest, signed flag, and signature are excluded;
/// controls and evidence references are sorted before canonical serialization.
pub fn digest(report: &ComplianceReport) -> Result<String, ComplianceError> {
    let mut canonical = report.clone();
    canonical.generated_unix_ms = 0;
    canonical.digest.clear();
    canonical.signed = false;
    canonical.signature = None;
    canonical
        .controls
        .sort_by(|left, right| left.control_id.cmp(&right.control_id));
    for control in &mut canonical.controls {
        control.evidence_refs.sort()
    }
    let bytes = serde_json::to_vec(&canonical)
        .map_err(|error| ComplianceError::Canonicalization(error.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

pub fn parse_report_bytes(raw: &[u8]) -> Result<ComplianceReport, ComplianceError> {
    serde_json::from_slice(raw).map_err(|error| ComplianceError::MalformedReport(error.to_string()))
}

pub fn verify_integrity(report: &ComplianceReport) -> Result<(), ComplianceError> {
    if report.schema != REPORT_SCHEMA {
        return Err(ComplianceError::ReportSchema(report.schema.clone()));
    }
    if !matches!(report.profile_id.as_str(), "cis-l1" | "cis-l2") {
        return Err(ComplianceError::ReportProfile(report.profile_id.clone()));
    }
    if report.signed != report.signature.is_some() {
        return Err(ComplianceError::SignatureFlagInconsistent);
    }
    let mut ids = BTreeSet::new();
    for control in &report.controls {
        if !ids.insert(control.control_id.clone()) {
            return Err(ComplianceError::DuplicateReportControl(
                control.control_id.clone(),
            ));
        }
    }
    if Score::from_controls(&report.controls) != report.score
        || report.score.total() != report.controls.len()
    {
        return Err(ComplianceError::ScoreMismatch);
    }
    if report.digest != digest(report)? {
        return Err(ComplianceError::DigestMismatch);
    }
    Ok(())
}

/// Renders one printable, self-contained bilingual document with inline CSS.
pub fn render_html(report: &ComplianceReport) -> String {
    let rows = report
        .controls
        .iter()
        .map(|control| {
            format!(
                "<tr><td><code>{}</code></td><td>{:?}</td><td><code>{}</code></td><td>{}</td></tr>",
                escape(&control.control_id),
                control.status,
                escape(&control.reason_key),
                escape(&control.evidence_refs.join(", "))
            )
        })
        .collect::<String>();
    let score = match report.score.score_pct {
        ScorePct::Calculated { value } => format!("{value:.2}%"),
        ScorePct::NoVerifiableControls => "N/A".to_string(),
    };
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Compliance report</title><style>body{{font-family:system-ui,sans-serif;margin:2rem;color:#182230}}section{{margin-block:2rem}}table{{border-collapse:collapse;width:100%}}th,td{{border:1px solid #ccd3dc;padding:.45rem;text-align:start}}code{{font-family:ui-monospace,monospace}}@media print{{body{{margin:0}}section{{break-inside:avoid}}}}</style></head><body><section dir=\"ltr\"><h1>Compliance report</h1><p><code>{}</code> · <code>{}</code> · score {}</p></section><section lang=\"ar\" dir=\"rtl\"><h1>تقرير الامتثال</h1><p><code>{}</code> · <code>{}</code> · النتيجة {}</p></section><table><thead><tr><th>Control / الضابط</th><th>Status / الحالة</th><th>Reason / السبب</th><th>Evidence / الدليل</th></tr></thead><tbody>{}</tbody></table></body></html>",
        escape(&report.profile_id),
        escape(&report.host_fingerprint),
        score,
        escape(&report.profile_id),
        escape(&report.host_fingerprint),
        score,
        rows
    )
}

fn escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_ignores_timestamp() {
        let score = Score {
            pass: 1,
            fail: 0,
            na: 0,
            not_verified: 0,
            score_pct: ScorePct::Calculated { value: 100.0 },
        };
        let first = ComplianceReport {
            schema: REPORT_SCHEMA.to_string(),
            profile_id: "cis-l1".to_string(),
            host_fingerprint: "host".to_string(),
            generated_unix_ms: 1,
            controls: Vec::new(),
            score,
            digest: String::new(),
            signed: false,
            signature: None,
        };
        let mut second = first.clone();
        second.generated_unix_ms = 2;
        assert_eq!(digest(&first), digest(&second));
    }
}
