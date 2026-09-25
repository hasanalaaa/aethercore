//! P75 lane db-diagnostics: each test states a claim the old code got wrong.

use aethercore_db_diagnostics::{config_lint, slow_log};

fn write(dir: &std::path::Path, name: &str, body: &str) -> std::path::PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, body).unwrap();
    p
}

#[test]
fn mysql_query_time_is_seconds_not_milliseconds() {
    let dir = tempfile::tempdir().unwrap();
    let log = "# Query_time: 1.5  Lock_time: 0.0 Rows_sent: 1  Rows_examined: 1\nSELECT 1;\n";
    let p = write(dir.path(), "slow.log", log);
    let report = slow_log::analyze_mysql_log(p.to_str().unwrap(), 10).unwrap();
    let total = report.findings[0]
        .evidence
        .iter()
        .find(|e| e.fact == "slowlog.aggregate.totalMs")
        .expect("total evidence");
    assert_eq!(total.observed, "1500", "1.5 s is 1500 ms");
}

#[test]
fn slow_log_evidence_claims_no_comparison_it_never_made() {
    let dir = tempfile::tempdir().unwrap();
    let log = "2026-08-25 10:00:00.000 UTC [1] LOG:  duration: 5.0 ms  statement: SELECT 1;\n";
    let p = write(dir.path(), "pg.log", log);
    let report = slow_log::analyze_postgres_log(p.to_str().unwrap(), 10).unwrap();
    for evidence in report.findings.iter().flat_map(|f| &f.evidence) {
        assert!(
            !evidence.expected_or_threshold.contains("outlier"),
            "no outlier test is computed: {evidence:?}"
        );
    }
}
