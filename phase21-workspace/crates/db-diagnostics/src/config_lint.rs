//! Phase 30 (T3) — config-lint providers: PostgreSQL/MySQL config file parsing.
//! File parsing ONLY — zero connections to any engine.

use crate::model::{Confidence, DbFinding, DiagnosticTarget, EvidenceRef, Severity};

/// Bounded include/override depth to survive hostile include loops.
pub const MAX_INCLUDE_DEPTH: usize = 4;
/// Hard cap on parsed directives per file.
pub const MAX_DIRECTIVES: usize = 5_000;

#[derive(Clone, Debug, PartialEq)]
pub struct Directive {
    pub key: String,
    pub value: String,
    pub source_location: String,
}

/// Parses one PostgreSQL-style line (`key = value`, `#` comments).
fn parse_pg_line(line: &str, loc: &str) -> Option<Directive> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let (key, value) = trimmed.split_once('=')?;
    // Strip inline comments: `value = on  # note` → "on".
    let mut raw = value.trim();
    if let Some(hash) = raw.find('#') {
        raw = raw[..hash].trim_end();
    }
    Some(Directive {
        key: key.trim().to_ascii_lowercase(),
        value: raw.trim_matches('\'').to_string(),
        source_location: loc.to_string(),
    })
}

/// Parses one MySQL-style line (`key = value` or `key` alone, `#`/`;` comments).
fn parse_my_line(line: &str, loc: &str) -> Option<Directive> {
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('#')
        || trimmed.starts_with(';')
        || trimmed.starts_with('[')
    {
        return None;
    }
    let (key, value) = match trimmed.split_once('=') {
        Some((k, v)) => (k.trim(), v.trim()),
        None => (trimmed, ""),
    };
    Some(Directive {
        key: key.trim().to_ascii_lowercase(),
        value: value.to_string(),
        source_location: loc.to_string(),
    })
}

fn read_capped(path: &std::path::Path) -> Result<String, String> {
    const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(format!(
            "{}: exceeds {MAX_FILE_BYTES} bytes",
            path.display()
        ));
    }
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Collects postgresql.conf + postgresql.auto.conf (auto overrides base), bounded.
pub fn load_postgres_directives(dir: &std::path::Path) -> Result<Vec<Directive>, String> {
    let mut out = Vec::new();
    let base = dir.join("postgresql.conf");
    for (i, line) in read_capped(&base)?.lines().enumerate() {
        if out.len() >= MAX_DIRECTIVES {
            break;
        }
        if let Some(d) = parse_pg_line(line, &format!("{}:{}", base.display(), i + 1)) {
            out.push(d);
        }
    }
    let auto = dir.join("postgresql.auto.conf");
    if auto.exists() {
        for (i, line) in read_capped(&auto)?.lines().enumerate() {
            if out.len() >= MAX_DIRECTIVES {
                break;
            }
            if let Some(d) = parse_pg_line(line, &format!("{}:{}", auto.display(), i + 1)) {
                // auto.conf entries override base-file entries with the same key.
                if let Some(existing) = out
                    .iter_mut()
                    .find(|e| e.key == d.key && e.source_location.contains("postgresql.conf"))
                {
                    *existing = d;
                } else if let Some(existing) = out.iter_mut().find(|e| e.key == d.key) {
                    *existing = d;
                } else {
                    out.push(d);
                }
            }
        }
    }
    Ok(out)
}

/// Collects mysqld.cnf / mysql.cnf under a conf.d-style directory.
pub fn load_mysql_directives(dir: &std::path::Path) -> Result<Vec<Directive>, String> {
    let mut out: Vec<Directive> = Vec::new();
    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    for name in ["mysqld.cnf", "mysql.cnf"] {
        let p = dir.join(name);
        if p.exists() {
            paths.push(p);
        }
    }
    let paths: Vec<std::path::PathBuf> = if paths.is_empty() {
        vec![dir.to_path_buf()]
    } else {
        paths
    };
    for p in paths {
        for (i, line) in read_capped(&p)?.lines().enumerate() {
            if out.len() >= MAX_DIRECTIVES {
                break;
            }
            if let Some(d) = parse_my_line(line, &format!("{}:{}", p.display(), i + 1)) {
                if let Some(existing) = out.iter_mut().find(|e| e.key == d.key) {
                    *existing = d; // last wins, like MySQL
                } else {
                    out.push(d);
                }
            }
        }
    }
    Ok(out)
}

fn boolish(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "on" | "true" | "1" | "yes"
    )
}

/// Public wrappers for fuzz facades (same parsers, pub visibility).
pub fn parse_pg_line_public(line: &str) -> Option<Directive> {
    parse_pg_line(line, "fuzz")
}

pub fn parse_my_line_public(line: &str) -> Option<Directive> {
    parse_my_line(line, "fuzz")
}

