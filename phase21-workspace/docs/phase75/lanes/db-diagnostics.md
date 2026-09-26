# P75 lane `db-diagnostics` (Wave 2) — evidence (ledger `DBT-P75-030`…`034`)

Branch `lane/db-diagnostics`. Scope: `crates/db-diagnostics/**`. File parsing only; no
engine connection, dependency or lockfile change. Every test is in
`crates/db-diagnostics/tests/p75_db_truth.rs` and failed on `main` (`b032a71`) before its fix
(7 failed, 0 passed).

| row | commit | what was wrong | now |
|---|---|---|---|
| `DBT-P75-030` | `f166891` | MySQL `Query_time` (seconds) was aggregated as milliseconds: 1.5 s showed as 1.5 ms | seconds × 1000 |
| `DBT-P75-031` | `f166891` | slow-log evidence claimed "outlier vs sibling aggregates"; no outlier test exists | "occurrences of the top-1 statement" |
| `DBT-P75-032` | `6966d93` | PostgreSQL duplicates kept; the rule read the first, the server uses the last | one last-wins upsert over `postgresql.conf` then `postgresql.auto.conf` |
| `DBT-P75-033` | `6966d93` | `synchronous_commit = local` / `remote_write` reported as "off" | only an off value is reported |
| `DBT-P75-034` | `fecef56` | MySQL: every option group linted as server config; inline `#` comments and quotes kept in values; `-`/`_` treated as different; a bare `skip-networking` read as off | server groups only; comments and quotes stripped; names normalised; a bare flag is ON |

## Local verification (macOS)

* `cargo test -p aethercore-db-diagnostics --locked`: 17 pass (4 + 6 existing, 7 new).
* `cargo clippy -p aethercore-db-diagnostics --all-targets --locked -- -D warnings`: clean, host and
  `--target x86_64-pc-windows-msvc`.
* The fuzz facades `parse_pg_line_public` / `parse_my_line_public` keep their signatures.
