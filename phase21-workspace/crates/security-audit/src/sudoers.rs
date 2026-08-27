//! Phase 32 — Sudoers parse-only provider.
//!
//! NEVER evaluates via sudo (no subprocess). Parses the named file plus one
//! level of `#includedir` expansion, flagging NOPASSWD grants and root
//! ALL=(ALL) lines with typed severity rationale in evidence.

use crate::model::{
    sort_and_clamp, Confidence, EvidenceRef, SecFinding, Severity, MAX_FILES_PER_SCAN,
    MAX_PARSE_BYTES,
};
use std::path::Path;

fn read_capped(path: &Path) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("stat: {e}"))?;
    if !meta.is_file() {
        return Err(format!("not a regular file: {}", path.display()));
    }
    if meta.len() > MAX_PARSE_BYTES as u64 {
        return Err(format!("oversize: {} > {MAX_PARSE_BYTES}", path.display()));
    }
    std::fs::read_to_string(path).map_err(|e| format!("read: {e}"))
}

fn push_nopasswd(findings: &mut Vec<SecFinding>, loc: &str, line_no: usize, verbatim: &str) {
    if let Some(f) = SecFinding::try_new(
        "SEC-SUDO-001",
        "sudo.nopasswd",
        Severity::Advisory,
        vec![EvidenceRef {
            fact: verbatim.to_string(),
            observed: "NOPASSWD grant present".to_string(),
            expected_or_threshold:
                "password re-authentication for privileged ops (or documented exception)"
                    .to_string(),
            source_location: format!("{loc}:{line_no}"),
        }],
        Some("CIS L1 1.3.4"),
        "sec.sudo.noPasswd",
        Confidence::Exact,
    ) {
        findings.push(f);
    }
}

fn scan_one_file(
    path_str: &str,
    findings: &mut Vec<SecFinding>,
    visited: &mut Vec<std::path::PathBuf>,
) -> Result<(), String> {
    let path = Path::new(path_str);
    if visited.contains(&path.to_path_buf()) {
        return Ok(());
    }
    visited.push(path.to_path_buf());
    if visited.len() > MAX_FILES_PER_SCAN {
        return Err("includedir expansion exceeded scan cap".to_string());
    }
    let text = read_capped(path)?;
    for (idx, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') && !line.starts_with("#includedir") {
            continue;
        }
        if let Some(rest) = line.strip_prefix("#includedir") {
            let dir = rest.trim();
            let dir_path = Path::new(dir);
            let Ok(entries) = std::fs::read_dir(dir_path) else {
                continue; // absent includedir is not an error
            };
            let mut sub: Vec<_> = entries.flatten().map(|e| e.path()).collect();
            sub.sort();
            for s in sub {
                if let Some(n) = s.file_name().and_then(|n| n.to_str())
                    && !n.contains('.')
                {
                    scan_one_file(s.to_string_lossy().as_ref(), findings, visited)?;
                }
            }
            continue;
        }
        if line.contains("NOPASSWD") {
            push_nopasswd(findings, path_str, idx + 1, line);
        }
        // root ALL=(ALL) without password constraints is normal sudoers shape;
        // flag only ALL=(ALL) NOPASSWD variants (covered above) and wildcard
        // user grants.
        if line.contains("ALL=(ALL:ALL)")
            && !line.contains("NOPASSWD")
            && line
                .split(':')
                .next()
                .map(|u| u.trim() == "ALL")
                .unwrap_or(false)
            && let Some(f) = SecFinding::try_new(
                "SEC-SUDO-002",
                "sudo.wildcard_all",
                Severity::Low,
                vec![EvidenceRef {
                    fact: line.to_string(),
                    observed: "unrestricted ALL=(ALL:ALL) grant".to_string(),
                    expected_or_threshold: "least-privilege command list per grantee".to_string(),
                    source_location: format!("{path_str}:{}", idx + 1),
                }],
                None,
                "sec.sudo.wildcardAll",
                Confidence::Inferred,
            ) {
                findings.push(f);
        }
    }
    Ok(())
}

pub fn audit_sudoers(path_str: &str) -> Result<Vec<SecFinding>, String> {
    let mut findings = Vec::new();
    let mut visited = Vec::new();
    scan_one_file(path_str, &mut findings, &mut visited)?;
    Ok(sort_and_clamp(findings))
}