/// PostgreSQL durability + exposure + replication rules. Evidence-cited only.
pub fn lint_postgres(dir: &std::path::Path) -> Result<(Vec<DbFinding>, Vec<String>), String> {
    let directives = load_postgres_directives(dir)?;
    let mut findings = Vec::new();
    let get = |key: &str| directives.iter().find(|d| d.key == key);

    if let Some(d) = get("fsync") {
        if !boolish(&d.value) {
            let ev = vec![EvidenceRef {
                fact: "pg.fsync".into(),
                observed: d.value.clone(),
                expected_or_threshold: "on".into(),
                source_location: d.source_location.clone(),
            }];
            if let Some(f) = DbFinding::try_new(
                "pg-fsync-off",
                "db.pg.durability.fsyncOff",
                Severity::Critical,
                ev,
                "db.finding.pgFsync",
                Confidence::Measured,
            ) {
                findings.push(f);
            }
        }
    }
    if let Some(d) = get("synchronous_commit") {
        if !boolish(&d.value) && d.value != "remote_apply" {
            let ev = vec![EvidenceRef {
                fact: "pg.synchronous_commit".into(),
                observed: d.value.clone(),
                expected_or_threshold: "on|remote_apply".into(),
                source_location: d.source_location.clone(),
            }];
            if let Some(f) = DbFinding::try_new(
                "pg-sync-commit-off",
                "db.pg.durability.synchronousCommitOff",
                Severity::Warning,
                ev,
                "db.finding.pgSyncCommit",
                Confidence::Measured,
            ) {
                findings.push(f);
            }
        }
    }
    if let (Some(listen), ssl) = (get("listen_addresses"), get("ssl")) {
        let exposed = listen.value == "*" || listen.value == "0.0.0.0";
        let ssl_on = ssl.map(|d| boolish(&d.value)).unwrap_or(false);
        if exposed && !ssl_on {
            let ev = vec![
                EvidenceRef {
                    fact: "pg.listen_addresses".into(),
                    observed: listen.value.clone(),
                    expected_or_threshold: "loopback or TLS-protected wildcard".into(),
                    source_location: listen.source_location.clone(),
                },
                EvidenceRef {
                    fact: "pg.ssl".into(),
                    observed: ssl
                        .map(|d| d.value.clone())
                        .unwrap_or_else(|| "<unset>".into()),
                    expected_or_threshold: "on when listening broadly".into(),
                    source_location: ssl
                        .map(|d| d.source_location.clone())
                        .unwrap_or_else(|| format!("{}/postgresql.conf", dir.display())),
                },
            ];
            if let Some(f) = DbFinding::try_new(
                "pg-exposure",
                "db.pg.exposure.unencryptedWildcard",
                Severity::Critical,
                ev,
                "db.finding.pgExposure",
                Confidence::Measured,
            ) {
                findings.push(f);
            }
        }
    }
    // Replication basics only when a standby hint exists.
    let standby_hint = directives
        .iter()
        .any(|d| d.key == "primary_conninfo" || d.key == "standby_mode");
    if standby_hint {
        if let Some(wal) = get("wal_level") {
            if !matches!(wal.value.as_str(), "replica" | "logical") {
                let ev = vec![EvidenceRef {
                    fact: "pg.wal_level".into(),
                    observed: wal.value.clone(),
                    expected_or_threshold: "replica|logical with standby hints present".into(),
                    source_location: wal.source_location.clone(),
                }];
                if let Some(f) = DbFinding::try_new(
                    "pg-wal-level",
                    "db.pg.replication.walLevel",
                    Severity::Critical,
                    ev,
                    "db.finding.pgWalLevel",
                    Confidence::Measured,
                ) {
                    findings.push(f);
                }
            }
        }
    }

    // Engine-live lane honestly NotAvailable on this host (QD-030-001).
    let not_available = vec!["postgres.live".to_string()];
    let _ = DiagnosticTarget::PostgresConfigDir {
        path: dir.display().to_string(),
    };
    Ok((findings, not_available))
}

/// MySQL durability + exposure rules. Evidence-cited only.
pub fn lint_mysql(dir: &std::path::Path) -> Result<(Vec<DbFinding>, Vec<String>), String> {
    let directives = load_mysql_directives(dir)?;
    let mut findings = Vec::new();
    let get = |key: &str| directives.iter().find(|d| d.key == key);

    if let Some(d) = get("innodb_flush_log_at_trx_commit") {
        if d.value != "1" {
            let ev = vec![EvidenceRef {
                fact: "my.innodb_flush_log_at_trx_commit".into(),
                observed: d.value.clone(),
                expected_or_threshold: "1".into(),
                source_location: d.source_location.clone(),
            }];
            if let Some(f) = DbFinding::try_new(
                "my-flush-trx",
                "db.my.durability.flushLogAtTrxCommit",
                Severity::Critical,
                ev,
                "db.finding.myFlushTrx",
                Confidence::Measured,
            ) {
                findings.push(f);
            }
        }
    }
    let skip_networking = get("skip_networking")
        .map(|d| boolish(&d.value))
        .unwrap_or(false);
    let bind = get("bind_address").or_else(|| get("bind-address"));
    let broad_bind = bind
        .map(|d| d.value == "0.0.0.0" || d.value == "*")
        .unwrap_or(false);
    if !skip_networking && broad_bind {
        let bind_ref = bind.expect("checked above");
        let ev = vec![EvidenceRef {
            fact: "my.bind_address".into(),
            observed: bind_ref.value.clone(),
            expected_or_threshold: "loopback or skip_networking=ON".into(),
            source_location: bind_ref.source_location.clone(),
        }];
        if let Some(f) = DbFinding::try_new(
            "my-exposure",
            "db.my.exposure.broadBind",
            Severity::Warning,
            ev,
            "db.finding.myBind",
            Confidence::Measured,
        ) {
            findings.push(f);
        }
    }
    // Engine-live lane honestly NotAvailable on this host (QD-030-001).
    let not_available = vec!["mysql.live".to_string()];
    Ok((findings, not_available))
}
