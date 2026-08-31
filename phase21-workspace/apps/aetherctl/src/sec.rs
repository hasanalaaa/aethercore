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

/// CIS baselines ship COMPILED IN, not as files beside the binary.
///
/// This used to resolve `env!("CARGO_MANIFEST_DIR")/../../assets/compliance/profiles`,
/// an absolute path on the machine that BUILT the binary, so `sec audit` could never
/// find a profile on any other machine. `profile_id` is already constrained to
/// `cis-l1|cis-l2` by the caller, so there are exactly two possible artifacts;
/// embedding them removes the resolution problem instead of relocating it.
fn compliance_profile_bytes(profile_id: &str) -> &'static [u8] {
    match profile_id {
        "cis-l2" => include_bytes!("../../../assets/compliance/profiles/cis-l2.json"),
        _ => include_bytes!("../../../assets/compliance/profiles/cis-l1.json"),
    }
}

/// Phase 33 compliance report pipeline: direct, offline, and explicit about signing.
pub fn run_compliance_audit(
    profile_id: &str,
    out: &str,
    format: &str,
    sign: bool,
    key_path: Option<&str>,
    targets: &[(String, String)],
) -> Result<serde_json::Value, CliError> {
    if !matches!(profile_id, "cis-l1" | "cis-l2") {
        return Err(usage_error(format!("unsupported profile '{profile_id}'")));
    }
    if !matches!(format, "json" | "html" | "both") {
        return Err(usage_error(format!("unsupported report format '{format}'")));
    }
    if sign != key_path.is_some() {
        return Err(usage_error(
            "--sign and explicit --key <seedfile> must be supplied together".to_string(),
        ));
    }

    let parsed = if targets.is_empty() {
        default_live_targets()
    } else {
        parse_targets(targets)?
    };
    sec::validate_targets(&parsed).map_err(|detail| CliError::Rejected {
        message_key: "sec.traversalRejected".to_string(),
        detail: Some(detail),
    })?;
    let audit = sec::run_audit(&parsed);
    let profile = sec::compliance::Profile::from_bytes(compliance_profile_bytes(profile_id))
        .map_err(|error| CliError::LocalIo {
            message_key: "sec.complianceProfile".to_string(),
            detail: Some(error.to_string()),
        })?;
    let generated_unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| CliError::LocalIo {
            message_key: "sec.reportInvalid".to_string(),
            detail: Some(format!("system clock before epoch: {error}")),
        })?
        .as_millis() as i64;
    let host_fingerprint = format!("{}:{}", audit.platform, audit.digest);
    let mut report =
        sec::compliance::evaluate(&profile, &audit, host_fingerprint, generated_unix_ms)
            .map_err(compliance_error)?;

    if let Some(path) = key_path {
        let raw = std::fs::read_to_string(path).map_err(|error| CliError::LocalIo {
            message_key: "local.io.read".to_string(),
            detail: Some(format!("read signing key: {error}")),
        })?;
        let seed = decode_seed(raw.trim()).map_err(|detail| CliError::LocalIo {
            message_key: "local.keys.badKeyFile".to_string(),
            detail: Some(detail),
        })?;
        let signing_key = aethercore_persistence::export::signing_key_from_seed(&seed);
        let signature = aethercore_persistence::export::sign_digest(&report.digest, &signing_key);
        report.signed = true;
        report.signature = Some(sec::compliance::ReportSignature {
            public_key_hex: signature.public_key_hex,
            signature_hex: signature.signature_hex,
        });
    }

    let value = serde_json::to_value(&report).map_err(|error| CliError::LocalIo {
        message_key: "sec.reportInvalid".to_string(),
        detail: Some(format!("serialize report: {error}")),
    })?;
    if matches!(format, "json" | "both") {
        let json = serde_json::to_vec_pretty(&report).map_err(|error| CliError::LocalIo {
            message_key: "sec.reportInvalid".to_string(),
            detail: Some(format!("serialize report: {error}")),
        })?;
        std::fs::write(out, json).map_err(|error| CliError::LocalIo {
            message_key: "local.io.write".to_string(),
            detail: Some(format!("write JSON report: {error}")),
        })?;
    }
    if matches!(format, "html" | "both") {
        let html_path = if format == "both" {
            std::path::Path::new(out).with_extension("html")
        } else {
            std::path::PathBuf::from(out)
        };
        std::fs::write(&html_path, sec::compliance::render_html(&report)).map_err(|error| {
            CliError::LocalIo {
                message_key: "local.io.write".to_string(),
                detail: Some(format!("write HTML report: {error}")),
            }
        })?;
    }
    Ok(value)
}

