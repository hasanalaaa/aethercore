# Phase 30 — Architecture: Database Diagnostics Domain (Read-Only First, Evidence-Cited)

Status: SEALED · Extends docs/phase29/ARCHITECTURE.md

## 1. Read-only guarantee mechanics

`crates/db-diagnostics` opens SQLite targets with
`OpenFlags::SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX` and sets
`busy_timeout(Duration::from_millis(0))` — fail fast; the provider never holds or waits
on a live application's locks. Config-lint providers parse FILES only
(`postgresql.conf`, `postgresql.auto.conf`, `mysqld.cnf`, `mysql.cnf`) — zero engine
connections. Slow-log analyzers read files with hard byte caps.

Audit-enforced: forbidden SQL verbs (INSERT INTO / UPDATE … SET / DELETE FROM /
DROP TABLE / DROP DATABASE / ALTER TABLE / VACUUM / REPLACE INTO) are absent from
executable src (comments, doc-prose, test fixture strings excluded by the scanner).

## 2. Finding model (house discipline)

`DbFinding { id, code, severity, evidence: Vec<EvidenceRef>, summary_key,
message_args, confidence }`. `DbFinding::try_new` returns None when the evidence
matrix is empty — a rule that cannot cite measured facts emits NOTHING.
Reports sort findings deterministically (severity desc → code → id) and seal a sha256
digest over the canonical serialization; identical inputs produce byte-stable digests.

## 3. Rule catalogs

SQLite (live, proven on this Mac): integrity_check (bounded rows) · quick_check ·
foreign_key_check count · journal_mode + wal_autocheckpoint · page_count/freelist waste
ratio (≥25% → Info) · schema census.

PostgreSQL config lint: fsync=off (Critical) · synchronous_commit off (Warning) ·
listen_addresses='*' without ssl=on (Critical) · wal_level vs standby hints.
MySQL config lint: innodb_flush_log_at_trx_commit ≠ 1 (Critical) · broad bind-address
without skip_networking (Warning). Inline comments stripped; auto.conf overrides base;
malformed lines skipped typed.

Slow-log analyzers: literals collapsed to fingerprints; top-N by total duration;
MySQL Rows_examined/Rows_sent ≥ 100 → missing-index suspicion Advisory.

## 4. Offset-100 corruption discovery (GD-1, recorded honestly)

Single-byte flips in free space do NOT trip integrity_check on small WAL databases —
the flipped byte must hit structural metadata. The reliable reproducible corruption for
tests flips one byte of the page-1 b-tree header (offset 100); severe corruption makes
PRAGMA integrity_check itself fail with "database disk image is malformed", which the
provider cites VERBATIM as the Critical finding evidence
(`fact = pragma.integrity_check.error`). Deeper PRAGMAs are skipped after a Critical
integrity verdict (early-seal) since they fail on a corrupt image by definition.

## 5. Trust model — what config-lint can and CANNOT prove

CAN prove: exact directive values present in the parsed files at parse time, cited
file:line. CANNOT prove: runtime behavior (engine may override via SQL), that files are
current, or that a live server matches its config. Engine-live lanes are declared
NotAvailable unless the binary is detected AND the owner passes an explicit connect flag
(QD-030-001).

## 6. Honest limits

- QD-030-001: engine-live lanes need real servers (install or Docker-less env).
- QD-030-002: remediation actions deferred to the existing consent pipeline.
- QD-030-003: rule-set calibration against real production configs open.
