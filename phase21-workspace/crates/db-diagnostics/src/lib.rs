//! Phase 30 (T1) — DATABASE DIAGNOSTICS DOMAIN: READ-ONLY diagnostics with
//! evidence-cited advisory findings.
//!
//! Phase contract (binding):
//! - ZERO mutations against any scanned database. No VACUUM, no REPAIR, no writes of
//!   any kind. SQLite targets open STRICTLY `SQLITE_OPEN_READ_ONLY` with
//!   `busy_timeout = 0` (fail fast — never hold or wait on a live application's locks).
//! - Every finding cites measured facts (`EvidenceRef`). A rule lacking its full
//!   evidence matrix emits NOTHING (Phases 17.1/20 discipline).
//! - A lane that cannot honestly run is declared NotAvailable(typed reason) — never
//!   simulated.
//!
//! Deterministic report digest over sorted findings (byte-stable for identical inputs).

pub mod config_lint;
pub mod model;
pub mod slow_log;
pub mod sqlite_provider;

pub use model::{Confidence, DbFinding, DiagnosticReport, DiagnosticTarget, EvidenceRef, Severity};

// ---------------------------------------------------------------------------
// Phase 29/30 — fuzz facades (used by fuzz/ targets; also handy for tooling).
// Each wraps one untrusted-input parser with production capacity bounds.
// ---------------------------------------------------------------------------

/// Parses raw bytes as PostgreSQL config directives.
pub fn parse_pg_conf(data: &[u8]) -> usize {
    let mut n = 0usize;
    for line in String::from_utf8_lossy(data)
        .lines()
        .take(config_lint::MAX_DIRECTIVES)
    {
        if crate::config_lint::parse_pg_line_public(line).is_some() {
            n += 1;
        }
    }
    n
}

/// Parses raw bytes as MySQL config directives.
pub fn parse_mysql_conf(data: &[u8]) -> usize {
    let mut n = 0usize;
    for line in String::from_utf8_lossy(data)
        .lines()
        .take(config_lint::MAX_DIRECTIVES)
    {
        if crate::config_lint::parse_my_line_public(line).is_some() {
            n += 1;
        }
    }
    n
}

/// Parses raw bytes as a PostgreSQL slow log; returns entry count.
pub fn parse_pg_slow_log(data: &[u8]) -> usize {
    let tmp = std::env::temp_dir().join(format!("p30fuzz-pg-{}", std::process::id()));
    std::fs::write(&tmp, data).ok();
    let r = slow_log::analyze_postgres_log(tmp.to_str().unwrap_or(""), 10);
    let _ = std::fs::remove_file(&tmp);
    r.map(|_| 1).unwrap_or(0)
}

/// Parses raw bytes as a MySQL slow log; returns entry count.
pub fn parse_mysql_slow_log(data: &[u8]) -> usize {
    let tmp = std::env::temp_dir().join(format!("p30fuzz-my-{}", std::process::id()));
    std::fs::write(&tmp, data).ok();
    let r = slow_log::analyze_mysql_log(tmp.to_str().unwrap_or(""), 10);
    let _ = std::fs::remove_file(&tmp);
    r.map(|_| 1).unwrap_or(0)
}