pub fn verify_compliance_report(file: &str) -> Result<serde_json::Value, CliError> {
    let raw = std::fs::read(file).map_err(|error| CliError::LocalIo {
        message_key: "local.io.read".to_string(),
        detail: Some(format!("read compliance report: {error}")),
    })?;
    let report = sec::compliance::parse_report_bytes(&raw).map_err(compliance_error)?;
    sec::compliance::verify_integrity(&report).map_err(compliance_error)?;
    if let Some(signature) = &report.signature {
        let signature = aethercore_persistence::export::ExportSignature {
            public_key_hex: signature.public_key_hex.clone(),
            signature_hex: signature.signature_hex.clone(),
        };
        aethercore_persistence::export::verify_digest_signature(&report.digest, &signature)
            .map_err(|_| CliError::LocalIo {
                message_key: "sec.signatureMismatch".to_string(),
                detail: Some("signature mismatch".to_string()),
            })?;
    }
    Ok(serde_json::json!({
        "verified": true,
        "signed": report.signed,
        "digest": report.digest,
        "profile": report.profile_id,
    }))
}

fn parse_targets(targets: &[(String, String)]) -> Result<Vec<sec::model::AuditTarget>, CliError> {
    targets
        .iter()
        .map(|(kind, path)| match kind.as_str() {
            "ssh" => Ok(sec::model::AuditTarget::SshdConfig { path: path.clone() }),
            "sudoers" => Ok(sec::model::AuditTarget::Sudoers { path: path.clone() }),
            "fs" => Ok(sec::model::AuditTarget::FilesystemPaths {
                paths: vec![path.clone()],
            }),
            "authlog" => Ok(sec::model::AuditTarget::AuthLogs {
                paths: vec![path.clone()],
            }),
            "secrets" => Ok(sec::model::AuditTarget::SecretsDir { dir: path.clone() }),
            "firewall" => Ok(sec::model::AuditTarget::FirewallState),
            _ => Err(usage_error(format!("unknown audit target kind '{kind}'"))),
        })
        .collect()
}

fn default_live_targets() -> Vec<sec::model::AuditTarget> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/unknown".to_string());
    vec![
        sec::model::AuditTarget::SshdConfig {
            path: "/etc/ssh/sshd_config".to_string(),
        },
        sec::model::AuditTarget::Sudoers {
            path: "/etc/sudoers".to_string(),
        },
        sec::model::AuditTarget::FilesystemPaths { paths: vec![home] },
        sec::model::AuditTarget::FirewallState,
    ]
}

fn usage_error(detail: String) -> CliError {
    CliError::Usage {
        message_key: "cli.usage.compliance".to_string(),
        detail: Some(detail),
    }
}

fn compliance_error(error: sec::compliance::ComplianceError) -> CliError {
    CliError::LocalIo {
        message_key: "sec.reportInvalid".to_string(),
        detail: Some(error.to_string()),
    }
}

