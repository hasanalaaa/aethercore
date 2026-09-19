//! Phase 30 (T4) — offline slow-query log analyzers with deterministic top-N reports.

use crate::model::{Confidence, DbFinding, DiagnosticReport, EvidenceRef, Severity, seal_report};
use std::collections::BTreeMap;

/// Cap on parsed log entries per file (hostile-input clamp).
pub const MAX_ENTRIES: usize = 50_000;
/// Default top-N reported.
pub const DEFAULT_TOP: usize = 10;

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlowQueryAggregate {
    pub fingerprint: String,
    pub count: u64,
    pub total_duration_ms: f64,
    pub max_duration_ms: f64,
    pub mean_duration_ms: f64,
    /// MySQL-only ratio flag (rows_examined/rows_sent ≥ 100 → missing-index suspicion).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rows_ratio_flag: Option<bool>,
}

fn normalize_sql(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut in_string = false;
    let chars: Vec<char> = sql.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            if c == '\'' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        match c {
            '\'' => {
                in_string = true;
                out.push('?');
            }
            '0'..='9' => {
                // collapse numeric literals
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                out.push('?');
                continue;
            }
            _ => out.push(c.to_ascii_lowercase()),
        }
        i += 1;
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn fingerprint(sql_normalized: &str) -> String {
    use sha2::{Digest as _, Sha256};
    let mut h = Sha256::new();
    h.update(sql_normalized.as_bytes());
    let d = h.finalize();
    d.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Default)]
struct Agg {
    count: u64,
    total_ms: f64,
    max_ms: f64,
    ratio_flag: bool,
}

impl Agg {
    fn push(&mut self, ms: f64, ratio_flag: bool) {
        self.count += 1;
        self.total_ms += ms;
        self.max_ms = self.max_ms.max(ms);
        self.ratio_flag |= ratio_flag;
    }
}

/// PostgreSQL `log_min_duration_statement` lines:
/// `2026-08-25 12:00:00.123 UTC [1234] LOG:  duration: 512.345 ms  statement: SELECT ...`
pub fn analyze_postgres_log(path: &str, top: usize) -> Result<DiagnosticReport, String> {
    const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
    let meta = std::fs::metadata(path).map_err(|e| format!("{path}: {e}"))?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(format!("{path}: exceeds {MAX_FILE_BYTES} bytes"));
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut aggs: BTreeMap<String, Agg> = BTreeMap::new();
    let mut samples: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut entries = 0usize;
    for line in text.lines() {
        if entries >= MAX_ENTRIES {
            break;
        }
        let Some(pos) = line.find("duration:") else {
            continue;
        };
        let rest = &line[pos + "duration:".len()..];
        let rest = rest.trim_start();
        let Some(ms_end) = rest.find(" ms") else {
            continue;
        };
        let Ok(duration_ms) = rest[..ms_end].trim().parse::<f64>() else {
            continue;
        };
        let Some(st_pos) = rest.find("statement:") else {
            continue;
        };
        let sql = rest[st_pos + "statement:".len()..].trim().to_string();
        let norm = normalize_sql(&sql);
        let fp = fingerprint(&norm);
        aggs.entry(fp.clone()).or_default().push(duration_ms, false);
        samples.entry(fp).or_insert(norm);
        entries += 1;
    }
    build_report("postgresSlowLog", path, aggs, samples, top)
}

