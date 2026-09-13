use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use aethercore_cleaner::{
    CleanerError, CleanupCandidate, CleanupEngine, CleanupMutationLease, CleanupPlatform,
    CleanupScanState,
};
use aethercore_operation_engine::{CleanupDeleteAction, CleanupFileEvidence, OperationEngine};
use aethercore_operation_kernel::{
    MutationSupervisor, MutationWorkload, ReadBudgetManager, ReadWorkload,
};
use aethercore_persistence::Database;

fn start_cleanup(
    cleaner: &CleanupEngine,
    owner: &str,
    plan_id: &str,
) -> aethercore_cleaner::Result<aethercore_cleaner::CleanupExecutionStatus> {
    let supervisor = MutationSupervisor::new();
    let lease = supervisor
        .try_acquire(MutationWorkload::Cleanup, plan_id, owner)
        .expect("mutation lease");
    cleaner.start_with_lease(owner, plan_id, lease)
}

fn start_cleanup_scan(
    cleaner: &CleanupEngine,
    owner: &str,
) -> aethercore_cleaner::Result<aethercore_cleaner::CleanupSnapshot> {
    let budget = ReadBudgetManager::new(4);
    let lease = budget
        .try_acquire(ReadWorkload::CleanupDiscovery)
        .expect("read budget lease");
    cleaner.start_scan_with_lease(owner, lease)
}

#[derive(Clone)]
struct FakeCleanupPlatform {
    fail_delete: bool,
}

impl CleanupPlatform for FakeCleanupPlatform {
    fn scan(&self) -> aethercore_cleaner::Result<Vec<CleanupCandidate>> {
        Ok(vec![CleanupCandidate {
            candidate_id: "candidate-temp".into(),
            provider: "TestTemp".into(),
            title: "Test temporary files".into(),
            description: "fixture".into(),
            reclaimable_bytes: 100,
            file_count: 2,
            selected_by_default: true,
            requires_explicit_confirmation: false,
            truncated: false,
            special_kind: "Files".into(),
            files: vec![
                CleanupFileEvidence {
                    path: r"C:\fixture\a.tmp".into(),
                    root: r"C:\fixture".into(),
                    root_final_path: r"C:\fixture".into(),
                    size_bytes: 60,
                    modified_unix_ms: 1,
                    volume_serial_number: 1,
                    file_id_128: "00112233445566778899aabbccddeeff".into(),
                },
                CleanupFileEvidence {
                    path: r"C:\fixture\b.tmp".into(),
                    root: r"C:\fixture".into(),
                    root_final_path: r"C:\fixture".into(),
                    size_bytes: 40,
                    modified_unix_ms: 1,
                    volume_serial_number: 1,
                    file_id_128: "00112233445566778899aabbccddeeff".into(),
                },
            ],
        }])
    }

    fn delete_action(
        &self,
        _action: &CleanupDeleteAction,
    ) -> aethercore_cleaner::Result<(u64, u64, String)> {
        if self.fail_delete {
            Err(CleanerError::Safety("injected validation failure".into()))
        } else {
            Ok((60, 40, "one changed file was skipped".into()))
        }
    }

    /// DBT-P61-002: the fake deletes nothing on the real machine, so it holds no
    /// machine-wide boundary. Before this existed the fake fell through to the real
    /// Windows lease, whose lock file only the installer creates — which is why these
    /// tests could not pass on any Windows machine the product was not installed on.
    fn acquire_mutation_lease(&self) -> aethercore_cleaner::Result<CleanupMutationLease> {
        Ok(Box::new(()))
    }
}

struct LeaseDenyingPlatform {
    inner: FakeCleanupPlatform,
    asked: Arc<AtomicUsize>,
}

impl CleanupPlatform for LeaseDenyingPlatform {
    fn scan(&self) -> aethercore_cleaner::Result<Vec<CleanupCandidate>> {
        self.inner.scan()
    }

    fn delete_action(
        &self,
        action: &CleanupDeleteAction,
    ) -> aethercore_cleaner::Result<(u64, u64, String)> {
        panic!("deletion ran despite a refused mutation lease: {action:?}");
    }

    fn acquire_mutation_lease(&self) -> aethercore_cleaner::Result<CleanupMutationLease> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        Err(CleanerError::MutationBusy)
    }
}

fn temp_root(label: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "aethercore-phase4-cleaner-{label}-{}-{nonce}",
        std::process::id()
    ))
}

fn wait_scan(cleaner: &CleanupEngine) -> aethercore_cleaner::CleanupSnapshot {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let snapshot = cleaner
            .snapshot_for_owner(OWNER)
            .expect("owned cleanup snapshot");
        if snapshot.state == CleanupScanState::Ready {
            return snapshot;
        }
        assert_ne!(snapshot.state, CleanupScanState::Failed);
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    }
}

const OWNER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn authorize(engine: &OperationEngine, plan_id: &str, digest: &str) {
    let intent = engine
        .begin_consent_intent(plan_id, OWNER)
        .expect("consent intent");
    engine
        .approve_consent_intent(&intent.intent_id, OWNER, 4242)
        .expect("consent approval");
}

