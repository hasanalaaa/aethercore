//! Phase 32 — Offline secrets scanner (Trivy-inspired heuristics).
//!
//! Regex + Shannon-entropy heuristics over an EXPLICITLY named target
//! directory (never whole-disk). Findings cite file:line; matched secret
//! content is REDACTED to its first 4 chars in evidence — the privacy
//! contract (tested in GD-2 and enforced by the audit gate).

use crate::model::{
    sort_and_clamp, Confidence, EvidenceRef, SecFinding, Severity, MAX_EVIDENCE_REFS,
    MAX_FILES_PER_SCAN, MAX_PARSE_BYTES, MAX_SCAN_MILLIS,
};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;

/// Skipped file extensions (binaries/media).
const SKIP_EXTENSIONS: [&str; 14] = [
    "png", "jpg", "jpeg", "gif", "webp", "ico", "pdf", "zip", "gz", "tar", "bin", "dylib", "so",
    "dll",
];

// Each detector degrades honestly to None if its regex fails to construct
// (never panics); scanning continues with the remaining detectors.
type LazyRegex = OnceLock<Option<Regex>>;

fn aws_re() -> Option<&'static Regex> {
    static RE: LazyRegex = LazyRegex::new();
    RE.get_or_init(|| Regex::new(r"(AKIA|ASIA)[0-9A-Z]{16}").ok())
        .as_ref()
}

fn private_key_re() -> Option<&'static Regex> {
    static RE: LazyRegex = LazyRegex::new();
    RE.get_or_init(|| {
        Regex::new(
            r"-----BEGIN ((?:RSA |EC |DSA |OPENSSH |PGP |ENCRYPTED )?)PRIVATE KEY( BLOCK)?-----",
        )
        .ok()
    })
    .as_ref()
}

fn generic_assignment_re() -> Option<&'static Regex> {
    static RE: LazyRegex = LazyRegex::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"(?i)(api[_-]?key|secret|token|passwd|password)["']?\s*[:=]\s*["']?([A-Za-z0-9+/_\-=]{20,})["']?"#,
        )
        .ok()
    })
    .as_ref()
}

fn dotenv_re() -> Option<&'static Regex> {
    static RE: LazyRegex = LazyRegex::new();
    RE.get_or_init(|| {
        Regex::new(r"^([A-Z][A-Z0-9_]*(KEY|SECRET|TOKEN|PASSWORD)[A-Z0-9_]*)=(\S{8,})$").ok()
    })
    .as_ref()
}

/// Redaction contract: keep the first 4 characters, mask the rest.
pub fn redact(matched: &str) -> String {
    let n = matched.chars().count();
    if n <= 4 {
        return "*".repeat(n);
    }
    let head: String = matched.chars().take(4).collect();
    format!("{head}{}", "*".repeat(n - 4))
}

fn shannon_entropy(s: &str) -> f64 {
    if s.is_empty() {
        return 0.0;
    }
    let mut counts = [0u64; 256];
    for b in s.bytes() {
        counts[b as usize] += 1;
    }
    let len = s.len() as f64;
    counts
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / len;
            -p * p.log2()
        })
        .sum()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    AwsKey,
    PrivateKeyBlock,
    GenericAssignment,
    DotenvSecret,
}

impl Kind {
    fn code(self) -> &'static str {
        match self {
            Kind::AwsKey => "secrets.aws_key",
            Kind::PrivateKeyBlock => "secrets.private_key_block",
            Kind::GenericAssignment => "secrets.generic_assignment",
            Kind::DotenvSecret => "secrets.dotenv",
        }
    }
    fn id(self) -> &'static str {
        match self {
            Kind::AwsKey => "SEC-SEC-001",
            Kind::PrivateKeyBlock => "SEC-SEC-002",
            Kind::GenericAssignment => "SEC-SEC-003",
            Kind::DotenvSecret => "SEC-SEC-004",
        }
    }
    fn severity(self) -> Severity {
        match self {
            Kind::AwsKey | Kind::PrivateKeyBlock => Severity::Critical,
            Kind::GenericAssignment | Kind::DotenvSecret => Severity::High,
        }
    }
    fn summary_key(self) -> &'static str {
        match self {
            Kind::AwsKey => "sec.secrets.awsKey",
            Kind::PrivateKeyBlock => "sec.secrets.privateKey",
            Kind::GenericAssignment => "sec.secrets.genericAssignment",
            Kind::DotenvSecret => "sec.secrets.dotenv",
        }
    }
}

struct Hit {
    kind: Kind,
    line_no: usize,
    /// Exact secret substring at match time; crosses into evidence ONLY
    /// through `redact()` in the evidence-construction block below.
    raw: String,
}

