//! Phase 32 — Auth-log failure-burst provider (deterministic).
//!
//! Parses auth.log/secure-style lines with syslog timestamps; counts failed
//! authentication attempts per source within a sliding window. Pure text
//! parsing — no journald, no elevation.

use crate::model::{
    sort_and_clamp, Confidence, EvidenceRef, SecFinding, Severity, MAX_PARSE_BYTES,
};
use std::collections::BTreeMap;
use std::path::Path;

/// Failure burst threshold (attempts within window).
pub const BURST_THRESHOLD: usize = 10;
/// Sliding window seconds.
pub const WINDOW_SECS: u64 = 300;

const FAIL_MARKERS: [&str; 6] = [
    "Failed password",
    "authentication failure",
    "Invalid user",
    "Connection closed by authenticating user",
    "error: maximum authentication attempts exceeded",
    "pam_unix(sudo:auth): authentication failure",
];

/// Minimal syslog front timestamp → seconds-of-day (UTC assumed); lines that
/// don't start with a month-day-time triple are skipped deterministically.
fn parse_syslog_secs(line: &str) -> Option<u64> {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let parts: Vec<&str> = line.split_whitespace().take(3).collect();
    if parts.len() < 3 {
        return None;
    }
    let m = MONTHS.iter().position(|&x| x == parts[0])? as u64;
    let d: u64 = parts[1].parse().ok()?;
    let hms: Vec<&str> = parts[2].split(':').collect();
    if hms.len() != 3 {
        return None;
    }
    let h: u64 = hms[0].parse().ok()?;
    let mi: u64 = hms[1].parse().ok()?;
    let s: u64 = hms[2].parse().ok()?;
    // Day-granular ordering is enough for windowed bursts inside one file.
    Some((((m * 31 + d) * 24 + h) * 60 + mi) * 60 + s)
}

fn is_failure(line: &str) -> bool {
    FAIL_MARKERS.iter().any(|m| line.contains(m))
}

/// Extracts a source token (host/IP after 'from') when present.
fn extract_source(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    match lower.find(" from ") {
        Some(pos) => line[pos + 6..]
            .split_whitespace()
            .next()
            .unwrap_or("unknown")
            .trim_end_matches(':').trim_end_matches(',')
            .to_string(),
        None => "unknown".to_string(),
    }
}

pub fn audit_auth_log(path_str: &str) -> Result<Vec<SecFinding>, String> {
    let path = Path::new(path_str);
    let meta = std::fs::metadata(path).map_err(|e| format!("stat: {e}"))?;
    if !meta.is_file() {
        return Err(format!("not a regular file: {path_str}"));
    }
    if meta.len() > MAX_PARSE_BYTES as u64 {
        return Err(format!("oversize: {} > {MAX_PARSE_BYTES}", meta.len()));
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("read: {e}"))?;
    // per-source: sorted list of (secs, line_no, verbatim)
    let mut events: BTreeMap<String, Vec<(u64, usize, String)>> = BTreeMap::new();
    for (idx, raw) in text.lines().enumerate() {
        if !is_failure(raw) {
            continue;
        }
        if let Some(secs) = parse_syslog_secs(raw) {
            events
                .entry(extract_source(raw))
                .or_default()
                .push((secs, idx + 1, raw.to_string()));
        }
    }
    let mut findings = Vec::new();
    for (source, mut evs) in events {
        if evs.len() < BURST_THRESHOLD {
            continue;
        }
        // Sliding window over sorted timestamps; report the densest window once
        // per source (deterministic: earliest densest).
        evs.sort_by_key(|(s, n, _)| (*s, *n));
        let mut best: Option<(usize, usize)> = None; // (count, start_idx)
        let mut j = 0usize;
        for i in 0..evs.len() {
            while evs[j].0 + WINDOW_SECS < evs[i].0 {
                j += 1;
            }
            let count = i - j + 1;
            if count >= BURST_THRESHOLD && best.map(|(c, _)| count > c).unwrap_or(true) {
                best = Some((count, j));
            }
        }
        if let Some((count, start)) = best {
            let first = &evs[start];
            let last_idx = (start + count - 1).min(evs.len() - 1);
            let last = &evs[last_idx];
            if let Some(f) = SecFinding::try_new(
                "SEC-AUTH-001",
                "auth.failure_burst",
                Severity::Advisory,
                vec![EvidenceRef {
                    fact: first.2.clone(),
                    observed: format!("{count} failures from '{source}' within {WINDOW_SECS}s"),
                    expected_or_threshold: format!(
                        "< {BURST_THRESHOLD} failures per {WINDOW_SECS}s"
                    ),
                    source_location: format!("{path_str}:{}", first.1),
                }],
                None,
                "sec.auth.failureBurst",
                Confidence::Heuristic,
            )
            .map(|mut f| {
                // Second evidence ref cites the window's final event.
                f.evidence.push(EvidenceRef {
                    fact: last.2.clone(),
                    observed: format!("window end at line {}", last.1),
                    expected_or_threshold: format!("{WINDOW_SECS}s window"),
                    source_location: format!("{path_str}:{}", last.1),
                });
                f
            }) {
                findings.push(f);
            }
        }
    }
    Ok(sort_and_clamp(findings))
}