#[test]
fn cleanup_uses_frozen_candidate_evidence_and_reports_partial_skips() {
    let root = temp_root("partial");
    std::fs::create_dir_all(&root).expect("root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("db"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let cleaner = CleanupEngine::with_platform(
        engine.clone(),
        db.clone(),
        Arc::new(FakeCleanupPlatform { fail_delete: false }),
    );

    start_cleanup_scan(&cleaner, OWNER).expect("scan");
    let snapshot = wait_scan(&cleaner);
    let id = snapshot.candidates[0].candidate_id.clone();
    let plan = cleaner
        .create_plan(
            OWNER,
            &snapshot.scan_id,
            snapshot.inventory_epoch,
            &[id.clone()],
        )
        .expect("plan");
    let actions = engine.cleanup_actions(&plan.id).expect("actions");
    assert_eq!(actions[0].files.len(), 2);
    assert_eq!(actions[0].files[0].root_final_path, r"C:\fixture");
    assert_eq!(actions[0].expected_bytes, 100);
    assert!(matches!(
        cleaner.create_plan(
            OWNER,
            &snapshot.scan_id,
            snapshot.inventory_epoch,
            &[id.clone(), id]
        ),
        Err(CleanerError::CandidateInvalid(_))
    ));

    authorize(&engine, &plan.id, &plan.digest);
    start_cleanup(&cleaner, OWNER, &plan.id).expect("start");
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let status = cleaner.status(OWNER, Some(&plan.id)).unwrap().unwrap();
        if status.plan_state == "Completed" {
            assert!(status.mutation_started);
            assert_eq!(status.reclaimed_bytes, 60);
            assert_eq!(status.skipped_bytes, 40);
            assert_eq!(status.items[0].result_code, "Partial");
            break;
        }
        assert_ne!(status.plan_state, "Failed", "{}", status.failure_message);
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cleanup_failure_after_deletion_barrier_requires_recovery_review() {
    let root = temp_root("failure");
    std::fs::create_dir_all(&root).expect("root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("db"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let cleaner = CleanupEngine::with_platform(
        engine.clone(),
        db.clone(),
        Arc::new(FakeCleanupPlatform { fail_delete: true }),
    );

    start_cleanup_scan(&cleaner, OWNER).expect("scan");
    let snapshot = wait_scan(&cleaner);
    let plan = cleaner
        .create_plan(
            OWNER,
            &snapshot.scan_id,
            snapshot.inventory_epoch,
            &[snapshot.candidates[0].candidate_id.clone()],
        )
        .expect("plan");
    authorize(&engine, &plan.id, &plan.digest);
    start_cleanup(&cleaner, OWNER, &plan.id).expect("start");

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let status = cleaner.status(OWNER, Some(&plan.id)).unwrap().unwrap();
        if status.plan_state == "Failed" {
            assert!(status.mutation_started);
            assert!(status.recovery_required);
            break;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    }
    let _ = std::fs::remove_dir_all(root);
}

/// DBT-P61-002: the machine-wide mutation boundary is the platform's to supply. A
/// platform that refuses the lease must stop the cleanup before any deletion, and the
/// refusal must be the one the platform named. Without the routing this test cannot be
/// written at all: the guard was a free function no injected platform could reach.
#[test]
fn cleanup_takes_its_mutation_lease_from_the_platform() {
    let root = temp_root("lease");
    std::fs::create_dir_all(&root).expect("root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("db"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let platform = Arc::new(LeaseDenyingPlatform {
        inner: FakeCleanupPlatform { fail_delete: false },
        asked: Arc::new(AtomicUsize::new(0)),
    });
    let asked = platform.asked.clone();
    let cleaner = CleanupEngine::with_platform(engine.clone(), db.clone(), platform);

    start_cleanup_scan(&cleaner, OWNER).expect("scan");
    let snapshot = wait_scan(&cleaner);
    let plan = cleaner
        .create_plan(
            OWNER,
            &snapshot.scan_id,
            snapshot.inventory_epoch,
            &[snapshot.candidates[0].candidate_id.clone()],
        )
        .expect("plan");
    authorize(&engine, &plan.id, &plan.digest);
    start_cleanup(&cleaner, OWNER, &plan.id).expect("start");

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let status = cleaner.status(OWNER, Some(&plan.id)).unwrap().unwrap();
        // Wait on the execution record's own stage, not the plan state: the engine's
        // transition to Failed lands before fail_cleanup writes the record that carries
        // the message.
        if status.stage == "Failed" {
            assert_eq!(status.plan_state, "Failed");
            assert_eq!(asked.load(Ordering::SeqCst), 1);
            assert!(
                status
                    .failure_message
                    .contains("another AetherCore maintenance mutation is active"),
                "{}",
                status.failure_message
            );
            assert!(!status.mutation_started);
            break;
        }
        assert_ne!(status.plan_state, "Completed");
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cleanup_snapshot_is_principal_bound() {
    let root = temp_root("ownership");
    std::fs::create_dir_all(&root).expect("root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("db"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let cleaner = CleanupEngine::with_platform(
        engine,
        db,
        Arc::new(FakeCleanupPlatform { fail_delete: false }),
    );
    start_cleanup_scan(&cleaner, OWNER).expect("scan");
    let _ = wait_scan(&cleaner);
    let other = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    assert!(matches!(
        cleaner.snapshot_for_owner(other),
        Err(CleanerError::OwnershipMismatch)
    ));
    let _ = std::fs::remove_dir_all(root);
}
