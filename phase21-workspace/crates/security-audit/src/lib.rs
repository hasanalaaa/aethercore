//! Phase 32 — aethercore-security-audit: read-only security audit domain.
//!
//! CIS-mapped, offline-by-construction config/state linter. Contract:
//! - READ-ONLY: providers parse files; zero system mutations, zero elevation.
//! - HONEST: every lane that cannot run yields typed NotAvailable/Degraded
//!   reasons instead of simulated results.
//! - DETERMINISTIC: findings sorted + clamped, report digest stable per input.
//!
//! Network access is BANNED in this crate (audit gate enforces the symbol
//! ban; vulndb updates are explicit owner actions via external tooling).

pub mod authlog;
pub mod census;
pub mod cis_map;
pub mod compliance;
pub mod filesystem;
pub mod firewall;
pub mod model;
pub mod password;
pub mod scope;
pub mod secrets;
pub mod sshd;
pub mod sudoers;
pub mod vulndb;
pub mod vulnjoin;

use serde::{Deserialize, Serialize};

pub use scope::{OwnerScope, TargetDenial, authorize_targets, is_reparse_point};

/// Report schema version.
pub const REPORT_SCHEMA_VERSION: u32 = 1;

/// Typed status of one audit lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum LaneStatus {
    /// Lane ran and produced N findings.
    Ok { count: usize },
    /// Lane could not produce an honest answer; reason is user-facing.
    NotAvailable { reason: String },
}

impl LaneStatus {
    pub fn ok(count: usize) -> LaneStatus {
        LaneStatus::Ok { count }
    }
    pub fn not_available(reason: impl Into<String>) -> LaneStatus {
        LaneStatus::NotAvailable {
            reason: reason.into(),
        }
    }
}

/// One lane's outcome.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaneReport {
    /// Lane identity, e.g. `sshd`, `secrets`, `cve`.
    pub lane: String,
    pub status: LaneStatus,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub findings: Vec<model::SecFinding>,
}

/// Full audit report over a target set. Deterministic ordering by lane name.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityAuditReport {
    pub schema_version: u32,
    /// Host platform/SKU tag (for example "windowsServerCore").
    pub platform: String,
    pub lanes: Vec<LaneReport>,
    /// Deterministic SHA-256 over sorted findings (see model::report_digest).
    pub digest: String,
}

/// Typed traversal guard: rejects any target path containing a `..`
/// component (defense-in-depth for service- and CLI-invoked scans).
pub fn validate_targets(targets: &[model::AuditTarget]) -> Result<(), String> {
    fn check(p: &str) -> Result<(), String> {
        let bad = std::path::Path::new(p)
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir));
        if bad {
            Err(format!("traversal rejected: {p}"))
        } else {
            Ok(())
        }
    }
    for t in targets {
        match t {
            model::AuditTarget::SshdConfig { path } => check(path)?,
            model::AuditTarget::PasswordPolicy { path } => check(path)?,
            model::AuditTarget::Sudoers { path } => check(path)?,
            model::AuditTarget::FilesystemPaths { paths } => {
                for p in paths {
                    check(p)?;
                }
            }
            model::AuditTarget::AuthLogs { paths } => {
                for p in paths {
                    check(p)?;
                }
            }
            model::AuditTarget::SecretsDir { dir } => check(dir)?,
            model::AuditTarget::FirewallState => {}
        }
    }
    Ok(())
}

fn platform_tag() -> &'static str {
    aethercore_platform_capabilities::current_platform_name()
}

