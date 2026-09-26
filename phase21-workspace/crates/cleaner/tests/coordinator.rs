use std::{
    sync::{
        Arc, Mutex,
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
    MutationSupervisor, MutationWorkload, ProgressTelemetry, ProgressTelemetryStore,
    ReadBudgetManager, ReadWorkload,
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

/// Poll until the plan reaches `state`; the record fields are then already final.
///
/// `plan_state` comes from the operation engine and `mutation_started`, `recovery_required`
/// and `items` from the maintenance record. The terminal transition commits that record in
/// the same transaction, and `status()` reads the plan before the record (DBT-P63-012), so
/// waiting on `plan_state` alone is enough. It was not while `fail_cleanup` transitioned
/// first and upserted after: that read-before-write race failed
/// `cleanup_failure_after_deletion_barrier_requires_recovery_review` on
/// `status.recovery_required` in CI run 34763617017, and this loop had to wait on `stage` too.
fn wait_terminal(
    cleaner: &CleanupEngine,
    plan_id: &str,
    state: &str,
) -> aethercore_cleaner::CleanupExecutionStatus {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let status = cleaner.status(OWNER, Some(plan_id)).unwrap().unwrap();
        if status.plan_state == state {
            return status;
        }
        if state != "Failed" {
            assert_ne!(status.plan_state, "Failed", "{}", status.failure_message);
        }
        assert!(
            Instant::now() < deadline,
            "plan never reached {state}: plan_state={} stage={} {}",
            status.plan_state,
            status.stage,
            status.failure_message
        );
        thread::sleep(Duration::from_millis(10));
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

fn authorize(engine: &OperationEngine, plan_id: &str) {
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
            std::slice::from_ref(&id),
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

    authorize(&engine, &plan.id);
    start_cleanup(&cleaner, OWNER, &plan.id).expect("start");
    let status = wait_terminal(&cleaner, &plan.id, "Completed");
    assert!(status.mutation_started);
    assert_eq!(status.reclaimed_bytes, 60);
    assert_eq!(status.skipped_bytes, 40);
    assert_eq!(status.items[0].result_code, "Partial");
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
    authorize(&engine, &plan.id);
    start_cleanup(&cleaner, OWNER, &plan.id).expect("start");

    let status = wait_terminal(&cleaner, &plan.id, "Failed");
    assert!(status.mutation_started);
    assert!(status.recovery_required);
    // DBT-P63-012: the Recovery panel reads recovery records, not the journal flag.
    let records = db.recovery_records(20).expect("recovery records");
    assert!(records.iter().any(|r| r.plan_id == plan.id), "{records:?}");
    let _ = std::fs::remove_dir_all(root);
}

/// DBT-P63-012: nobody may see Completed before the record that explains it. The observer runs
/// on the worker at the "Completed" publish, which follows the terminal transition; the
/// journal used to be written only after it, so this read saw `Verifying` with no completion.
#[test]
fn completed_plan_is_never_visible_before_its_completed_journal() {
    let root = temp_root("completed-journal");
    std::fs::create_dir_all(&root).expect("root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("db"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let observer = {
        let (db, seen) = (db.clone(), seen.clone());
        move |t: ProgressTelemetry| {
            if t.stage == "Completed" {
                let plan = db.get_plan(&t.plan_id).expect("plan").expect("plan row");
                let record = db
                    .get_maintenance_execution(&t.plan_id)
                    .expect("record")
                    .expect("record row");
                seen.lock().expect("seen").push((
                    plan.state,
                    record.stage,
                    record.completed_unix_ms.is_some(),
                ));
            }
        }
    };
    let cleaner = CleanupEngine::with_platform_and_telemetry(
        engine.clone(),
        db.clone(),
        Arc::new(FakeCleanupPlatform { fail_delete: false }),
        ProgressTelemetryStore::with_observer(Arc::new(observer)),
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
    authorize(&engine, &plan.id);
    start_cleanup(&cleaner, OWNER, &plan.id).expect("start");

    let deadline = Instant::now() + Duration::from_secs(5);
    while seen.lock().expect("seen").is_empty() {
        assert!(Instant::now() < deadline, "no Completed publish observed");
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        *seen.lock().expect("seen"),
        vec![("Completed".to_owned(), "Completed".to_owned(), true)]
    );
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
    authorize(&engine, &plan.id);
    start_cleanup(&cleaner, OWNER, &plan.id).expect("start");

    let status = wait_terminal(&cleaner, &plan.id, "Failed");
    assert_eq!(asked.load(Ordering::SeqCst), 1);
    assert!(
        status
            .failure_message
            .contains("another AetherCore maintenance mutation is active"),
        "{}",
        status.failure_message
    );
    assert!(!status.mutation_started);
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
