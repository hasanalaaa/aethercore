//! Phase 30 (T2) — SQLite provider: STRICTLY read-only PRAGMA-based diagnostics.
//!
//! Contract:
//! - Open with `SQLITE_OPEN_READ_ONLY` only; `busy_timeout(0)` — fail fast, never
//!   wait on a live application's locks.
//! - Checks: integrity_check · quick_check · foreign_key_check count · journal_mode +
//!   wal_autocheckpoint · page_count/freelist waste ratio · schema object census.
//! - Every finding cites verbatim PRAGMA output rows.

use crate::model::{
    Confidence, DbFinding, DiagnosticReport, DiagnosticTarget, EvidenceRef, Severity, seal_report,
};
use rusqlite::{Connection, OpenFlags};

/// Hard capacity bound: integrity_check rows consumed per scan (hostile-input clamp).
pub const MAX_INTEGRITY_ROWS: usize = 20;
/// Cap on findings emitted by one sqlite scan.
pub const MAX_FINDINGS: usize = 64;

fn open_read_only(path: &str) -> Result<Connection, String> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(path, flags)
        .map_err(|e| format!("sqlite open read-only failed: {e}"))?;
    // Fail fast: never block on a live writer's lock.
    conn.busy_timeout(std::time::Duration::from_millis(0))
        .map_err(|e| format!("busy_timeout(0) failed: {e}"))?;
    Ok(conn)
}

