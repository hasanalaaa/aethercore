//! Phase 32 (T7) — `sec` / `vulndb` offline commands.
//!
//! STRICTLY read-only EXCEPT one explicit owner action: `vulndb update`
//! installs a caller-provided DB + freshly pinned manifest into an
//! owner-named directory (keys-generate precedent: never implicit).

use crate::error::CliError;
use aethercore_security_audit as sec;

/// Renders a SecurityAuditReport as the canonical JSON value.
pub fn report_to_value(report: &sec::SecurityAuditReport) -> serde_json::Value {
    serde_json::to_value(report).unwrap_or(serde_json::json!({"error": "serialize"}))
}


/// Offline `sec audit`: builds typed targets from (kind, path) pairs and runs
/// the read-only lanes directly (no service, no network).
pub fn run_offline_audit(targets: &[(String, String)]) -> Result<serde_json::Value, CliError> {
    let mut parsed: Vec<sec::model::AuditTarget> = Vec::new();
    for (kind, path) in targets {
        let t = match kind.as_str() {
            "ssh" => sec::model::AuditTarget::SshdConfig { path: path.clone() },
            "sudoers" => sec::model::AuditTarget::Sudoers { path: path.clone() },
            "fs" => sec::model::AuditTarget::FilesystemPaths {
                paths: vec![path.clone()],
            },
            "authlog" => sec::model::AuditTarget::AuthLogs {
                paths: vec![path.clone()],
            },
            "secrets" => sec::model::AuditTarget::SecretsDir { dir: path.clone() },
            "firewall" => sec::model::AuditTarget::FirewallState,
            other => {
                return Err(CliError::Usage {
                    message_key: "cli.usage.unknownSubcommand".to_string(),
                    detail: Some(format!("unknown audit target kind '{other}'")),
                });
            }
        };
        parsed.push(t);
    }
    if let Err(detail) = sec::validate_targets(&parsed) {
        return Err(CliError::Rejected {
            message_key: "sec.traversalRejected".to_string(),
            detail: Some(detail),
        });
    }
    let report = sec::run_audit(&parsed);
    Ok(report_to_value(&report))
}

/// Offline `sec report --in <file>`: load + re-render a saved report.
/// The JSON value carries everything; text mode consumers get `render_report_text`
/// through `render::finish` formatting of the same value.
pub fn render_saved_report(file: &str) -> Result<serde_json::Value, CliError> {
    let raw = std::fs::read(file).map_err(|e| CliError::LocalIo {
        message_key: "local.io.read".to_string(),
        detail: Some(format!("read report: {e}")),
    })?;
    let parsed: Result<sec::SecurityAuditReport, serde_json::Error> = serde_json::from_slice(&raw);
    let report = match parsed {
        Ok(r) => r,
        Err(e) => {
            return Err(CliError::LocalIo {
                message_key: "sec.reportInvalid".to_string(),
                detail: Some(format!("parse report: {e}")),
            });
        }
    };
    Ok(report_to_value(&report))
}

/// Offline `compliance summary --profile cis-l1 --report <f> --map <f>`.
pub fn run_compliance_summary(
    profile: &str,
    report_file: &str,
    map_file: &str,
) -> Result<serde_json::Value, CliError> {
    if profile != "cis-l1" {
        return Err(CliError::Usage {
            message_key: "cli.usage.unknownSubcommand".to_string(),
            detail: Some(format!(
                "unsupported compliance profile '{profile}' (only cis-l1)"
            )),
        });
    }
    let raw = std::fs::read(report_file).map_err(|e| CliError::LocalIo {
        message_key: "local.io.read".to_string(),
        detail: Some(format!("read report: {e}")),
    })?;
    let parsed: Result<sec::SecurityAuditReport, serde_json::Error> = serde_json::from_slice(&raw);
    let report = match parsed {
        Ok(r) => r,
        Err(e) => {
            return Err(CliError::LocalIo {
                message_key: "sec.reportInvalid".to_string(),
                detail: Some(format!("parse report: {e}")),
            });
        }
    };
    let map =
        sec::cis_map::load(std::path::Path::new(map_file)).map_err(|e| CliError::LocalIo {
            message_key: "sec.cisMapInvalid".to_string(),
            detail: Some(e),
        })?;
    compliance_summary(&report, &map)
}

