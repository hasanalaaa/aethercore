//! Phase 31 (W8) — SQLite diagnose baseline on a ~10MB healthy db.

use criterion::{Criterion, criterion_group, criterion_main};
use rusqlite::OpenFlags;

fn build_db(path: &std::path::Path) {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .unwrap();
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT NOT NULL, blob TEXT);",
    )
    .unwrap();
    // 20k rows x ~200-byte filler ≈ several MB — enough to exercise page traversal.
    {
        use rusqlite::params;
        let filler = "x".repeat(200);
        for i in 0..20_000i64 {
            conn.execute(
                "INSERT INTO items(id, name, blob) VALUES (?, ?, ?)",
                params![i, format!("row-{i}"), format!("{filler}{i}")],
            )
            .unwrap();
        }
    }
    let _ = conn.pragma_update(None, "wal_checkpoint", "TRUNCATE");
}

fn bench_sqlite_diagnose_healthy(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bench.db");
    build_db(&path);
    let path_str = path.to_str().unwrap().to_string();
    c.bench_function("sqlite_diagnose_healthy_10mb", |b| {
        b.iter(|| {
            aethercore_db_diagnostics::sqlite_provider::diagnose_sqlite(&path_str)
                .expect("diagnose")
        })
    });
}

criterion_group!(benches, bench_sqlite_diagnose_healthy);
criterion_main!(benches);