fn scan_line(kind: Kind, line: &str, line_no: usize, hits: &mut Vec<Hit>) {
    match kind {
        Kind::AwsKey => {
            if let Some(re) = aws_re() {
                for m in re.find_iter(line) {
                    hits.push(Hit {
                        kind,
                        line_no,
                        raw: m.as_str().to_string(),
                    });
                }
            }
        }
        Kind::PrivateKeyBlock => {
            if let Some(re) = private_key_re()
                && re.is_match(line)
            {
                hits.push(Hit {
                    kind,
                    line_no,
                    // Header marker only; key BODY is never captured.
                    raw: "-----BEGIN ... PRIVATE KEY-----".to_string(),
                });
            }
        }
        Kind::GenericAssignment => {
            if let Some(re) = generic_assignment_re() {
                for c in re.captures_iter(line) {
                    if let Some(val) = c.get(2) {
                        let v = val.as_str();
                        // Dual gate for generic assignments only:
                        //  a) mixed character classes (random credential
                        //     shape, not prose passphrases like
                        //     correcthorsebatterystaple);
                        //  b) Shannon entropy >= 4.0 AND length >= 24.
                        let has_upper = v.chars().any(|c| c.is_ascii_uppercase());
                        let has_lower_or_digit =
                            v.chars()
                                .any(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
                        if has_upper
                            && has_lower_or_digit
                            && v.chars().count() >= 24
                            && shannon_entropy(v) >= 4.0
                        {
                            hits.push(Hit {
                                kind,
                                line_no,
                                raw: v.to_string(),
                            });
                        }
                        break; // one hit per line per kind
                    }
                }
            }
        }
        Kind::DotenvSecret => {
            if let Some(re) = dotenv_re()
                && let Some(caps) = re.captures(line)
                && let Some(val) = caps.get(3)
            {
                hits.push(Hit {
                    kind,
                    line_no,
                    raw: val.as_str().to_string(),
                });
            }
        }
    }
}

/// Walks the target dir within bounds; reports whether a bound was hit.
fn collect_files(root: &Path) -> (Vec<PathBuf>, bool) {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    let mut visited = 0usize;
    let deadline = Instant::now() + std::time::Duration::from_millis(MAX_SCAN_MILLIS);
    while let Some(dir) = stack.pop() {
        if visited >= MAX_FILES_PER_SCAN || Instant::now() > deadline {
            return (out, true);
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for p in paths {
            visited += 1;
            if visited >= MAX_FILES_PER_SCAN || Instant::now() > deadline {
                return (out, true);
            }
            let Ok(meta) = std::fs::symlink_metadata(&p) else {
                continue;
            };
            if meta.is_dir() {
                stack.push(p);
            } else if meta.is_file() {
                let skip = p
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| SKIP_EXTENSIONS.contains(&x.to_ascii_lowercase().as_str()))
                    .unwrap_or(false);
                if !skip && meta.len() <= MAX_PARSE_BYTES as u64 {
                    out.push(p);
                }
            }
        }
    }
    (out, false)
}

pub fn scan_secrets(dir_str: &str) -> Result<Vec<SecFinding>, String> {
    let root = Path::new(dir_str);
    let meta = std::fs::metadata(root).map_err(|e| format!("stat: {e}"))?;
    if !meta.is_dir() {
        return Err(format!("not a directory: {dir_str}"));
    }
    let (files, truncated) = collect_files(root);
    let mut findings = Vec::new();
    let kinds = [
        Kind::AwsKey,
        Kind::PrivateKeyBlock,
        Kind::GenericAssignment,
        Kind::DotenvSecret,
    ];
    for f in &files {
        let Ok(text) = std::fs::read_to_string(f) else {
            continue; // binary/undecodable files are skipped deterministically
        };
        let shown = f.display().to_string();
        let mut hits: Vec<Hit> = Vec::new();
        for (idx, line) in text.lines().enumerate() {
            for k in kinds {
                scan_line(k, line, idx + 1, &mut hits);
            }
        }
        // One finding per kind per file; evidence refs carry REDACTED facts only.
        for k in kinds {
            let khits: Vec<&Hit> = hits.iter().filter(|h| h.kind == k).collect();
            if khits.is_empty() {
                continue;
            }
            let evidence: Vec<EvidenceRef> = khits
                .iter()
                .take(MAX_EVIDENCE_REFS)
                .map(|h| EvidenceRef {
                    // Privacy contract: first 4 chars, remainder masked.
                    fact: redact(&h.raw),
                    observed: format!("{:?} pattern matched at line {}", h.kind, h.line_no),
                    expected_or_threshold: "no credential-shaped material in scanned tree"
                        .to_string(),
                    source_location: format!("{shown}:{}", h.line_no),
                })
                .collect();
            if let Some(sf) = SecFinding::try_new(
                k.id(),
                k.code(),
                k.severity(),
                evidence,
                None,
                k.summary_key(),
                Confidence::Exact,
            ) {
                findings.push(sf);
            }
        }
    }
    if truncated
        && let Some(ft) = SecFinding::try_new(
            "SEC-SEC-900",
            "secrets.scan_truncated",
            Severity::Advisory,
            vec![EvidenceRef {
                fact: format!(
                    "secrets scan hit a bound after {} entries or {}ms",
                    MAX_FILES_PER_SCAN, MAX_SCAN_MILLIS
                ),
                observed: "truncated=true".to_string(),
                expected_or_threshold: "full coverage within bounds".to_string(),
                source_location: dir_str.to_string(),
            }],
            None,
            "sec.secrets.scanTruncated",
            Confidence::Inferred,
        ) {
            findings.push(ft);
    }
    Ok(sort_and_clamp(findings))
}