fn decode_seed(input: &str) -> Result<[u8; 32], String> {
    if input.len() != 64 {
        return Err("seed must contain exactly 64 hexadecimal characters".to_string());
    }
    let mut seed = [0_u8; 32];
    for (index, chunk) in input.as_bytes().chunks(2).enumerate() {
        let high = (chunk[0] as char)
            .to_digit(16)
            .ok_or_else(|| "invalid seed encoding".to_string())? as u8;
        let low = (chunk[1] as char)
            .to_digit(16)
            .ok_or_else(|| "invalid seed encoding".to_string())? as u8;
        seed[index] = (high << 4) | low;
    }
    Ok(seed)
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

#[cfg(test)]
mod phase33_tests {
    use super::*;

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("aethercore-p33-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create isolated test directory");
        path
    }

    #[test]
    fn gd2_and_gd3_report_determinism_signature_tamper_and_unsigned_honesty() {
        let dir = temp_dir("lifecycle");
        let ssh = dir.join("sshd_config");
        std::fs::write(
            &ssh,
            "PermitRootLogin yes\nPasswordAuthentication yes\nPubkeyAuthentication yes\n",
        )
        .expect("write fixture");
        let key = dir.join("owner.seed");
        std::fs::write(&key, format!("{}\n", "09".repeat(32))).expect("write test seed");
        let signed_path = dir.join("signed.json");
        let targets = vec![("ssh".to_string(), ssh.display().to_string())];

        let signed = run_compliance_audit(
            "cis-l1",
            &signed_path.display().to_string(),
            "both",
            true,
            Some(&key.display().to_string()),
            &targets,
        )
        .expect("signed report");
        assert_eq!(signed["signed"], true);
        let verified = verify_compliance_report(&signed_path.display().to_string())
            .expect("verify signed report");
        assert_eq!(verified["verified"], true);
        assert_eq!(verified["signed"], true);

        let html = std::fs::read_to_string(signed_path.with_extension("html"))
            .expect("read standalone HTML");
        assert!(!html.contains("http://"));
        assert!(!html.contains("https://"));
        assert!(html.contains("dir=\"rtl\""));

        let mut tampered: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&signed_path).expect("read signed report"))
                .expect("parse signed report");
        tampered["host_fingerprint"] = serde_json::json!("tampered");
        let tampered_path = dir.join("tampered.json");
        std::fs::write(
            &tampered_path,
            serde_json::to_vec_pretty(&tampered).expect("serialize tamper"),
        )
        .expect("write tampered report");
        let error = verify_compliance_report(&tampered_path.display().to_string())
            .expect_err("tamper must fail");
        assert!(format!("{error:?}").contains("digest mismatch"));

        let mut signature_tampered: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&signed_path).expect("read signed report"))
                .expect("parse signed report");
        let signature = signature_tampered["signature"]["signature_hex"]
            .as_str()
            .expect("signature hex");
        let replacement = if signature.starts_with('0') { "1" } else { "0" };
        let tampered_hex = format!("{replacement}{}", &signature[1..]);
        signature_tampered["signature"]["signature_hex"] = serde_json::json!(tampered_hex);
        let signature_tampered_path = dir.join("signature-tampered.json");
        std::fs::write(
            &signature_tampered_path,
            serde_json::to_vec_pretty(&signature_tampered).expect("serialize signature tamper"),
        )
        .expect("write signature tamper");
        let signature_error =
            verify_compliance_report(&signature_tampered_path.display().to_string())
                .expect_err("signature tamper must fail");
        assert!(format!("{signature_error:?}").contains("signature mismatch"));

        let unsigned_path = dir.join("unsigned.json");
        let unsigned = run_compliance_audit(
            "cis-l1",
            &unsigned_path.display().to_string(),
            "json",
            false,
            None,
            &targets,
        )
        .expect("unsigned report");
        assert_eq!(unsigned["signed"], false);
        assert!(unsigned.get("signature").is_none());
        let unsigned_verified = verify_compliance_report(&unsigned_path.display().to_string())
            .expect("verify unsigned integrity");
        assert_eq!(unsigned_verified["signed"], false);

        let signed_report: sec::compliance::ComplianceReport =
            serde_json::from_slice(&std::fs::read(&signed_path).expect("read report first time"))
                .expect("parse first report");
        let second_path = dir.join("second.json");
        let second = run_compliance_audit(
            "cis-l1",
            &second_path.display().to_string(),
            "json",
            false,
            None,
            &targets,
        )
        .expect("second semantic report");
        assert_eq!(signed_report.digest, second["digest"]);

        println!("GD-2 digest identical: {}", signed_report.digest);
        println!("GD-3 signed verification: PASS");
        println!("GD-3 content tamper: {error:?}");
        println!("GD-3 signature tamper: {signature_error:?}");
        println!("GD-3 unsigned integrity: PASS signed=false signature=absent");

        std::fs::remove_dir_all(dir).expect("remove isolated test directory");
    }
}
