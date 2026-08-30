#![cfg(windows)]

const OWNER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

use std::{sync::Arc, thread, time::{Duration, Instant}};

use aethercore_cleaner::{CleanupEngine, CleanupScanState};
use aethercore_operation_kernel::{ReadBudgetManager, ReadWorkload};
use aethercore_operation_engine::OperationEngine;
use aethercore_persistence::Database;
use uuid::Uuid;

#[test]
#[ignore = "requires a Windows staging machine; scan only, performs no deletion"]
fn live_cleanup_scan_returns_only_service_minted_categories() {
    let path = std::env::temp_dir().join(format!("aethercore-live-cleanup-{}.db", Uuid::new_v4()));
    let db = Arc::new(Database::open(&path).expect("database"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let cleaner = CleanupEngine::new(engine, db);
    let budget = ReadBudgetManager::new(4);
    let lease = budget.try_acquire(ReadWorkload::CleanupDiscovery).expect("read budget lease");
    cleaner.start_scan_with_lease(OWNER, lease).expect("start scan");

    let deadline = Instant::now() + Duration::from_secs(5 * 60);
    loop {
        let snapshot = cleaner.snapshot_for_owner(OWNER).expect("owned cleanup snapshot");
        match snapshot.state {
            CleanupScanState::Ready => {
                assert!(snapshot.candidates.iter().all(|candidate| !candidate.candidate_id.is_empty()));
                assert!(snapshot.candidates.iter().all(|candidate| candidate.file_count <= 5_000));
                break;
            }
            CleanupScanState::Failed => panic!("cleanup scan failed: {}", snapshot.error_message),
            _ => {}
        }
        assert!(Instant::now() < deadline, "live cleanup scan timed out");
        thread::sleep(Duration::from_millis(500));
    }

    drop(cleaner);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("db-wal"));
    let _ = std::fs::remove_file(path.with_extension("db-shm"));
}
