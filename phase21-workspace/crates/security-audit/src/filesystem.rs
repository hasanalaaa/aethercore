//! Phase 32 — Filesystem posture provider (read-only, bounded walk).
//!
//! Flags: world-writable files outside sanctioned system sets, SUID binaries
//! inventory, `.ssh` directory/key permission violations, home-dir exposure.
//! The walk is depth- and wall-clock-bounded; hitting a bound degrades the
//! lane honestly instead of pretending completeness.

use crate::model::{
    Confidence, EvidenceRef, MAX_FILES_PER_SCAN, MAX_SCAN_MILLIS, SecFinding, Severity,
    sort_and_clamp,
};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Directories whose world-writable entries are sanctioned by the OS baseline.
const SANCTIONED_PREFIXES: [&str; 5] = [
    "/tmp",
    "/private/var/tmp",
    "/System/Volumes/Data/private/tmp",
    "/dev",
    "/Library/Caches",
];

struct WalkBudget {
    files_visited: usize,
    deadline: Instant,
    truncated: bool,
}

fn is_sanctioned(path: &str) -> bool {
    SANCTIONED_PREFIXES
        .iter()
        .any(|p| path == *p || path.starts_with(&format!("{p}/")))
}

fn mode_bits(meta: &std::fs::Metadata) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode()
    }
    #[cfg(windows)]
    {
        // Windows has no POSIX mode bits; report read-only as the single
        // security-relevant distinction this provider consumes.
        let readonly = meta.permissions().readonly();
        0o444 | if readonly { 0 } else { 0o222 }
    }
}

