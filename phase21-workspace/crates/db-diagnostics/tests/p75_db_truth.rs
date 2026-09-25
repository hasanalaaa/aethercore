//! P75 lane db-diagnostics: each test states a claim the old code got wrong.

use aethercore_db_diagnostics::{config_lint, slow_log};

fn write(dir: &std::path::Path, name: &str, body: &str) -> std::path::PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, body).unwrap();
    p
}

fn pg_codes(conf: &str) -> Vec<String> {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "postgresql.conf", conf);
    let (findings, _) = config_lint::lint_postgres(dir.path()).expect("lint postgres");
    findings.into_iter().map(|f| f.code).collect()
}

fn my_codes(cnf: &str) -> Vec<String> {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "mysqld.cnf", cnf);
    let (findings, _) = config_lint::lint_mysql(dir.path()).expect("lint mysql");
    findings.into_iter().map(|f| f.code).collect()
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

#[test]
fn postgres_last_setting_in_a_file_wins() {
    let codes = pg_codes("fsync = off\nfsync = on\n");
    assert!(
        !codes.iter().any(|c| c == "db.pg.durability.fsyncOff"),
        "{codes:?}"
    );
}

#[test]
fn synchronous_commit_is_off_only_when_it_is_off() {
    for durable in ["on", "local", "remote_write", "remote_apply"] {
        let codes = pg_codes(&format!("synchronous_commit = {durable}\n"));
        assert!(
            !codes
                .iter()
                .any(|c| c == "db.pg.durability.synchronousCommitOff"),
            "{durable}: {codes:?}"
        );
    }
    let codes = pg_codes("synchronous_commit = off\n");
    assert!(
        codes
            .iter()
            .any(|c| c == "db.pg.durability.synchronousCommitOff")
    );
}

#[test]
fn mysql_client_section_is_not_server_configuration() {
    let codes = my_codes("[mysqld]\nbind-address = 127.0.0.1\n[client]\nbind-address = 0.0.0.0\n");
    assert!(
        !codes.iter().any(|c| c == "db.my.exposure.broadBind"),
        "{codes:?}"
    );
}

#[test]
fn mysql_skip_networking_written_as_a_flag_is_on() {
    let codes = my_codes("[mysqld]\nbind-address = 0.0.0.0\nskip-networking\n");
    assert!(
        !codes.iter().any(|c| c == "db.my.exposure.broadBind"),
        "{codes:?}"
    );
}

#[test]
fn mysql_inline_comment_and_quotes_are_not_part_of_the_value() {
    let codes = my_codes("[mysqld]\ninnodb_flush_log_at_trx_commit = \"1\" # durable\n");
    assert!(
        !codes
            .iter()
            .any(|c| c == "db.my.durability.flushLogAtTrxCommit"),
        "{codes:?}"
    );
}