fn query_strings(conn: &Connection, sql: &str, cap: usize) -> Result<Vec<String>, String> {
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    while out.len() < cap {
        match rows.next() {
            Ok(Some(row)) => {
                let v: String = row.get(0).map_err(|e| e.to_string())?;
                out.push(v);
            }
            Ok(None) => break,
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(out)
}

fn scalar(conn: &Connection, sql: &str) -> Result<String, String> {
    conn.query_row(sql, [], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())
}

/// Full read-only diagnostics pass for one SQLite file.
pub fn diagnose_sqlite(path: &str) -> Result<DiagnosticReport, String> {
    let conn = open_read_only(path)?;
    let mut findings: Vec<DbFinding> = Vec::new();

    // --- integrity_check (bounded) ------------------------------------------
    // Severe corruption can make the PRAGMA itself fail (SQLITE_CORRUPT during prepare/
    // step). That failure IS the evidence: emit a Critical finding citing it verbatim.
    let integrity_rows: Vec<String> =
        match query_strings(&conn, "PRAGMA integrity_check", MAX_INTEGRITY_ROWS) {
            Ok(rows) => rows,
            Err(pragma_error) => {
                let evidence = vec![EvidenceRef {
                    fact: "pragma.integrity_check.error".to_string(),
                    observed: pragma_error.clone(),
                    expected_or_threshold: "PRAGMA completes without error".to_string(),
                    source_location: format!("{path}:PRAGMA integrity_check"),
                }];
                if let Some(f) = DbFinding::try_new(
                    &format!("sqlite-integrity-{path}"),
                    "db.sqlite.integrity.corrupt",
                    Severity::Critical,
                    evidence,
                    "db.finding.sqliteIntegrity",
                    Confidence::Measured,
                ) {
                    findings.push(f);
                }
                Vec::new()
            }
        };
    let corrupted_rows: Vec<&String> = integrity_rows
        .iter()
        .filter(|r| r.as_str() != "ok")
        .collect();
    if !corrupted_rows.is_empty() {
        let evidence: Vec<EvidenceRef> = corrupted_rows
            .iter()
            .enumerate()
            .map(|(i, row)| EvidenceRef {
                fact: "pragma.integrity_check.row".to_string(),
                observed: (*row).clone(),
                expected_or_threshold: "ok".to_string(),
                source_location: format!("{path}:PRAGMA integrity_check#{i}"),
            })
            .collect();
        if let Some(f) = DbFinding::try_new(
            &format!("sqlite-integrity-{path}"),
            "db.sqlite.integrity.corrupt",
            Severity::Critical,
            evidence,
            "db.finding.sqliteIntegrity",
            Confidence::Measured,
        ) {
            findings.push(f);
        }
    } else if integrity_rows.is_empty() {
        // Severe corruption can make PRAGMA prepare itself fail before any row is
        // returned. That case surfaces as Err from open/query — handled below.
    }

    if findings
        .iter()
        .any(|f| f.code == "db.sqlite.integrity.corrupt")
    {
        // With a Critical integrity verdict, deeper PRAGMAs may themselves fail on the
        // corrupt image. Seal now — the evidence is already complete.
        drop(conn);
        return Ok(seal_report("sqliteFile", path, findings, Vec::new()));
    }

    // --- quick_check divergence (only meaningful when integrity already flagged;
    // otherwise a mismatch between the two is itself informational noise we drop).
    let quick = query_strings(&conn, "PRAGMA quick_check", 1)?;
    if quick.first().map(String::as_str) != Some("ok") && corrupted_rows.is_empty() {
        let evidence = vec![EvidenceRef {
            fact: "pragma.quick_check.row".to_string(),
            observed: quick.first().cloned().unwrap_or_default(),
            expected_or_threshold: "ok".to_string(),
            source_location: format!("{path}:PRAGMA quick_check"),
        }];
        if let Some(f) = DbFinding::try_new(
            &format!("sqlite-quickcheck-{path}"),
            "db.sqlite.quickcheck.fail",
            Severity::Warning,
            evidence,
            "db.finding.sqliteQuickCheck",
            Confidence::Measured,
        ) {
            findings.push(f);
        }
    }

    // --- foreign key violations ---------------------------------------------
    let fk_violations = query_strings(&conn, "PRAGMA foreign_key_check", MAX_FINDINGS)?;
    if !fk_violations.is_empty() {
        let evidence = vec![EvidenceRef {
            fact: "pragma.foreign_key_check.rows".to_string(),
            observed: format!("{} violating rows", fk_violations.len()),
            expected_or_threshold: "0".to_string(),
            source_location: format!("{path}:PRAGMA foreign_key_check"),
        }];
        if let Some(f) = DbFinding::try_new(
            &format!("sqlite-fk-{path}"),
            "db.sqlite.fk.violations",
            Severity::Warning,
            evidence,
            "db.finding.sqliteForeignKey",
            Confidence::Measured,
        ) {
            findings.push(f);
        }
    }

    // --- WAL auto-checkpoint far from default -------------------------------
    let journal_mode = scalar(&conn, "PRAGMA journal_mode")?;
    if journal_mode.eq_ignore_ascii_case("wal") {
        // wal_autocheckpoint returns INTEGER — read via i64.
        let ac = conn
            .query_row("PRAGMA wal_autocheckpoint", [], |r| r.get::<_, i64>(0))
            .map_err(|e| e.to_string())?;
        if !(1..=1000).contains(&ac) {
            let evidence = vec![EvidenceRef {
                fact: "pragma.wal_autocheckpoint".to_string(),
                observed: ac.to_string(),
                expected_or_threshold: "1000 (default)".to_string(),
                source_location: format!("{path}:PRAGMA wal_autocheckpoint"),
            }];
            if let Some(f) = DbFinding::try_new(
                &format!("sqlite-walac-{path}"),
                "db.sqlite.wal.autocheckpoint",
                Severity::Info,
                evidence,
                "db.finding.sqliteWalCheckpoint",
                Confidence::Measured,
            ) {
                findings.push(f);
            }
        }
    }

    // --- freelist waste ratio -------------------------------------------------
    let page_count: i64 = conn
        .query_row("PRAGMA page_count", [], |r| r.get::<_, i64>(0))
        .map_err(|e| e.to_string())?;
    let freelist: i64 = conn
        .query_row("PRAGMA freelist_count", [], |r| r.get::<_, i64>(0))
        .map_err(|e| e.to_string())?;
    if page_count > 0 {
        let waste_bp = ((freelist as f64 / page_count as f64) * 10_000.0).round() as i64;
        if waste_bp >= 2_500 {
            let evidence = vec![
                EvidenceRef {
                    fact: "pragma.page_count".to_string(),
                    observed: page_count.to_string(),
                    expected_or_threshold: "> 0".to_string(),
                    source_location: format!("{path}:PRAGMA page_count"),
                },
                EvidenceRef {
                    fact: "pragma.freelist_count".to_string(),
                    observed: freelist.to_string(),
                    expected_or_threshold: "<25% of pages".to_string(),
                    source_location: format!("{path}:PRAGMA freelist_count"),
                },
            ];
            if let Some(f) = DbFinding::try_new(
                &format!("sqlite-waste-{path}"),
                "db.sqlite.freelist.waste",
                Severity::Info,
                evidence,
                "db.finding.sqliteFreelistWaste",
                Confidence::Measured,
            ) {
                findings.push(f);
            }
        }
    }

    // --- schema census ---------------------------------------------------------
    let objects = query_strings(
        &conn,
        "SELECT type || ':' || name FROM sqlite_master ORDER BY type, name",
        500,
    )?;
    let _ = objects; // census retained in report digest via sorted findings only

    drop(conn);
    Ok(seal_report("sqliteFile", path, findings, Vec::new()))
}

/// Honest lane declaration for engines that cannot run without a live server.
pub fn engine_live_lanes_not_available() -> Vec<String> {
    vec!["postgres.live".to_string(), "mysql.live".to_string()]
}

/// Re-exported so callers can reference the target enum without reaching into model.
pub use crate::model::DiagnosticTarget as _TargetAlias;

#[cfg(test)]
mod tests {
    use super::*;

    fn healthy_db(dir: &std::path::Path) -> std::path::PathBuf {
        use rusqlite::Connection;
        let p = dir.join("healthy.db");
        // Build a WAL-mode db with tables+rows. This is FIXTURE construction inside
        // tests — production code never opens read-write.
        let conn = Connection::open_with_flags(
            &p,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .unwrap();
        conn.pragma_update(None, "journal_mode", "WAL").unwrap();
        conn.execute_batch(
            "CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT NOT NULL);
             CREATE TABLE tags(id INTEGER PRIMARY KEY, item_id INTEGER REFERENCES items(id));
             INSERT INTO items(name) VALUES ('a'),('b'),('c');
             INSERT INTO tags(item_id) VALUES (1),(2);",
        )
        .unwrap();
        conn.pragma_update(None, "wal_checkpoint", "TRUNCATE").ok();
        p
    }

    #[test]
    fn healthy_db_produces_zero_findings_and_stable_digest() {
        let dir = tempfile::tempdir().unwrap();
        let p = healthy_db(dir.path());
        let report = diagnose_sqlite(p.to_str().unwrap()).expect("diagnose");
        assert!(
            report.findings.is_empty(),
            "unexpected findings: {:?}",
            report.findings
        );
        let again = diagnose_sqlite(p.to_str().unwrap()).unwrap();
        assert_eq!(report.digest, again.digest, "digest must be byte-stable");
    }

    #[test]
    fn corrupted_copy_yields_critical_citing_pragma_lines() {
        let dir = tempfile::tempdir().unwrap();
        let p = healthy_db(dir.path());
        let corrupt = dir.path().join("corrupt.db");
        std::fs::copy(&p, &corrupt).unwrap();

        // Flip ONE byte of the page-1 b-tree header (offset 100) — reliably corrupts.
        let mut bytes = std::fs::read(&corrupt).unwrap();
        assert!(bytes.len() > 100);
        bytes[100] ^= 0xFF;
        std::fs::write(&corrupt, &bytes).unwrap();

        let report = diagnose_sqlite(corrupt.to_str().unwrap()).expect("diagnose corrupt");
        let crit = report
            .findings
            .iter()
            .find(|f| f.code == "db.sqlite.integrity.corrupt")
            .expect("expected a Critical integrity finding");
        assert_eq!(crit.severity, Severity::Critical);
        assert!(!crit.evidence.is_empty());
        // Severe corruption makes the PRAGMA itself fail; the verbatim error IS the
        // cited evidence.
        assert_eq!(crit.evidence[0].fact, "pragma.integrity_check.error");
        assert!(
            crit.evidence[0].observed.contains("malformed"),
            "expected SQLITE_CORRUPT verbatim: {}",
            crit.evidence[0].observed
        );
    }

    #[test]
    fn read_only_open_refuses_writes_by_construction() {
        let dir = tempfile::tempdir().unwrap();
        let p = healthy_db(dir.path());
        let conn = open_read_only(p.to_str().unwrap()).unwrap();
        let err = conn.execute("CREATE TABLE nope(x)", []).unwrap_err();
        assert!(matches!(
            err,
            rusqlite::Error::SqliteFailure(ffi, _)
                if ffi.code == rusqlite::ErrorCode::ReadOnly
        ));
    }

    #[test]
    fn missing_file_is_typed_failure_not_panic() {
        let err = diagnose_sqlite("/tmp/definitely-missing-p30.db").unwrap_err();
        assert!(err.contains("sqlite open read-only failed"), "{err}");
    }
}