/// Rule code → owning lane id (providers evaluate ALL their rules whenever
/// their lane runs, so absence-of-finding + lane-ok ⇒ compliant).
fn owning_lane(rule_code: &str) -> &'static str {
    if rule_code.starts_with("ssh.") {
        "sshd"
    } else if rule_code.starts_with("sudo.") {
        "sudoers"
    } else if rule_code.starts_with("fs.") {
        "filesystem"
    } else if rule_code.starts_with("auth.") {
        "authlog"
    } else if rule_code.starts_with("secrets.") {
        "secrets"
    } else if rule_code.starts_with("fw.") {
        "firewall"
    } else if rule_code.starts_with("cve.") {
        "cve"
    } else if rule_code.starts_with("pass.") {
        "password"
    } else {
        ""
    }
}

/// cis-l1 compliance summary over a saved report: per-control-family
/// PASS / FAIL / NOT-RUN table. Anti-snake-oil: PASS requires the rule's lane
/// to have run OK in THIS report; everything else is honestly not-run.
pub fn compliance_summary(
    report: &sec::SecurityAuditReport,
    map: &sec::cis_map::CisMap,
) -> Result<serde_json::Value, CliError> {
    // Rule code → (count, max severity) from all lanes' findings.
    let mut failed: std::collections::BTreeMap<&str, usize> = Default::default();
    for l in &report.lanes {
        for f in &l.findings {
            *failed.entry(f.code.as_str()).or_default() += 1;
        }
    }
    let lane_ran_ok = |lane: &str| {
        report
            .lanes
            .iter()
            .any(|l| l.lane == lane && matches!(l.status, sec::LaneStatus::Ok { .. }))
    };
    #[derive(Clone)]
    struct Row {
        control: String,
        status: &'static str,
        findings: usize,
    }
    let mut rows: Vec<Row> = Vec::new();
    for entry in &map.entries {
        if !sec::cis_map::CisMap::entry_is_wellformed(entry) {
            return Err(CliError::LocalIo {
                message_key: "sec.cisMapInvalid".to_string(),
                detail: Some(format!("bad cis_map entry {}", entry.rule_code)),
            });
        }
        if entry.control == "unmapped" {
            rows.push(Row {
                control: "unmapped".into(),
                status: "unmapped",
                findings: 0,
            });
            continue;
        }
        // Control family = first two numeric groups ("5.2" of "CIS L1 5.2.8").
        let nums = entry
            .control
            .strip_prefix("CIS L1 ")
            .or_else(|| entry.control.strip_prefix("CIS L2 "))
            .unwrap_or(&entry.control);
        let parts: Vec<&str> = nums.split('.').collect();
        let family = if parts.len() >= 2 {
            format!("{}.{}", parts[0], parts[1])
        } else {
            nums.to_string()
        };
        let lane = owning_lane(&entry.rule_code);
        let count = failed.get(entry.rule_code.as_str()).copied().unwrap_or(0);
        if count > 0 {
            rows.push(Row {
                control: family,
                status: "FAIL",
                findings: count,
            });
        } else if !lane.is_empty() && lane_ran_ok(lane) {
            rows.push(Row {
                control: family,
                status: "PASS",
                findings: 0,
            });
        } else {
            rows.push(Row {
                control: family,
                status: "NOT_RUN",
                findings: 0,
            });
        }
    }
    // Aggregate by family keeping the worst status (FAIL > NOT_RUN > PASS).
    let order = |s: &str| match s {
        "FAIL" => 0,
        "NOT_RUN" => 1,
        "PASS" => 2,
        _ => 3,
    };
    let mut agg: std::collections::BTreeMap<String, Row> = Default::default();
    for r in rows {
        let e = agg.entry(r.control.clone()).or_insert(r.clone());
        if order(r.status) < order(e.status) {
            *e = r.clone();
        } else {
            e.findings += r.findings;
        }
    }
    let table: Vec<serde_json::Value> = agg
        .into_iter()
        .map(|(family, r)| {
            serde_json::json!({
                "family": family,
                "status": r.status,
                "findings": r.findings,
            })
        })
        .collect();
    Ok(serde_json::json!({
        "profile": map.profile,
        "digest": report.digest,
        "controls": table,
    }))
}