/// Runs every requested lane. Unknown/None targets ⇒ empty request is an error
/// at the caller (CLI/router enforce non-empty allowlists).
pub fn run_audit(targets: &[model::AuditTarget]) -> SecurityAuditReport {
    if let Err(reason) = validate_targets(targets) {
        // Typed rejection surfaces as a single honest degraded lane.
        let lanes = vec![LaneReport {
            lane: "targets".to_string(),
            status: LaneStatus::not_available(reason),
            findings: Vec::new(),
        }];
        return SecurityAuditReport {
            schema_version: REPORT_SCHEMA_VERSION,
            platform: platform_tag().to_string(),
            digest: model::report_digest(&[]),
            lanes,
        };
    }
    let mut lanes: Vec<(String, LaneReport)> = Vec::new();
    let mut push_lane = |lane: &str, res: Result<Vec<model::SecFinding>, String>| {
        let report = match res {
            Ok(findings) => {
                let count = findings.len();
                LaneReport {
                    lane: lane.to_string(),
                    status: LaneStatus::ok(count),
                    findings,
                }
            }
            Err(reason) => LaneReport {
                lane: lane.to_string(),
                status: LaneStatus::not_available(reason),
                findings: Vec::new(),
            },
        };
        lanes.push((lane.to_string(), report));
    };

    for t in targets {
        match t {
            model::AuditTarget::SshdConfig { .. } => {
                push_lane("sshd", sshd::audit_target(t));
            }
            model::AuditTarget::PasswordPolicy { path } => {
                if cfg!(target_os = "macos") {
                    // macOS keeps password policy outside login.defs by
                    // construction (OpenDirectory); honest NotAvailable.
                    push_lane(
                        "password",
                        Err(format!(
                            "{} (requested target: {path})",
                            password::platform_note()
                        )),
                    );
                } else {
                    push_lane("password", password::audit_password_policy(path));
                }
            }
            model::AuditTarget::Sudoers { path } => {
                push_lane("sudoers", sudoers::audit_sudoers(path));
            }
            model::AuditTarget::FilesystemPaths { paths } => {
                push_lane("filesystem", filesystem::audit_filesystem(paths));
            }
            model::AuditTarget::AuthLogs { paths } => {
                for (i, log) in paths.iter().enumerate() {
                    push_lane(&format!("authlog[{i}]"), authlog::audit_auth_log(log));
                }
            }
            model::AuditTarget::SecretsDir { dir } => {
                push_lane("secrets", secrets::scan_secrets(dir));
            }
            model::AuditTarget::FirewallState => {
                let status = firewall::audit_firewall_state();
                let findings = firewall::firewall_findings(&status);
                push_lane(
                    "firewall",
                    Ok(findings.into_iter().collect::<Vec<model::SecFinding>>()),
                );
            }
        }
    }

    // CVE join runs whenever any filesystem/authlog-free host census applies:
    // it is always attempted once per audit (read-only census), degrading
    // honestly when no package source exists or the DB fails integrity.
    lanes.push(cve_lane());

    lanes.sort_by(|a, b| a.0.cmp(&b.0));
    let lane_reports: Vec<LaneReport> = lanes.into_iter().map(|(_, r)| r).collect();
    let all_findings: Vec<model::SecFinding> = lane_reports
        .iter()
        .flat_map(|l| l.findings.iter().cloned())
        .collect();
    SecurityAuditReport {
        schema_version: REPORT_SCHEMA_VERSION,
        platform: platform_tag().to_string(),
        digest: model::report_digest(&all_findings),
        lanes: lane_reports,
    }
}

