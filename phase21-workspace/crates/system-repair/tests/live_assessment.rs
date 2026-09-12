#![cfg(windows)]

const OWNER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use aethercore_operation_engine::OperationEngine;
use aethercore_operation_kernel::{ReadBudgetManager, ReadWorkload};
use aethercore_persistence::Database;
use aethercore_system_repair::{RepairAssessmentState, RepairCoordinator};
use uuid::Uuid;

#[test]
#[ignore = "requires an elevated Windows staging machine; read-only DISM/SFC/CHKDSK assessment"]
fn live_windows_repair_assessment_is_read_only_and_completes() {
    let path = std::env::temp_dir().join(format!("aethercore-live-repair-{}.db", Uuid::new_v4()));
    let db = Arc::new(Database::open(&path).expect("database"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let coordinator = RepairCoordinator::new(engine, db);
    let budget = ReadBudgetManager::new(4);
    let lease = budget
        .try_acquire(ReadWorkload::RepairAssessment)
        .expect("read budget lease");
    coordinator
        .start_assessment_with_lease(OWNER, lease)
        .expect("start assessment");

    let deadline = Instant::now() + Duration::from_secs(60 * 30);
    loop {
        let assessment = coordinator
            .assessment_for_owner(OWNER)
            .expect("owned assessment");
        match assessment.state {
            RepairAssessmentState::Ready => {
                assert!(!assessment.assessment_id.is_empty());
                assert!(!assessment.checks.is_empty());
                break;
            }
            RepairAssessmentState::Failed => {
                panic!("assessment failed: {}", assessment.error_message)
            }
            _ => {}
        }
        assert!(Instant::now() < deadline, "live assessment timed out");
        thread::sleep(Duration::from_secs(1));
    }

    drop(coordinator);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("db-wal"));
    let _ = std::fs::remove_file(path.with_extension("db-shm"));
}
