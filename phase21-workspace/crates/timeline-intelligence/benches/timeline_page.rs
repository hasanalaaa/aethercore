//! Phase 31 (W8) — timeline storage read-path baseline (plan_events paging through
//! the persistence layer, the same access pattern TimelineCoordinator::page_for_owner
//! uses).

use aethercore_persistence::Database;
use criterion::{Criterion, criterion_group, criterion_main};
use std::sync::Arc;

fn db_with_events(n: usize) -> (tempfile::TempDir, Arc<Database>, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("timeline-bench.db");
    let db = Arc::new(Database::open(&path).expect("open db"));
    let owner = "bench-owner-0123456789abcdef0123456789abcdef";
    use rusqlite::params;
    let conn = rusqlite::Connection::open(dir.path().join("timeline-bench.db")).unwrap();
    conn.pragma_update(None, "journal_mode", "WAL").ok();
    for i in 0..n as i64 {
        let plan_id = format!("plan-{i}");
        conn.execute(
            "INSERT INTO plans(id,title,state,digest,risk,immutable_json,created_unix_ms,updated_unix_ms,owner_principal_key) VALUES (?,'bench','Succeeded','digest-' || ? ,'{}','low',?,?,'bench-owner-0123456789abcdef0123456789abcdef')",
            params![plan_id, i, 1_700_000_000_000i64 + i, 1_700_000_000_000i64 + i],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO plan_events(plan_id,from_state,to_state,event_kind,detail,created_unix_ms) \
             VALUES (?,'Draft','Scanning','state_transition',?,?)",
            rusqlite::params![plan_id, format!("bench event {i}"), 1_700_000_000_000i64 + i * 1000],
        )
        .unwrap_or_else(|e| panic!("plan_events insert failed: {e}"));
    }
    drop(conn);
    (dir, db, owner.to_string())
}

fn bench_timeline_page(c: &mut Criterion) {
    let (_dir, db, owner) = db_with_events(2_000);
    c.bench_function("timeline_page_read_2000_events", |b| {
        b.iter(|| {
            let events = db
                .support_journal_events_for_owner(&owner, 50)
                .expect("page read");
            assert!(!events.is_empty());
            events.len()
        })
    });
}

criterion_group!(benches, bench_timeline_page);
criterion_main!(benches);
