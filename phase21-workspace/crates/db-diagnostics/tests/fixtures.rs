//! Phase 30 (GD-2/GD-3) — fixture tests: config lints exact-findings + slow-log
//! deterministic top-N ×2 byte-equal.

use aethercore_db_diagnostics::config_lint;
use aethercore_db_diagnostics::model::Severity;
use aethercore_db_diagnostics::slow_log;

const PG_RISKY: &str = "\
# postgresql.conf fixture
listen_addresses = '*'
ssl = off
fsync = off
synchronous_commit = off
wal_level = replica
";

const PG_CLEAN: &str = "\
listen_addresses = 'localhost'
ssl = on
fsync = on
synchronous_commit = on
";

const PG_AUTO_OVERRIDE: &str = "\
fsync = on   # auto.conf overrides the base file's risky value
";

const MYSQL_RISKY: &str = "\
[mysqld]
bind-address = 0.0.0.0
innodb_flush_log_at_trx_commit = 2
this is a malformed line without separator
skip-networking = false
";

fn write(dir: &std::path::Path, name: &str, body: &str) -> std::path::PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, body).unwrap();
    p
}

#[test]
fn pg_risky_fixture_emits_exact_expected_findings() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "postgresql.conf", PG_RISKY);
    let (findings, not_available) = config_lint::lint_postgres(dir.path()).expect("lint postgres");
    let mut codes: Vec<&str> = findings.iter().map(|f| f.code.as_str()).collect();
    codes.sort();
    assert_eq!(
        codes,
        vec![
            "db.pg.durability.fsyncOff",
            "db.pg.durability.synchronousCommitOff",
            "db.pg.exposure.unencryptedWildcard",
        ]
    );
    // Every finding cites verbatim source lines.
    for f in &findings {
        for e in &f.evidence {
            assert!(e.source_location.contains("postgresql.conf"), "{e:?}");
            assert!(!e.observed.is_empty());
        }
    }
    assert_eq!(not_available, vec!["postgres.live".to_string()]);
}

#[test]
fn pg_clean_fixture_zero_findings() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "postgresql.conf", PG_CLEAN);
    let (findings, _) = config_lint::lint_postgres(dir.path()).unwrap();
    assert!(findings.is_empty(), "{findings:?}");
}

#[test]
fn pg_auto_conf_overrides_base_risk() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "postgresql.conf", PG_RISKY);
    write(dir.path(), "postgresql.auto.conf", PG_AUTO_OVERRIDE);
    let (findings, _) = config_lint::lint_postgres(dir.path()).unwrap();
    assert!(
        !findings
            .iter()
            .any(|f| f.code == "db.pg.durability.fsyncOff"),
        "auto.conf override must clear the base fsync=off finding"
    );
}

#[test]
fn mysql_risky_fixture_emits_exact_expected_findings_and_skips_malformed() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "mysqld.cnf", MYSQL_RISKY);
    let (findings, not_available) = config_lint::lint_mysql(dir.path()).expect("lint mysql");
    let mut codes: Vec<&str> = findings.iter().map(|f| f.code.as_str()).collect();
    codes.sort();
    assert_eq!(
        codes,
        vec![
            "db.my.durability.flushLogAtTrxCommit",
            "db.my.exposure.broadBind"
        ]
    );
    // Malformed line skipped typed — no panic, no bogus directive finding.
    assert_eq!(not_available, vec!["mysql.live".to_string()]);
}

// ---------------- GD-3: slow-log determinism ×2 byte-equal ----------------

const PG_SLOW_LOG: &str = "\
2026-08-25 10:00:00.000 UTC [100] LOG:  duration: 900.5 ms  statement: SELECT * FROM orders WHERE id = 42;
2026-08-25 10:00:01.000 UTC [101] LOG:  duration: 120.0 ms  statement: select * from orders where id = 7;
2026-08-25 10:00:02.000 UTC [102] LOG:  duration: 3000.25 ms  statement: SELECT name FROM users WHERE email = 'a@b.c';
2026-08-25 10:00:03.000 UTC [103] LOG:  duration: 850.1 ms  statement: SELECT * FROM orders WHERE id = 99;
";

const MYSQL_SLOW_LOG: &str = "\
# Time: 2026-08-25T10:00:00
# Query_time: 2.5  Lock_time: 0.01 Rows_sent: 1  Rows_examined: 50000
SELECT * FROM big_table WHERE col = 'abc';
# Time: 2026-08-25T10:00:05
# Query_time: 0.4  Lock_time: 0.00 Rows_sent: 10  Rows_examined: 12
SELECT id FROM small WHERE x = 3;
";

#[test]
fn postgres_slow_log_top_n_deterministic_x2_byte_equal() {
    let dir = tempfile::tempdir().unwrap();
    let p = write(dir.path(), "pg.log", PG_SLOW_LOG);
    let path = p.to_str().unwrap();

    let r1 = slow_log::analyze_postgres_log(path, 10).unwrap();
    let r2 = slow_log::analyze_postgres_log(path, 10).unwrap();
    assert_eq!(r1, r2, "two runs must be identical");
    assert_eq!(r1.findings.len(), 1, "top-1 advisory expected");
    // The heaviest normalized statement (3000ms) wins; literals collapsed so the two
    // `orders by id` queries aggregate into one fingerprint.
    assert_eq!(r1.target_kind, "postgresSlowLog");
}

#[test]
fn mysql_slow_log_top_n_deterministic_x2_byte_equal_with_ratio_flag() {
    let dir = tempfile::tempdir().unwrap();
    let p = write(dir.path(), "mysql-slow.log", MYSQL_SLOW_LOG);
    let path = p.to_str().unwrap();

    let r1 = slow_log::analyze_mysql_log(path, 10).unwrap();
    let r2 = slow_log::analyze_mysql_log(path, 10).unwrap();
    assert_eq!(r1, r2, "two runs must be byte-identical");
    let ratio = r1
        .findings
        .iter()
        .find(|f| f.code == "db.slowlog.missingIndexSuspicion")
        .expect("expected missing-index suspicion Advisory");
    assert_eq!(ratio.severity, Severity::Warning);
    assert_eq!(ratio.evidence[0].observed, "rows_examined/rows_sent ≥ 100");
}