/// CVE join lane: verified local DB × read-only census.
fn cve_lane() -> (String, LaneReport) {
    // Resolution order: AETHERCORE_VULNDB_DIR (owner-installed DB from
    // `vulndb update --dest`), then the seeded asset BESIDE THE EXECUTABLE (this is
    // where the MSI installs it), then the repo-relative path for in-tree dev runs.
    //
    // The exe-relative step is not cosmetic: without it an installed build resolved
    // `assets/vulndb` against the process CWD, which on a service or a shell in any
    // other directory is never the install directory, so the CVE lane reported
    // NotAvailable on every installed machine no matter what the MSI shipped.
    let dir = std::env::var_os("AETHERCORE_VULNDB_DIR")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            let beside = std::env::current_exe()
                .ok()?
                .parent()?
                .join("assets")
                .join("vulndb");
            beside.join("vulndb.json").is_file().then_some(beside)
        })
        .unwrap_or_else(|| std::path::PathBuf::from("assets/vulndb"));
    let db_path = dir.join("vulndb.json");
    let manifest_path = dir.join("vulndb.manifest.json");
    let entries = match vulndb::load_verified(&db_path, &manifest_path) {
        Ok(e) => e,
        Err(e) => {
            return (
                "cve".to_string(),
                LaneReport {
                    lane: "cve".to_string(),
                    status: LaneStatus::not_available(format!("vulndbIntegrity: {e}")),
                    findings: Vec::new(),
                },
            );
        }
    };
    let installed: Vec<vulnjoin::InstalledPackage> = census::census()
        .into_iter()
        .flat_map(|l| l.packages)
        .collect();
    let matches = vulnjoin::join(&installed, &entries);
    let findings: Vec<model::SecFinding> = matches
        .iter()
        .filter_map(|m| {
            model::SecFinding::try_new(
                "SEC-CVE-001",
                "cve.vulnerable_package",
                model::Severity::High,
                vec![model::EvidenceRef {
                    fact: format!(
                        "installed {} {} matched by {} (fixed in {})",
                        m.package, m.installed_version, m.cve_id, m.fixed
                    ),
                    observed: format!("{}=={}", m.package, m.installed_version),
                    expected_or_threshold: format!(">= {} < {}", m.fixed, m.fixed),
                    source_location: format!("census:{}", m.package),
                }],
                None,
                "sec.cve.vulnerablePackage",
                model::Confidence::Exact,
            )
        })
        .collect();
    let n = findings.len();
    (
        "cve".to_string(),
        LaneReport {
            lane: "cve".to_string(),
            status: LaneStatus::ok(n),
            findings,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn ev(loc: &str) -> EvidenceRef {
        EvidenceRef {
            fact: "PermitRootLogin yes".into(),
            observed: "permitrootlogin=yes".into(),
            expected_or_threshold: "no | prohibit-password".into(),
            source_location: loc.into(),
        }
    }

    /// Regression test for the defect recorded twice in Phase 37 (§16.4, §16.7):
    /// `platform_tag()` carried its OWN `cfg!` ladder that tested only macos and
    /// linux, so Windows -- the product's PRIMARY platform -- fell through to the
    /// `else` arm and every compliance report generated there was fingerprinted
    /// `other:<digest>`.
    ///
    /// The fix is that platform identity is derived in exactly ONE place,
    /// `aethercore_platform_capabilities::current_platform_name()`, and every
    /// caller routes through it. This test asserts both halves: the tag names the
    /// real host, and it AGREES with the shared helper by construction.
    ///
    /// Stated limit: the "never other" half is only discriminating when compiled
    /// for Windows, because that is the only target the old ladder got wrong. The
    /// agreement half is what makes a second, divergent derivation impossible to
    /// reintroduce on any target; `scripts/static_validate.py` checks the same
    /// property at the source level so it is enforced off-Windows too.
    #[test]
    fn platform_tag_names_the_host_and_never_falls_back_to_other() {
        let tag = platform_tag();

        assert_ne!(
            tag, "other",
            "platform_tag() fell back to \"other\"; a compliance report generated \
             here would be fingerprinted as an unknown platform"
        );

        // There must be no second derivation of platform identity.
        assert_eq!(
            tag,
            aethercore_platform_capabilities::current_platform_name(),
            "platform_tag() disagrees with the shared platform helper"
        );

        #[cfg(target_os = "windows")]
        assert!(
            tag.starts_with("windows"),
            "expected a windows* tag on Windows, got {tag:?}"
        );
        #[cfg(target_os = "macos")]
        assert_eq!(tag, "macos");
        #[cfg(target_os = "linux")]
        assert_eq!(tag, "linux");

        // The tag is what run_audit stamps into the report, and what
        // apps/aetherctl/src/sec.rs turns into host_fingerprint.
        assert_eq!(run_audit(&[model::AuditTarget::FirewallState]).platform, tag);
    }

    #[test]
    fn try_new_refuses_empty_evidence() {
        assert!(SecFinding::try_new(
            "X",
            "code",
            Severity::Low,
            vec![],
            None,
            "k",
            Confidence::Exact
        )
        .is_none());
    }

    #[test]
    fn digest_is_stable_and_order_insensitive() {
        let a = SecFinding::try_new(
            "A",
            "zz.code",
            Severity::Low,
            vec![ev("f:1")],
            None,
            "k.a",
            Confidence::Exact,
        )
        .unwrap();
        let b = SecFinding::try_new(
            "B",
            "aa.code",
            Severity::High,
            vec![ev("g:2")],
            None,
            "k.b",
            Confidence::Exact,
        )
        .unwrap();
        let d1 = report_digest(&[a.clone(), b.clone()]);
        let d2 = report_digest(&[b.clone(), a.clone()]);
        assert_eq!(d1, d2);
        let d3 = report_digest(&[a]);
        assert_ne!(d1, d3);
    }

    #[test]
    fn redaction_keeps_four_chars() {
        assert_eq!(
            secrets::redact("AKIAIOSFODNN7EXAMPLE"),
            "AKIA****************"
        );
        assert_eq!(secrets::redact("abc"), "***");
    }

    #[test]
    fn version_compare_basics() {
        use std::cmp::Ordering::*;
        assert_eq!(vulnjoin::compare_versions("1.2.3", "1.2.10"), Less);
        assert_eq!(vulnjoin::compare_versions("2.0", "1.9.9"), Greater);
        assert_eq!(vulnjoin::compare_versions("1.0.0", "1.0.0"), Equal);
    }

    #[test]
    fn cis_map_format_gate() {
        let good = cis_map::CisMapEntry {
            rule_code: "x".into(),
            control: "CIS L1 5.2.8".into(),
            note: String::new(),
        };
        let unmapped_ok = cis_map::CisMapEntry {
            rule_code: "y".into(),
            control: "unmapped".into(),
            note: "needs elevated lane".into(),
        };
        let bad_noteless = cis_map::CisMapEntry {
            rule_code: "z".into(),
            control: "unmapped".into(),
            note: String::new(),
        };
        let bad_fmt = cis_map::CisMapEntry {
            rule_code: "w".into(),
            control: "CIS L1 3.x".into(),
            note: String::new(),
        };
        assert!(cis_map::CisMap::entry_is_wellformed(&good));
        assert!(cis_map::CisMap::entry_is_wellformed(&unmapped_ok));
        assert!(!cis_map::CisMap::entry_is_wellformed(&bad_noteless));
        assert!(!cis_map::CisMap::entry_is_wellformed(&bad_fmt));
    }
}
