//! Phase 32 — Version comparison + CVE join engine (air-gapped).
//!
//! Joins a read-only installed-package census against the verified local DB.
//! Unknown packages are ignored; every match cites the installed version
//! verbatim alongside the DB `fixed` version.

use crate::model::MAX_FINDINGS;
use crate::vulndb::VulnEntry;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// One installed-package census row (parsed read-only from platform state).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledPackage {
    pub name: String,
    pub version: String,
}

/// A joined vulnerable-package result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VulnMatch {
    pub cve_id: String,
    pub package: String,
    /// Installed version, verbatim from the census.
    pub installed_version: String,
    /// First fixed version per the local DB.
    pub fixed: String,
    pub summary: String,
}

/// Tokenizes versions into numeric / alpha runs and compares run-wise.
/// Numeric runs compare numerically; alpha runs ASCII-lexically; a numeric run
/// orders before an alpha run at the same position (rpmvercmp-like, stable).
fn tokenize(v: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut cur_digit: Option<bool> = None;
    for ch in v.chars() {
        if ch.is_ascii_alphanumeric() {
            let d = ch.is_ascii_digit();
            if cur_digit == Some(!d) && !cur.is_empty() {
                tokens.push(std::mem::take(&mut cur));
            }
            cur_digit = Some(d);
            cur.push(ch);
        } else {
            if !cur.is_empty() {
                tokens.push(std::mem::take(&mut cur));
                cur_digit = None;
            }
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    tokens
}

fn cmp_token(a: &str, b: &str) -> Ordering {
    let (an, bn) = (a.parse::<u64>(), b.parse::<u64>());
    match (an, bn) {
        (Ok(x), Ok(y)) => x.cmp(&y),
        (Ok(_), Err(_)) => Ordering::Less,
        (Err(_), Ok(_)) => Ordering::Greater,
        (Err(_), Err(_)) => a.cmp(b),
    }
}

/// Deterministic three-way version comparison over the tokenizer above.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let (ta, tb) = (tokenize(a), tokenize(b));
    let n = ta.len().max(tb.len());
    for i in 0..n {
        let (x, y) = (
            ta.get(i).map(|s| s.as_str()).unwrap_or(""),
            tb.get(i).map(|s| s.as_str()).unwrap_or(""),
        );
        match cmp_token(x, y) {
            Ordering::Equal => continue,
            other => return other,
        }
    }
    Ordering::Equal
}

fn version_in_range(version: &str, introduced: &str, fixed: &str) -> bool {
    let ge_intro = introduced.is_empty() || compare_versions(version, introduced) != Ordering::Less;
    let lt_fixed = !fixed.is_empty() && compare_versions(version, fixed) == Ordering::Less;
    ge_intro && lt_fixed
}

/// Joins census × DB. Unknown packages are ignored; output sorted by
/// (cve_id, package) and clamped to `MAX_FINDINGS`.
pub fn join(installed: &[InstalledPackage], db: &[VulnEntry]) -> Vec<VulnMatch> {
    let mut out: Vec<VulnMatch> = Vec::new();
    for pkg in installed {
        if pkg.name.is_empty() || pkg.version.is_empty() {
            continue;
        }
        for e in db {
            if e.package != pkg.name {
                continue;
            }
            if version_in_range(&pkg.version, &e.introduced, &e.fixed) {
                out.push(VulnMatch {
                    cve_id: e.cve_id.clone(),
                    package: e.package.clone(),
                    installed_version: pkg.version.clone(),
                    fixed: e.fixed.clone(),
                    summary: e.summary.clone(),
                });
            }
        }
    }
    out.sort_by(|a, b| {
        a.cve_id
            .cmp(&b.cve_id)
            .then_with(|| a.package.cmp(&b.package))
    });
    out.dedup_by(|a, b| a.cve_id == b.cve_id && a.package == b.package);
    // House clamp ceiling so one noisy census cannot flood the report lane.
    if out.len() > MAX_FINDINGS {
        out.truncate(MAX_FINDINGS);
    }
    out
}
