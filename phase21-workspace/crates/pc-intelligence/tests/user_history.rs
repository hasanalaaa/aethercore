//! Quality pass B6, through the actual coordinator: a completed deep scan is in its Windows
//! user's history and is their latest result after a new sign-in (a new logon session key),
//! while the running scan and its cancel stay bound to the session that started it.
use std::{sync::Arc, thread, time::Duration};

use aethercore_cleaner::CleanupSnapshot;
use aethercore_collector_runtime::CancellationToken;
use aethercore_diagnostic_engine::DiagnosticsSnapshot;
use aethercore_driver_hub::DriverHubSnapshot;
use aethercore_pc_intelligence::{
    DeepScanBackend, DeepScanCoordinator, IntelligenceError, ScanState, SourceError,
};
use aethercore_persistence::Database;
use aethercore_startup_manager::StartupSnapshot;
use aethercore_system_repair::RepairAssessment;
use aethercore_update_engine::{UpdateChannel, UpdateSnapshot, UpdateState};

struct NothingAvailable;
impl DeepScanBackend for NothingAvailable {
    fn driver(&self, _: &str, _: &CancellationToken) -> Result<DriverHubSnapshot, SourceError> {
        Err(SourceError::Unavailable("fixture".into()))
    }
    fn repair(&self, _: &str, _: &CancellationToken) -> Result<RepairAssessment, SourceError> {
        Err(SourceError::Unavailable("fixture".into()))
    }
    fn diagnostics(
        &self,
        _: &str,
        _: &CancellationToken,
    ) -> Result<DiagnosticsSnapshot, SourceError> {
        Err(SourceError::Unavailable("fixture".into()))
    }
    fn startup(&self, _: &str, _: &CancellationToken) -> Result<StartupSnapshot, SourceError> {
        Err(SourceError::Unavailable("fixture".into()))
    }
    fn cleanup(&self, _: &str, _: &CancellationToken) -> Result<CleanupSnapshot, SourceError> {
        Err(SourceError::Unavailable("fixture".into()))
    }
    fn update(&self, _: &str) -> UpdateSnapshot {
        UpdateSnapshot {
            state: UpdateState::Disabled,
            channel: UpdateChannel::Stable,
            current_version: "0.1.13".into(),
            latest_release: None,
            staged_release: None,
            progress_known: false,
            overall_percent: 0,
            bytes_completed: 0,
            bytes_total: 0,
            status_message_key: String::new(),
            checked_unix_ms: 0,
            updated_unix_ms: 0,
        }
    }
}

#[test]
fn a_completed_scan_follows_its_user_into_the_next_logon_session() {
    let path = std::env::temp_dir().join(format!(
        "aethercore-user-history-{}.db",
        uuid::Uuid::new_v4()
    ));
    let db = Arc::new(Database::open(&path).unwrap());
    let coordinator = DeepScanCoordinator::new(db, Arc::new(NothingAvailable), "0.1.13");
    let started = coordinator.start("session-1", "user-a").unwrap();
    let mut finished = false;
    for _ in 0..200 {
        if coordinator.snapshot_for_owner("session-1").unwrap().state != ScanState::Scanning {
            finished = true;
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    assert!(finished, "the fixture scan settles");

    // After a reboot the same user has a new session key.
    let history = coordinator.history("user-a", 10).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].scan_id, started.scan_id);
    let latest = coordinator
        .latest_completed_for_user("user-a")
        .unwrap()
        .expect("the user's last result");
    assert_eq!(latest.scan_id, started.scan_id);

    // Another user sees nothing, and an unknown user key matches nothing.
    assert!(coordinator.history("user-b", 10).unwrap().is_empty());
    assert!(
        coordinator
            .latest_completed_for_user("user-b")
            .unwrap()
            .is_none()
    );
    assert!(coordinator.latest_completed_for_user("").unwrap().is_none());

    // The live state and its cancel stay with the session that ran the scan.
    assert!(matches!(
        coordinator.snapshot_for_owner("session-2"),
        Err(IntelligenceError::Ownership)
    ));
    assert!(matches!(
        coordinator.cancel("session-2", &started.scan_id),
        Err(IntelligenceError::Ownership)
    ));
    drop(coordinator);
    let _ = std::fs::remove_file(&path);
}