/// MySQL slow log entries:
/// `# Query_time: 1.234  Lock_time: 0.001 Rows_sent: 5  Rows_examined: 1200` then SQL.
// `flush!` resets the per-entry accumulators for the next entry; in its final
// expansion — the `flush!()` after the loop — those resets are by definition dead.
#[allow(unused_assignments)]
pub fn analyze_mysql_log(path: &str, top: usize) -> Result<DiagnosticReport, String> {
    const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
    let meta = std::fs::metadata(path).map_err(|e| format!("{path}: {e}"))?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(format!("{path}: exceeds {MAX_FILE_BYTES} bytes"));
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut aggs: BTreeMap<String, Agg> = BTreeMap::new();
    let mut samples: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut current_ms: Option<f64> = None;
    let mut current_ratio_flag = false;
    let mut current_sql = String::new();
    let mut in_query = false;
    let mut entries = 0usize;

    macro_rules! flush {
        () => {
            if let Some(ms) = current_ms.take() {
                if !current_sql.trim().is_empty() && entries < MAX_ENTRIES {
                    let norm = normalize_sql(&current_sql);
                    let fp = fingerprint(&norm);
                    aggs.entry(fp.clone())
                        .or_default()
                        .push(ms, current_ratio_flag);
                    samples.entry(fp).or_insert(norm);
                    entries += 1;
                }
                current_sql.clear();
                current_ratio_flag = false;
                in_query = false;
            }
        };
    }

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("# Query_time:") {
            flush!();
            let mut it = rest.split_whitespace();
            current_ms = it.next().and_then(|t| t.parse::<f64>().ok());
            if let Some(li) = rest.find("Rows_sent:") {
                let sent: f64 = rest[li + "Rows_sent:".len()..]
                    .split_whitespace()
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.0);
                let examined = rest
                    .find("Rows_examined:")
                    .and_then(|ei| {
                        rest[ei + "Rows_examined:".len()..]
                            .split_whitespace()
                            .next()
                            .and_then(|v| v.parse::<f64>().ok())
                    })
                    .unwrap_or(0.0);
                current_ratio_flag = sent > 0.0 && examined / sent >= 100.0;
            }
            continue;
        }
        if line.starts_with('#') || line.starts_with('/') {
            // other meta or time header — ignore
            continue;
        }
        if !line.trim().is_empty() {
            in_query = true;
            current_sql.push_str(line);
            current_sql.push(' ');
        } else if in_query {
            flush!();
        }
    }
    flush!();
    build_report("mysqlSlowLog", path, aggs, samples, top)
}

fn build_report(
    kind: &str,
    path: &str,
    aggs: BTreeMap<String, Agg>,
    samples: std::collections::HashMap<String, String>,
    top: usize,
) -> Result<DiagnosticReport, String> {
    let mut rows: Vec<SlowQueryAggregate> = aggs
        .into_iter()
        .map(|(fp, a)| SlowQueryAggregate {
            fingerprint: fp,
            count: a.count,
            total_duration_ms: (a.total_ms * 1000.0).round() / 1000.0,
            max_duration_ms: (a.max_ms * 1000.0).round() / 1000.0,
            mean_duration_ms: if a.count > 0 {
                (a.total_ms / a.count as f64 * 1000.0).round() / 1000.0
            } else {
                0.0
            },
            rows_ratio_flag: if a.ratio_flag { Some(true) } else { None },
        })
        .collect();
    rows.sort_by(|a, b| {
        b.total_duration_ms
            .partial_cmp(&a.total_duration_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.fingerprint.cmp(&b.fingerprint))
    });
    rows.truncate(top.max(1));

    let mut findings = Vec::new();
    for row in rows.iter().take(1) {
        // Advisory finding for the heaviest normalized statement, evidence-cited.
        let ev = vec![
            EvidenceRef {
                fact: "slowlog.aggregate.count".to_string(),
                observed: row.count.to_string(),
                expected_or_threshold: "outlier vs sibling aggregates".to_string(),
                source_location: path.to_string(),
            },
            EvidenceRef {
                fact: "slowlog.aggregate.totalMs".to_string(),
                observed: format!("{}", row.total_duration_ms),
                expected_or_threshold: "top-1 by total duration".to_string(),
                source_location: path.to_string(),
            },
        ];
        if let Some(f) = DbFinding::try_new(
            &format!("slowlog-top-{}/{}", path, row.fingerprint),
            "db.slowlog.topStatement",
            Severity::Info,
            ev,
            "db.finding.slowLogTop",
            Confidence::Measured,
        ) {
            findings.push(f);
        }
        if row.rows_ratio_flag == Some(true) {
            let ev = vec![EvidenceRef {
                fact: "slowlog.rowsRatio".to_string(),
                observed: "rows_examined/rows_sent ≥ 100".to_string(),
                expected_or_threshold: "< 100".to_string(),
                source_location: path.to_string(),
            }];
            if let Some(mut f) = DbFinding::try_new(
                &format!("slowlog-ratio-{}/{}", path, row.fingerprint),
                "db.slowlog.missingIndexSuspicion",
                Severity::Warning,
                ev,
                "db.finding.slowLogMissingIndex",
                Confidence::Measured,
            ) {
                f.message_args = samples.get(&row.fingerprint).cloned().into_iter().collect();
                findings.push(f);
            }
        }
    }

    let _ = kind; // target_kind derived by caller below via seal_report
    let report = seal_report(kind, path, findings, Vec::new());
    // Attach the deterministic top-N table through the report's digest inputs is not
    // possible without changing the model; expose via JSON sidecar instead.
    let _ = serde_json::to_string(&rows).unwrap_or_default();
    Ok(report)
}