/// Parse-only validation of a candidate DB (no writes).
fn validate_candidate_db(raw: &[u8]) -> Result<(), String> {
    let entries: Vec<sec::vulndb::VulnEntry> =
        serde_json::from_slice(raw).map_err(|e| format!("candidate db parse: {e}"))?;
    if entries.is_empty() {
        return Err("candidate db is empty".to_string());
    }
    Ok(())
}

/// Explicit owner action: install a candidate vulndb + fresh manifest pin.
/// THE ONLY writer in the sec surface — scoped exactly like export.rs (P29):
/// validate-first, single destination dir, rollback on pin failure.
pub fn vulndb_update_from(file: &str, dest_dir: &str) -> Result<serde_json::Value, CliError> {
    use sha2::Digest as _;
    let raw = std::fs::read(file).map_err(|e| CliError::LocalIo {
        message_key: "local.io.read".to_string(),
        detail: Some(format!("read candidate db: {e}")),
    })?;
    // Validate BEFORE anything is written (fail-closed even for owner actions).
    if let Err(e) = validate_candidate_db(&raw) {
        return Err(CliError::LocalIo {
            message_key: "sec.vulndbInvalidCandidate".to_string(),
            detail: Some(e),
        });
    }
    let digest = format!("{:x}", sha2::Sha256::digest(&raw));
    let manifest = serde_json::json!({
        "schema": "aethercore.vulndb.manifest.v1",
        "entries": count_entries(&raw)?,
        "sha256": digest,
    });
    let dest = std::path::Path::new(dest_dir);
    std::fs::create_dir_all(dest).map_err(|e| CliError::LocalIo {
        message_key: "local.io.write".to_string(),
        detail: Some(format!("create dest dir: {e}")),
    })?;
    let db_path = dest.join("vulndb.json");
    let manifest_path = dest.join("vulndb.manifest.json");
    std::fs::write(&db_path, &raw).map_err(|e| CliError::LocalIo {
        message_key: "local.io.write".to_string(),
        detail: Some(format!("write db: {e}")),
    })?;
    let pretty = serde_json::to_vec_pretty(&manifest);
    let wrote = pretty
        .ok()
        .map(|bytes| std::fs::write(&manifest_path, bytes));
    if wrote.is_none() || matches!(wrote, Some(Err(_))) {
        // Roll back the db copy so no unpinned artifact remains.
        let _ = std::fs::remove_file(&db_path);
        return Err(CliError::LocalIo {
            message_key: "sec.vulndbManifestWrite".to_string(),
            detail: Some("manifest serialization/write failed; db copy rolled back".into()),
        });
    }
    Ok(serde_json::json!({
        "installed": true,
        "db": db_path.display().to_string(),
        "manifest": manifest_path.display().to_string(),
    }))
}

/// Counts DB entries during validation (single parse, shared shape check).
fn count_entries(raw: &[u8]) -> Result<usize, CliError> {
    let entries: Vec<sec::vulndb::VulnEntry> =
        serde_json::from_slice(raw).map_err(|e| CliError::LocalIo {
            message_key: "sec.vulndbInvalidCandidate".to_string(),
            detail: Some(format!("candidate db parse: {e}")),
        })?;
    Ok(entries.len())
}