fn walk(dir: &Path, budget: &mut WalkBudget, findings_dirs: &mut Vec<PathBuf>) {
    if budget.files_visited >= MAX_FILES_PER_SCAN || Instant::now() > budget.deadline {
        budget.truncated = true;
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for p in paths {
        budget.files_visited += 1;
        if budget.files_visited >= MAX_FILES_PER_SCAN || Instant::now() > budget.deadline {
            budget.truncated = true;
            return;
        }
        let Ok(meta) = std::fs::symlink_metadata(&p) else {
            continue;
        };
        // Skipped, never followed — see the note in secrets.rs::collect_files.
        if crate::scope::is_reparse_point(&meta) {
            continue;
        }
        if meta.is_dir() {
            findings_dirs.push(p.clone());
            walk(&p, budget, findings_dirs);
        }
    }
}

/// Collects candidate regular-file paths under `roots` within bounds.
fn collect_files(roots: &[String]) -> (Vec<PathBuf>, bool) {
    let mut out = Vec::new();
    let mut budget = WalkBudget {
        files_visited: 0,
        deadline: Instant::now() + std::time::Duration::from_millis(MAX_SCAN_MILLIS),
        truncated: false,
    };
    for root in roots {
        let p = Path::new(root);
        match std::fs::metadata(p) {
            Ok(m) if m.is_file() => {
                out.push(p.to_path_buf());
                budget.files_visited += 1;
            }
            _ => {
                let mut dirs = Vec::new();
                walk(p, &mut budget, &mut dirs);
                for d in dirs {
                    if let Ok(entries) = std::fs::read_dir(&d) {
                        for e in entries.flatten() {
                            budget.files_visited += 1;
                            if budget.files_visited >= MAX_FILES_PER_SCAN
                                || Instant::now() > budget.deadline
                            {
                                budget.truncated = true;
                                break;
                            }
                            let ep = e.path();
                            if let Ok(m) = std::fs::symlink_metadata(&ep)
                                && !crate::scope::is_reparse_point(&m)
                                && m.is_file()
                            {
                                out.push(ep);
                            }
                        }
                    }
                    if budget.truncated {
                        break;
                    }
                }
            }
        }
        if budget.truncated {
            break;
        }
    }
    (out, budget.truncated)
}

pub fn audit_filesystem(roots: &[String]) -> Result<Vec<SecFinding>, String> {
    let (files, truncated) = collect_files(roots);
    let mut findings = Vec::new();
    let mut suid_count = 0usize;
    let mut ww_count = 0usize;
    for f in &files {
        let Ok(meta) = std::fs::symlink_metadata(f) else {
            continue;
        };
        let mode = mode_bits(&meta);
        let shown = f.display().to_string();
        // SUID inventory (informational, bounded).
        if mode & 0o4000 != 0 {
            suid_count += 1;
            if let Some(fd) = SecFinding::try_new(
                "SEC-FS-002",
                "fs.suid_inventory",
                Severity::Advisory,
                vec![EvidenceRef {
                    fact: format!("SUID binary present: {shown}"),
                    observed: format!("mode={:04o}", mode & 0o7777),
                    expected_or_threshold: "inventory only — review against baseline".to_string(),
                    source_location: shown.clone(),
                }],
                None,
                "sec.fs.suidInventory",
                Confidence::Exact,
            ) {
                findings.push(fd);
            }
        }
        // World-writable outside sanctioned sets.
        if mode & 0o002 != 0 && !is_sanctioned(&shown) {
            ww_count += 1;
            if let Some(fw) = SecFinding::try_new(
                "SEC-FS-001",
                "fs.world_writable",
                Severity::Medium,
                vec![EvidenceRef {
                    fact: format!("world-writable file outside sanctioned set: {shown}"),
                    observed: format!("mode={:04o}", mode & 0o7777),
                    expected_or_threshold: "no group/other write bits".to_string(),
                    source_location: shown.clone(),
                }],
                Some("CIS L1 6.1.1"),
                "sec.fs.worldWritable",
                Confidence::Exact,
            ) {
                findings.push(fw);
            }
        }
    }
    // .ssh posture per root that IS a home-like dir.
    for root in roots {
        let ssh_dir = Path::new(root).join(".ssh");
        // symlink_metadata, not metadata: a `.ssh` that is itself a junction must be
        // skipped rather than silently resolved to someone else's key material.
        let Ok(meta) = std::fs::symlink_metadata(&ssh_dir) else {
            continue;
        };
        if crate::scope::is_reparse_point(&meta) || !meta.is_dir() {
            continue;
        }
        let dmode = mode_bits(&meta);
        if dmode & 0o077 != 0
            && let Some(f) = SecFinding::try_new(
                "SEC-FS-003",
                "fs.ssh_dir_perms",
                Severity::High,
                vec![EvidenceRef {
                    fact: format!(".ssh directory over-permissive: {}", ssh_dir.display()),
                    observed: format!("dir mode={:04o}", dmode & 0o7777),
                    expected_or_threshold: "0700 on ~/.ssh".to_string(),
                    source_location: ssh_dir.display().to_string(),
                }],
                None,
                "sec.fs.sshDirPerms",
                Confidence::Exact,
            )
        {
            findings.push(f);
        }
        if let Ok(entries) = std::fs::read_dir(&ssh_dir) {
            for e in entries.flatten() {
                let p = e.path();
                let Ok(fm) = std::fs::symlink_metadata(&p) else {
                    continue;
                };
                if crate::scope::is_reparse_point(&fm) || !fm.is_file() {
                    continue;
                }
                let fmode = mode_bits(&fm);
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let is_key =
                    name.starts_with("id_") || name.ends_with(".pem") || name == "authorized_keys";
                if is_key
                    && fmode & 0o077 != 0
                    && let Some(fk) = SecFinding::try_new(
                        "SEC-FS-004",
                        "fs.ssh_key_perms",
                        Severity::Critical,
                        vec![EvidenceRef {
                            fact: format!("SSH key material over-permissive: {}", p.display()),
                            observed: format!("file mode={:04o}", fmode & 0o7777),
                            expected_or_threshold: "0600 on keys / authorized_keys".to_string(),
                            source_location: p.display().to_string(),
                        }],
                        None,
                        "sec.fs.sshKeyPerms",
                        Confidence::Exact,
                    )
                {
                    findings.push(fk);
                }
            }
        }
    }
    let _ = (suid_count, ww_count);
    let mut all = sort_and_clamp(findings);
    if truncated {
        // Honest truncation marker as an Advisory finding with evidence naming
        // the bound that was hit.
        if let Some(ft) = SecFinding::try_new(
            "SEC-FS-900",
            "fs.scan_truncated",
            Severity::Advisory,
            vec![EvidenceRef {
                fact: format!(
                    "filesystem scan hit a bound after {} entries or {}ms",
                    MAX_FILES_PER_SCAN, MAX_SCAN_MILLIS
                ),
                observed: "truncated=true".to_string(),
                expected_or_threshold: "full coverage within bounds".to_string(),
                source_location: roots.first().cloned().unwrap_or_default(),
            }],
            None,
            "sec.fs.scanTruncated",
            Confidence::Inferred,
        ) {
            all.push(ft);
        }
    }
    Ok(sort_and_clamp(all))
}
