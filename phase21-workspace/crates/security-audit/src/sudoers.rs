//! Phase 32 — Sudoers parse-only provider.
//!
//! NEVER evaluates via sudo (no subprocess). Parses the named file plus one
//! level of `#includedir` expansion, flagging NOPASSWD grants and root
//! ALL=(ALL) lines with typed severity rationale in evidence.

use crate::model::{
    Confidence, EvidenceRef, MAX_FILES_PER_SCAN, MAX_PARSE_BYTES, SecFinding, Severity,
    sort_and_clamp,
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

/// An `#includedir` is honoured only when it sits in the same directory as the sudoers
/// file that named it. `/etc/sudoers` -> `/etc/sudoers.d` passes; `/etc/sudoers` ->
/// `C:\Users\someone-else` does not.
fn includedir_is_beside(file: &Path, includedir: &Path) -> bool {
    let Some(parent) = file.parent() else {
        return false;
    };
    !includedir
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
        && includedir.parent() == Some(parent)
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
            // The include target comes from FILE CONTENT, not from the request, so the
            // router's owner-scope allowlist has not vetted it. An unconfined
            // `#includedir` inside a file the caller owns would hand a LocalSystem read
            // of any directory straight back — the same defect one level of indirection
            // away. Confine expansion to the directory holding the file being parsed,
            // which is where every real sudoers layout puts it (/etc/sudoers ->
            // /etc/sudoers.d), and refuse anything else.
            if !includedir_is_beside(path, dir_path) {
                continue;
            }
            let Ok(entries) = std::fs::read_dir(dir_path) else {
                continue; // absent includedir is not an error
            };
            let mut sub: Vec<_> = entries.flatten().map(|e| e.path()).collect();
            sub.sort();
            for s in sub {
                let Ok(meta) = std::fs::symlink_metadata(&s) else {
                    continue;
                };
                // Skipped, never followed — see the note in secrets.rs::collect_files.
                if crate::scope::is_reparse_point(&meta) {
                    continue;
                }
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
            )
        {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `#includedir` names a directory taken from FILE CONTENT, so the router's
    /// owner-scope allowlist never saw it. Left unconfined it re-opens the whole
    /// LocalSystem read-anywhere defect through a file the caller legitimately owns.
    #[test]
    fn includedir_expansion_cannot_leave_the_directory_of_the_file_that_named_it() {
        let root = std::env::temp_dir().join(format!(
            "aethercore-sudoers-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let owned = root.join("owned");
        let beside = owned.join("sudoers.d");
        let elsewhere = root.join("elsewhere");
        std::fs::create_dir_all(&beside).expect("fixture");
        std::fs::create_dir_all(&elsewhere).expect("fixture");
        // Same NOPASSWD line in both, so only the include decision can differ.
        let grant = "someone ALL=(ALL) NOPASSWD: ALL\n";
        std::fs::write(beside.join("local"), grant).expect("fixture");
        std::fs::write(elsewhere.join("local"), grant).expect("fixture");

        let allowed = owned.join("sudoers-allowed");
        std::fs::write(&allowed, format!("#includedir {}\n", beside.display())).expect("fixture");
        let refused = owned.join("sudoers-refused");
        std::fs::write(&refused, format!("#includedir {}\n", elsewhere.display()))
            .expect("fixture");

        let found = audit_sudoers(allowed.to_string_lossy().as_ref()).expect("scan");
        assert!(
            found.iter().any(|f| f.code == "sudo.nopasswd"),
            "an includedir beside the file is still expanded: {found:?}"
        );
        let refused_findings = audit_sudoers(refused.to_string_lossy().as_ref()).expect("scan");
        assert!(
            refused_findings.is_empty(),
            "an includedir pointing outside the file's own directory must not be read: \
             {refused_findings:?}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
