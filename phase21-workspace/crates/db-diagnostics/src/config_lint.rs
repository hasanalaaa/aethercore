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

/// Parses one MySQL option line (`key = value` or a bare `key` flag). As MySQL reads
/// option files: `#`/`;` start a comment line, `#` also ends a value outside quotes, a
/// value may be quoted, and `-`/`_` are the same in option names (`loose-` is a prefix
/// that only changes how unknown options are reported). Group headers are handled by
/// the caller, which keeps only server groups.
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
        Some((k, v)) => (k.trim().to_string(), strip_my_value(v)),
        None => (strip_my_value(trimmed), String::new()),
    };
    let key = key.to_ascii_lowercase().replace('-', "_");
    let key = key.strip_prefix("loose_").unwrap_or(&key).to_string();
    Some(Directive {
        key,
        value,
        source_location: loc.to_string(),
    })
}

/// A value without its trailing `#` comment (outside quotes) and without matching quotes.
fn strip_my_value(raw: &str) -> String {
    let mut quote: Option<char> = None;
    let mut end = raw.len();
    for (i, c) in raw.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, '#') => {
                end = i;
                break;
            }
            _ => {}
        }
    }
    let value = raw[..end].trim();
    for q in ['"', '\''] {
        if let Some(inner) = value.strip_prefix(q).and_then(|v| v.strip_suffix(q)) {
            return inner.to_string();
        }
    }
    value.to_string()
}

/// Option groups the MySQL/MariaDB server reads (`[client]`, `[mysqldump]` … are not).
fn is_server_group(header: &str) -> bool {
    let name = header.trim().to_ascii_lowercase();
    matches!(name.as_str(), "mysqld" | "server" | "mariadb" | "mariadbd")
        || name.starts_with("mysqld-")
        || name.starts_with("mariadb-")
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

/// Replaces an earlier setting of the same key, or appends: the server keeps the last.
fn set_last(out: &mut Vec<Directive>, directive: Directive) {
    match out
        .iter_mut()
        .find(|existing| existing.key == directive.key)
    {
        Some(existing) => *existing = directive,
        None => out.push(directive),
    }
}

/// Collects postgresql.conf then postgresql.auto.conf, bounded. As in PostgreSQL, the last
/// setting of a key wins, within a file and across the two, so auto.conf overrides the base.
pub fn load_postgres_directives(dir: &std::path::Path) -> Result<Vec<Directive>, String> {
    let mut out = Vec::new();
    let auto = dir.join("postgresql.auto.conf");
    let mut files = vec![dir.join("postgresql.conf")];
    if auto.exists() {
        files.push(auto);
    }
    for path in files {
        for (i, line) in read_capped(&path)?.lines().enumerate() {
            if out.len() >= MAX_DIRECTIVES {
                break;
            }
            if let Some(d) = parse_pg_line(line, &format!("{}:{}", path.display(), i + 1)) {
                set_last(&mut out, d);
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
        // Lines before any group header belong to no group; MySQL rejects them.
        let mut in_server_group = false;
        for (i, line) in read_capped(&p)?.lines().enumerate() {
            if out.len() >= MAX_DIRECTIVES {
                break;
            }
            let trimmed = line.trim();
            if let Some(header) = trimmed.strip_prefix('[').and_then(|h| h.split_once(']')) {
                in_server_group = is_server_group(header.0);
                continue;
            }
            if !in_server_group {
                continue;
            }
            if let Some(d) = parse_my_line(line, &format!("{}:{}", p.display(), i + 1)) {
                set_last(&mut out, d); // last wins, like MySQL
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

    if let Some(d) = get("fsync")
        && !boolish(&d.value)
    {
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
    // `local` and `remote_write` still flush the local WAL before a commit returns;
    // only `off` acknowledges commits that a crash can lose.
    if let Some(d) = get("synchronous_commit")
        && matches!(
            d.value.to_ascii_lowercase().as_str(),
            "off" | "false" | "no" | "0"
        )
    {
        let ev = vec![EvidenceRef {
            fact: "pg.synchronous_commit".into(),
            observed: d.value.clone(),
            expected_or_threshold: "not off (on|local|remote_write|remote_apply)".into(),
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
    if standby_hint
        && let Some(wal) = get("wal_level")
        && !matches!(wal.value.as_str(), "replica" | "logical")
    {
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

    if let Some(d) = get("innodb_flush_log_at_trx_commit")
        && d.value != "1"
    {
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
    // A bare `skip-networking` is a flag set ON.
    let skip_networking = get("skip_networking")
        .map(|d| d.value.is_empty() || boolish(&d.value))
        .unwrap_or(false);
    let bind = get("bind_address");
    let broad_bind = bind
        .map(|d| d.value == "0.0.0.0" || d.value == "*")
        .unwrap_or(false);
    // `broad_bind` is only ever true when `bind` is Some; the `if let` says so to the
    // compiler instead of asserting it at runtime. P63.
    if let (false, true, Some(bind_ref)) = (skip_networking, broad_bind, bind) {
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
