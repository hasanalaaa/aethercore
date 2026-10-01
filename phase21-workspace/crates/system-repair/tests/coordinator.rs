use std::{
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use aethercore_operation_engine::{OperationEngine, PlanState, SystemRepairAction};
use aethercore_operation_kernel::{
    MutationSupervisor, MutationWorkload, ReadBudgetManager, ReadWorkload,
};
use aethercore_persistence::Database;
use aethercore_system_repair::{
    AssessStep, RepairAssessmentState, RepairCheck, RepairCoordinator, RepairError, RepairPlatform,
};

fn wait_terminal(
    coordinator: &RepairCoordinator,
    plan_id: &str,
    state: &str,
) -> aethercore_system_repair::RepairExecutionStatus {
    // `status()` composes `plan_state` from the operation engine and every record field -
    // `mutation_started`, `recovery_required`, `outcome` - from the database. The terminal
    // transition now commits that record in the same transaction, and `status()` reads the
    // plan before the record (DBT-P63-012), so a loop keyed on `plan_state` alone sees the
    // final record.
    //
    // It did not while `fail_repair` and the verification path transitioned BEFORE they
    // upserted (`DBT-P62-003` in a second crate): run `34773880960` ran this very test twice
    // on one runner from one commit - step 13 `ok`, step 18 `FAILED` on
    // `assertion failed: status.recovery_required` - and this loop had to wait on `stage` too.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let status = coordinator
            .status(OWNER, Some(plan_id))
            .expect("status")
            .expect("execution");
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

fn start_repair(
    coordinator: &RepairCoordinator,
    owner: &str,
    plan_id: &str,
) -> aethercore_system_repair::Result<aethercore_system_repair::RepairExecutionStatus> {
    let supervisor = MutationSupervisor::new();
    let lease = supervisor
        .try_acquire(MutationWorkload::SystemRepair, plan_id, owner)
        .expect("mutation lease");
    coordinator.start_with_lease(owner, plan_id, lease)
}

fn start_assessment(
    coordinator: &RepairCoordinator,
    owner: &str,
) -> aethercore_system_repair::Result<aethercore_system_repair::RepairAssessment> {
    let budget = ReadBudgetManager::new(4);
    let lease = budget
        .try_acquire(ReadWorkload::RepairAssessment)
        .expect("read budget lease");
    coordinator.start_assessment_with_lease(owner, lease)
}

struct FakeRepairPlatform {
    fail_before_mutation: bool,
    fail_after_mutation: bool,
    events: Mutex<Vec<&'static str>>,
}

impl FakeRepairPlatform {
    fn event(&self, value: &'static str) {
        self.events.lock().expect("events").push(value);
    }
}

impl RepairPlatform for FakeRepairPlatform {
    fn assess(
        &self,
        _cancel: &Arc<std::sync::atomic::AtomicBool>,
        _progress: &mut dyn FnMut(AssessStep<'_>),
    ) -> aethercore_system_repair::Result<(String, Vec<RepairCheck>)> {
        self.event("assess");
        Ok((
            "C:".into(),
            vec![RepairCheck {
                id: "dism-check".into(),
                title: "Component store quick check".into(),
                stage: "Attention".into(),
                result_code: "ComponentStoreRepairable".into(),
                exit_code: 1,
                detail: "repairable corruption detected by synthetic evidence".into(),
                log_hint: "DISM.log".into(),
            }],
        ))
    }

    fn repair(
        &self,
        _action: &SystemRepairAction,
        _control: &mut aethercore_system_repair::RepairControl<'_>,
        begin_mutation: &mut dyn FnMut() -> aethercore_system_repair::Result<()>,
        emit: &mut dyn FnMut(RepairCheck),
    ) -> aethercore_system_repair::Result<()> {
        self.event("preflight");
        emit(RepairCheck {
            id: "dism-scan".into(),
            title: "DISM ScanHealth".into(),
            stage: "Completed".into(),
            result_code: "ExitCode0".into(),
            exit_code: 0,
            detail: "read-only servicing evidence".into(),
            log_hint: "DISM.log".into(),
        });
        if self.fail_before_mutation {
            return Err(RepairError::ServicingBusy);
        }
        begin_mutation()?;
        self.event("mutation");
        if self.fail_after_mutation {
            return Err(RepairError::Command("injected repair failure".into()));
        }
        emit(RepairCheck {
            id: "dism-restore".into(),
            title: "DISM RestoreHealth".into(),
            stage: "Completed".into(),
            result_code: "ExitCode0".into(),
            exit_code: 0,
            detail: "repaired".into(),
            log_hint: "DISM.log".into(),
        });
        Ok(())
    }

    fn verify(
        &self,
        _action: &SystemRepairAction,
        _control: &mut aethercore_system_repair::RepairControl<'_>,
        emit: &mut dyn FnMut(RepairCheck),
    ) -> aethercore_system_repair::Result<()> {
        self.event("verify");
        emit(RepairCheck {
            id: "verify-dism".into(),
            title: "Verify component store".into(),
            stage: "Completed".into(),
            result_code: "ComponentStoreHealthy".into(),
            exit_code: 0,
            detail: "verified healthy after repair".into(),
            log_hint: "DISM.log".into(),
        });
        emit(RepairCheck {
            id: "verify-sfc".into(),
            title: "Verify protected system files".into(),
            stage: "Completed".into(),
            result_code: "SystemFilesHealthy".into(),
            exit_code: 0,
            detail: "verified".into(),
            log_hint: "CBS.log".into(),
        });
        Ok(())
    }
}

fn temp_root(label: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "aethercore-phase4-repair-{label}-{}-{nonce}",
        std::process::id()
    ))
}

fn wait_assessment(coordinator: &RepairCoordinator) -> String {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let assessment = coordinator
            .assessment_for_owner(OWNER)
            .expect("owned assessment");
        if assessment.state == RepairAssessmentState::Ready {
            return assessment.assessment_id;
        }
        assert_ne!(assessment.state, RepairAssessmentState::Failed);
        assert!(Instant::now() < deadline, "assessment timed out");
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
fn repair_plan_is_authorized_and_mutation_barrier_is_durable() {
    let root = temp_root("success");
    std::fs::create_dir_all(&root).expect("temp root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("db"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let platform = Arc::new(FakeRepairPlatform {
        fail_before_mutation: false,
        fail_after_mutation: false,
        events: Mutex::new(Vec::new()),
    });
    let coordinator =
        RepairCoordinator::with_platform(engine.clone(), db.clone(), platform.clone());

    start_assessment(&coordinator, OWNER).expect("start assessment");
    let assessment_id = wait_assessment(&coordinator);
    let plan = coordinator
        .create_plan(OWNER, &assessment_id, true)
        .expect("create plan");
    assert_eq!(plan.state, PlanState::AwaitingAuthorization);
    assert!(matches!(
        start_repair(&coordinator, OWNER, &plan.id),
        Err(RepairError::AuthorizationRequired)
    ));
    authorize(&engine, &plan.id);
    start_repair(&coordinator, OWNER, &plan.id).expect("start repair");

    let status = wait_terminal(&coordinator, &plan.id, "Completed");
    assert!(status.mutation_started);
    assert!(!status.recovery_required);
    assert!(status.steps.iter().any(|step| step.id == "dism-restore"));

    let events = platform.events.lock().expect("events").clone();
    let preflight = events
        .iter()
        .position(|event| *event == "preflight")
        .unwrap();
    let mutation = events
        .iter()
        .position(|event| *event == "mutation")
        .unwrap();
    assert!(preflight < mutation);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn servicing_failure_before_barrier_does_not_require_recovery() {
    let root = temp_root("preflight-fail");
    std::fs::create_dir_all(&root).expect("temp root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("db"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let coordinator = RepairCoordinator::with_platform(
        engine.clone(),
        db.clone(),
        Arc::new(FakeRepairPlatform {
            fail_before_mutation: true,
            fail_after_mutation: false,
            events: Mutex::new(Vec::new()),
        }),
    );

    start_assessment(&coordinator, OWNER).expect("start assessment");
    let assessment_id = wait_assessment(&coordinator);
    let plan = coordinator
        .create_plan(OWNER, &assessment_id, false)
        .expect("plan");
    authorize(&engine, &plan.id);
    start_repair(&coordinator, OWNER, &plan.id).expect("start");

    let status = wait_terminal(&coordinator, &plan.id, "Failed");
    assert!(!status.mutation_started);
    assert!(!status.recovery_required);
    assert!(status.steps.iter().any(|step| step.id == "dism-scan"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn failure_after_barrier_requires_recovery_review() {
    let root = temp_root("mutation-fail");
    std::fs::create_dir_all(&root).expect("temp root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("db"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let coordinator = RepairCoordinator::with_platform(
        engine.clone(),
        db.clone(),
        Arc::new(FakeRepairPlatform {
            fail_before_mutation: false,
            fail_after_mutation: true,
            events: Mutex::new(Vec::new()),
        }),
    );

    start_assessment(&coordinator, OWNER).expect("start assessment");
    let assessment_id = wait_assessment(&coordinator);
    let plan = coordinator
        .create_plan(OWNER, &assessment_id, false)
        .expect("plan");
    authorize(&engine, &plan.id);
    start_repair(&coordinator, OWNER, &plan.id).expect("start");

    let status = wait_terminal(&coordinator, &plan.id, "Failed");
    assert!(status.mutation_started);
    assert!(status.recovery_required);
    // DBT-P63-012: the Recovery panel reads recovery records, not the journal flag.
    let records = db.recovery_records(20).expect("recovery records");
    assert!(records.iter().any(|r| r.plan_id == plan.id), "{records:?}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn repair_assessment_is_principal_bound() {
    let root = temp_root("ownership");
    std::fs::create_dir_all(&root).expect("temp root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("db"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let coordinator = RepairCoordinator::with_platform(
        engine,
        db,
        Arc::new(FakeRepairPlatform {
            fail_before_mutation: false,
            fail_after_mutation: false,
            events: Mutex::new(Vec::new()),
        }),
    );
    start_assessment(&coordinator, OWNER).expect("start assessment");
    let _ = wait_assessment(&coordinator);
    let other = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    assert!(matches!(
        coordinator.assessment_for_owner(other),
        Err(RepairError::OwnershipMismatch)
    ));
    let _ = std::fs::remove_dir_all(root);
}

// P76 DBT-P75-027: the crate-level SQL probes P75 ran and could not commit (they need this
// dev-dependency). Triggers installed from a second connection on the test database watch
// the real coordinator: what the journal says at the instant the plan turns terminal, and
// what is left when the journal write itself fails.
const FLIP_PROBE: &str =
    "CREATE TABLE probe(stage TEXT, recovery_required INTEGER, completed INTEGER);
     CREATE TRIGGER probe_flip AFTER UPDATE OF state ON plans
     WHEN NEW.state IN ('Failed','Completed') BEGIN
       INSERT INTO probe SELECT stage, recovery_required, completed_unix_ms IS NOT NULL
       FROM maintenance_executions WHERE plan_id = NEW.id;
     END;";
const JOURNAL_ABORT: &str =
    "CREATE TRIGGER probe_abort_insert BEFORE INSERT ON maintenance_executions
     WHEN NEW.stage IN ('Failed','Completed') BEGIN SELECT RAISE(ABORT, 'probe'); END;
     CREATE TRIGGER probe_abort_update BEFORE UPDATE ON maintenance_executions
     WHEN NEW.stage IN ('Failed','Completed') BEGIN SELECT RAISE(ABORT, 'probe'); END;";

/// Runs one repair to its end with `probe` installed; returns the plan id.
fn probed_repair(
    label: &str,
    fail_after_mutation: bool,
    probe: &str,
) -> (std::path::PathBuf, Arc<Database>, RepairCoordinator, String) {
    let root = temp_root(label);
    std::fs::create_dir_all(&root).expect("temp root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("db"));
    rusqlite::Connection::open(root.join("state.db"))
        .expect("probe connection")
        .execute_batch(probe)
        .expect("install probe");
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let coordinator = RepairCoordinator::with_platform(
        engine.clone(),
        db.clone(),
        Arc::new(FakeRepairPlatform {
            fail_before_mutation: false,
            fail_after_mutation,
            events: Mutex::new(Vec::new()),
        }),
    );
    start_assessment(&coordinator, OWNER).expect("start assessment");
    let assessment_id = wait_assessment(&coordinator);
    let plan = coordinator
        .create_plan(OWNER, &assessment_id, false)
        .expect("plan");
    authorize(&engine, &plan.id);
    start_repair(&coordinator, OWNER, &plan.id).expect("start");
    (root, db, coordinator, plan.id)
}

fn probe_rows(root: &std::path::Path) -> Vec<(String, i64, bool)> {
    let conn = rusqlite::Connection::open(root.join("state.db")).expect("probe connection");
    let mut stmt = conn.prepare("SELECT * FROM probe").expect("probe query");
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .expect("probe rows")
        .map(|row| row.expect("probe row"))
        .collect()
}

#[test]
fn sql_probe_a_failed_repair_is_journaled_at_the_instant_it_turns_terminal() {
    let (root, _db, coordinator, plan) = probed_repair("probe-failed", true, FLIP_PROBE);
    wait_terminal(&coordinator, &plan, "Failed");
    assert_eq!(probe_rows(&root), vec![("Failed".to_owned(), 1, true)]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn sql_probe_a_completed_repair_is_journaled_at_the_instant_it_turns_terminal() {
    let (root, _db, coordinator, plan) = probed_repair("probe-completed", false, FLIP_PROBE);
    wait_terminal(&coordinator, &plan, "Completed");
    assert_eq!(probe_rows(&root), vec![("Completed".to_owned(), 0, true)]);
    let _ = std::fs::remove_dir_all(root);
}

/// A journal write that fails leaves the plan non-terminal, where recovery finds it.
#[test]
fn sql_probe_a_repair_whose_journal_write_fails_stays_recoverable() {
    let (root, db, coordinator, plan) = probed_repair("probe-abort", true, JOURNAL_ABORT);
    // The unprobed failure reaches Failed in milliseconds (the test above); two seconds is
    // the window in which this one would have, had the aborted journal not stopped it.
    let window = Instant::now() + Duration::from_secs(2);
    while Instant::now() < window {
        let status = coordinator.status(OWNER, Some(&plan)).unwrap().unwrap();
        assert_ne!(
            status.plan_state, "Failed",
            "plan Failed is terminal although its journal was never written"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let state = db.get_plan(&plan).expect("plan").expect("row").state;
    assert!(
        matches!(state.as_str(), "Executing" | "Verifying"),
        "{state}"
    );
    coordinator.recover_incomplete().expect("recovery");
    let records = db.recovery_records(20).expect("recovery records");
    assert!(records.iter().any(|r| r.plan_id == plan), "{records:?}");
    let _ = std::fs::remove_dir_all(root);
}

/// P76 DBT-P76-007: an assessment whose platform panics. The worker used to die and leave
/// the state Scanning for the life of the service, refusing every later start as Busy.
struct PanickingAssessment;

impl RepairPlatform for PanickingAssessment {
    fn assess(
        &self,
        _cancel: &Arc<std::sync::atomic::AtomicBool>,
        _progress: &mut dyn FnMut(AssessStep<'_>),
    ) -> aethercore_system_repair::Result<(String, Vec<RepairCheck>)> {
        panic!("a check blew up")
    }
    fn repair(
        &self,
        _: &SystemRepairAction,
        _control: &mut aethercore_system_repair::RepairControl<'_>,
        _: &mut dyn FnMut() -> aethercore_system_repair::Result<()>,
        _: &mut dyn FnMut(RepairCheck),
    ) -> aethercore_system_repair::Result<()> {
        Ok(())
    }
    fn verify(
        &self,
        _: &SystemRepairAction,
        _control: &mut aethercore_system_repair::RepairControl<'_>,
        _: &mut dyn FnMut(RepairCheck),
    ) -> aethercore_system_repair::Result<()> {
        Ok(())
    }
}

#[test]
fn an_assessment_whose_platform_panics_fails_instead_of_staying_assessing() {
    let root = temp_root("assess-panic");
    std::fs::create_dir_all(&root).expect("temp root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("db"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let coordinator = RepairCoordinator::with_platform(engine, db, Arc::new(PanickingAssessment));
    start_assessment(&coordinator, OWNER).expect("start");
    let deadline = Instant::now() + Duration::from_secs(5);
    let state = loop {
        let current = coordinator.assessment_for_owner(OWNER).expect("snapshot");
        if current.state != RepairAssessmentState::Scanning || Instant::now() > deadline {
            break current;
        }
        thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(state.state, RepairAssessmentState::Failed, "{state:?}");
    assert!(state.completed_unix_ms >= state.started_unix_ms);
    // And the next assessment is not refused as Busy.
    start_assessment(&coordinator, OWNER).expect("a new assessment starts");
    let _ = std::fs::remove_dir_all(root);
}

/// A slow assessment: one check finishes, the next runs until cancelled.
struct SlowAssessment;

impl RepairPlatform for SlowAssessment {
    fn assess(
        &self,
        cancel: &Arc<std::sync::atomic::AtomicBool>,
        progress: &mut dyn FnMut(AssessStep<'_>),
    ) -> aethercore_system_repair::Result<(String, Vec<RepairCheck>)> {
        let done = RepairCheck {
            id: "dism-scan".into(),
            title: "Component store".into(),
            stage: "Completed".into(),
            result_code: "ComponentStoreHealthy".into(),
            ..RepairCheck::default()
        };
        progress(AssessStep::Started("dism-scan"));
        progress(AssessStep::Finished(&done));
        progress(AssessStep::Started("sfc-verify"));
        let deadline = Instant::now() + Duration::from_secs(10);
        while !cancel.load(std::sync::atomic::Ordering::SeqCst) {
            assert!(Instant::now() < deadline, "never cancelled");
            thread::sleep(Duration::from_millis(10));
        }
        Err(RepairError::Cancelled)
    }
    fn repair(
        &self,
        _: &SystemRepairAction,
        _control: &mut aethercore_system_repair::RepairControl<'_>,
        _: &mut dyn FnMut() -> aethercore_system_repair::Result<()>,
        _: &mut dyn FnMut(RepairCheck),
    ) -> aethercore_system_repair::Result<()> {
        Ok(())
    }
    fn verify(
        &self,
        _: &SystemRepairAction,
        _control: &mut aethercore_system_repair::RepairControl<'_>,
        _: &mut dyn FnMut(RepairCheck),
    ) -> aethercore_system_repair::Result<()> {
        Ok(())
    }
}

/// P76 DBT-P76-007: "Assessing…" for 10+ minutes on the owner's install gave no sign of
/// life. The snapshot now carries each finished check and the one running, and the owner
/// can stop it, keeping what was measured.
#[test]
fn a_running_assessment_shows_its_progress_and_can_be_cancelled() {
    let root = temp_root("assess-cancel");
    std::fs::create_dir_all(&root).expect("temp root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("db"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let coordinator = RepairCoordinator::with_platform(engine, db, Arc::new(SlowAssessment));
    start_assessment(&coordinator, OWNER).expect("start");
    let deadline = Instant::now() + Duration::from_secs(5);
    let running = loop {
        let current = coordinator.assessment_for_owner(OWNER).expect("snapshot");
        if current.current_check_id == "sfc-verify" || Instant::now() > deadline {
            break current;
        }
        thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(running.state, RepairAssessmentState::Scanning);
    assert_eq!(running.current_check_id, "sfc-verify", "{running:?}");
    assert_eq!(running.checks.len(), 1);
    assert_eq!(running.checks[0].id, "dism-scan");

    coordinator.cancel_assessment(OWNER, "").expect("cancel");
    let deadline = Instant::now() + Duration::from_secs(5);
    let stopped = loop {
        let current = coordinator.assessment_for_owner(OWNER).expect("snapshot");
        if current.state != RepairAssessmentState::Scanning || Instant::now() > deadline {
            break current;
        }
        thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(
        stopped.state,
        RepairAssessmentState::Cancelled,
        "{stopped:?}"
    );
    assert_eq!(stopped.checks.len(), 1, "what was measured is kept");
    assert!(stopped.current_check_id.is_empty());
    assert!(coordinator.cancel_assessment("someone-else", "").is_err());
    let _ = std::fs::remove_dir_all(root);
}

/// P78-01: the completion time is taken when the checks end, not when they start. An assessment
/// that took time reported `completed == started`, so a slow scan looked instant and "last checked"
/// was wrong by the length of the scan.
struct TimedAssessment {
    ended_unix_ms: Arc<std::sync::atomic::AtomicI64>,
}

impl RepairPlatform for TimedAssessment {
    fn assess(
        &self,
        _cancel: &Arc<std::sync::atomic::AtomicBool>,
        _progress: &mut dyn FnMut(AssessStep<'_>),
    ) -> aethercore_system_repair::Result<(String, Vec<RepairCheck>)> {
        thread::sleep(Duration::from_millis(80));
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_millis() as i64;
        self.ended_unix_ms
            .store(now, std::sync::atomic::Ordering::SeqCst);
        Ok(("C:".into(), Vec::new()))
    }
    fn repair(
        &self,
        _: &SystemRepairAction,
        _control: &mut aethercore_system_repair::RepairControl<'_>,
        _: &mut dyn FnMut() -> aethercore_system_repair::Result<()>,
        _: &mut dyn FnMut(RepairCheck),
    ) -> aethercore_system_repair::Result<()> {
        Ok(())
    }
    fn verify(
        &self,
        _: &SystemRepairAction,
        _control: &mut aethercore_system_repair::RepairControl<'_>,
        _: &mut dyn FnMut(RepairCheck),
    ) -> aethercore_system_repair::Result<()> {
        Ok(())
    }
}

#[test]
fn an_assessment_completes_after_its_measurement_ended() {
    let root = temp_root("assess-completed");
    std::fs::create_dir_all(&root).expect("temp root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("db"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let ended = Arc::new(std::sync::atomic::AtomicI64::new(0));
    let platform = Arc::new(TimedAssessment {
        ended_unix_ms: ended.clone(),
    });
    let coordinator = RepairCoordinator::with_platform(engine, db, platform);
    start_assessment(&coordinator, OWNER).expect("start");
    let deadline = Instant::now() + Duration::from_secs(5);
    let state = loop {
        let current = coordinator.assessment_for_owner(OWNER).expect("snapshot");
        if current.state != RepairAssessmentState::Scanning || Instant::now() > deadline {
            break current;
        }
        thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(state.state, RepairAssessmentState::Ready, "{state:?}");
    let ended = ended.load(std::sync::atomic::Ordering::SeqCst);
    assert!(ended > 0, "the platform never finished");
    assert!(
        state.completed_unix_ms >= ended,
        "completed {} is before the measurement ended {ended}",
        state.completed_unix_ms
    );
    assert!(
        state.completed_unix_ms - state.started_unix_ms >= 80,
        "{state:?}"
    );
    let _ = std::fs::remove_dir_all(root);
}

/// P78-03: cancel verbs carry the id of what they cancel. An id from an older assessment cancels
/// nothing, repeating a cancel is safe, and another owner is refused.
#[test]
fn an_assessment_cancel_names_its_target_and_is_safe_to_repeat() {
    let root = temp_root("assess-cancel-id");
    std::fs::create_dir_all(&root).expect("temp root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("db"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let coordinator = RepairCoordinator::with_platform(engine, db, Arc::new(SlowAssessment));
    let started = start_assessment(&coordinator, OWNER).expect("start");
    let deadline = Instant::now() + Duration::from_secs(5);
    while coordinator
        .assessment_for_owner(OWNER)
        .expect("snapshot")
        .current_check_id
        != "sfc-verify"
    {
        assert!(
            Instant::now() < deadline,
            "the assessment never reached its slow check"
        );
        thread::sleep(Duration::from_millis(10));
    }

    // An id from an older assessment must not stop this one.
    let stale = coordinator
        .cancel_assessment(OWNER, "an-older-assessment")
        .expect("stale cancel");
    assert_eq!(stale.state, RepairAssessmentState::Scanning);
    thread::sleep(Duration::from_millis(150));
    let still = coordinator.assessment_for_owner(OWNER).expect("snapshot");
    assert_eq!(
        still.state,
        RepairAssessmentState::Scanning,
        "a stale id cancelled the current assessment"
    );

    // Another owner is refused, whatever id it names.
    assert!(
        coordinator
            .cancel_assessment("someone-else", &started.assessment_id)
            .is_err()
    );

    // The right id stops it, and asking again is not an error.
    coordinator
        .cancel_assessment(OWNER, &started.assessment_id)
        .expect("cancel");
    let deadline = Instant::now() + Duration::from_secs(5);
    let stopped = loop {
        let current = coordinator.assessment_for_owner(OWNER).expect("snapshot");
        if current.state != RepairAssessmentState::Scanning || Instant::now() > deadline {
            break current;
        }
        thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(
        stopped.state,
        RepairAssessmentState::Cancelled,
        "{stopped:?}"
    );
    let again = coordinator
        .cancel_assessment(OWNER, &started.assessment_id)
        .expect("repeat");
    assert_eq!(again.state, RepairAssessmentState::Cancelled);
    let _ = std::fs::remove_dir_all(root);
}

/// A repair platform that waits before the mutation barrier (or after it) until released.
struct GatedRepair {
    hold_before_barrier: std::sync::atomic::AtomicBool,
    hold_after_barrier: std::sync::atomic::AtomicBool,
    waiting: std::sync::atomic::AtomicBool,
    release: std::sync::atomic::AtomicBool,
    mutations: std::sync::atomic::AtomicUsize,
}

impl GatedRepair {
    fn new() -> Self {
        Self {
            hold_before_barrier: false.into(),
            hold_after_barrier: false.into(),
            waiting: false.into(),
            release: false.into(),
            mutations: 0.into(),
        }
    }
    fn hold(&self, before_barrier: bool) {
        self.waiting
            .store(false, std::sync::atomic::Ordering::SeqCst);
        self.release
            .store(false, std::sync::atomic::Ordering::SeqCst);
        self.hold_before_barrier
            .store(before_barrier, std::sync::atomic::Ordering::SeqCst);
        self.hold_after_barrier
            .store(!before_barrier, std::sync::atomic::Ordering::SeqCst);
    }
    fn wait_until_waiting(&self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !self.waiting.load(std::sync::atomic::Ordering::SeqCst) {
            assert!(
                Instant::now() < deadline,
                "the repair never reached its wait"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }
    fn park(&self) {
        self.waiting
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !self.release.load(std::sync::atomic::Ordering::SeqCst) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
    }
}

impl RepairPlatform for GatedRepair {
    fn assess(
        &self,
        cancel: &Arc<std::sync::atomic::AtomicBool>,
        progress: &mut dyn FnMut(AssessStep<'_>),
    ) -> aethercore_system_repair::Result<(String, Vec<RepairCheck>)> {
        FakeRepairPlatform {
            fail_before_mutation: false,
            fail_after_mutation: false,
            events: Mutex::new(Vec::new()),
        }
        .assess(cancel, progress)
    }
    fn repair(
        &self,
        _: &SystemRepairAction,
        _control: &mut aethercore_system_repair::RepairControl<'_>,
        begin_mutation: &mut dyn FnMut() -> aethercore_system_repair::Result<()>,
        _: &mut dyn FnMut(RepairCheck),
    ) -> aethercore_system_repair::Result<()> {
        if self
            .hold_before_barrier
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            self.park();
        }
        begin_mutation()?;
        self.mutations
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self
            .hold_after_barrier
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            self.park();
        }
        Ok(())
    }
    fn verify(
        &self,
        _: &SystemRepairAction,
        _control: &mut aethercore_system_repair::RepairControl<'_>,
        emit: &mut dyn FnMut(RepairCheck),
    ) -> aethercore_system_repair::Result<()> {
        for (id, code) in [
            ("verify-dism", "ComponentStoreHealthy"),
            ("verify-sfc", "SystemFilesHealthy"),
        ] {
            emit(RepairCheck {
                id: id.into(),
                title: id.into(),
                stage: "Completed".into(),
                result_code: code.into(),
                ..RepairCheck::default()
            });
        }
        Ok(())
    }
}

fn gated_plan(
    coordinator: &RepairCoordinator,
    engine: &OperationEngine,
    platform: &GatedRepair,
    before_barrier: bool,
) -> String {
    start_assessment(coordinator, OWNER).expect("start assessment");
    let assessment_id = wait_assessment(coordinator);
    let plan = coordinator
        .create_plan(OWNER, &assessment_id, false)
        .expect("plan");
    authorize(engine, &plan.id);
    platform.hold(before_barrier);
    start_repair(coordinator, OWNER, &plan.id).expect("start");
    plan.id
}

/// P78-03: a cancel that arrives before the mutation barrier stops the repair from crossing it.
/// The plan ends failed-before-mutation with nothing to recover; no command ran.
#[test]
fn a_repair_cancelled_before_the_barrier_never_crosses_it() {
    let root = temp_root("repair-cancel-before");
    std::fs::create_dir_all(&root).expect("temp root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("db"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let platform = Arc::new(GatedRepair::new());
    let coordinator = RepairCoordinator::with_platform(engine.clone(), db, platform.clone());
    let plan = gated_plan(&coordinator, &engine, &platform, true);
    platform.wait_until_waiting();

    // Another owner and an unknown plan are refused; neither touches this repair.
    assert!(coordinator.cancel_repair("someone-else", &plan).is_err());
    assert!(coordinator.cancel_repair(OWNER, "no-such-plan").is_err());
    coordinator.cancel_repair(OWNER, &plan).expect("cancel");
    coordinator
        .cancel_repair(OWNER, &plan)
        .expect("cancel again");
    platform
        .release
        .store(true, std::sync::atomic::Ordering::SeqCst);

    let status = wait_terminal(&coordinator, &plan, "Failed");
    assert!(!status.mutation_started, "{status:?}");
    assert!(!status.recovery_required, "{status:?}");
    assert_eq!(status.outcome, "FailedBeforeMutation");
    assert!(
        status.failure_message.contains("cancelled"),
        "{}",
        status.failure_message
    );
    assert_eq!(
        platform.mutations.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "a command ran after the cancel"
    );
    // The finished plan answers a repeated cancel with its state, and the journal is untouched.
    let after = coordinator
        .cancel_repair(OWNER, &plan)
        .expect("cancel a finished plan")
        .expect("status");
    assert_eq!(after.plan_state, "Failed");
    let _ = std::fs::remove_dir_all(root);
}

/// The other two edges: an id from an older plan does not cancel the plan that replaced it, and a
/// cancel after the barrier stops nothing (a real stop of the running tool is P85) - the repair
/// finishes and is verified as it would have been.
#[test]
fn a_stale_plan_id_stops_nothing_and_a_current_cancel_stops_at_the_next_boundary() {
    let root = temp_root("repair-cancel-edges");
    std::fs::create_dir_all(&root).expect("temp root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("db"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let platform = Arc::new(GatedRepair::new());
    let coordinator = RepairCoordinator::with_platform(engine.clone(), db, platform.clone());

    let older = gated_plan(&coordinator, &engine, &platform, false);
    platform.wait_until_waiting();
    platform
        .release
        .store(true, std::sync::atomic::Ordering::SeqCst);
    wait_terminal(&coordinator, &older, "Completed");

    let newer = gated_plan(&coordinator, &engine, &platform, true);
    platform.wait_until_waiting();
    coordinator
        .cancel_repair(OWNER, &older)
        .expect("an older plan's cancel is not an error");
    platform
        .release
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let status = wait_terminal(&coordinator, &newer, "Completed");
    assert!(
        status.mutation_started,
        "an older plan's id cancelled the newer plan"
    );

    let past = gated_plan(&coordinator, &engine, &platform, false);
    platform.wait_until_waiting();
    coordinator
        .cancel_repair(OWNER, &past)
        .expect("cancel after the barrier");
    platform
        .release
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let status = wait_terminal(&coordinator, &past, "Failed");
    assert!(
        status.mutation_started && status.recovery_required,
        "{status:?}"
    );
    let _ = std::fs::remove_dir_all(root);
}

/// P85-02: a repair whose tool reports its own progress and can be stopped. After the platform
/// crosses the barrier it reports `None` (the tool cannot say), then 40, then 100, each only after the
/// test has looked at the status; or it waits for the owner's cancel and stops.
struct ToolRepair {
    reported: std::sync::atomic::AtomicUsize,
    allow: std::sync::atomic::AtomicUsize,
    stop_on_cancel: bool,
}

impl ToolRepair {
    fn wait(&self, counter: &std::sync::atomic::AtomicUsize, at_least: usize) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while counter.load(std::sync::atomic::Ordering::SeqCst) < at_least {
            assert!(
                Instant::now() < deadline,
                "the test never released the repair"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }
}

impl RepairPlatform for ToolRepair {
    fn assess(
        &self,
        cancel: &Arc<std::sync::atomic::AtomicBool>,
        progress: &mut dyn FnMut(AssessStep<'_>),
    ) -> aethercore_system_repair::Result<(String, Vec<RepairCheck>)> {
        FakeRepairPlatform {
            fail_before_mutation: false,
            fail_after_mutation: false,
            events: Mutex::new(Vec::new()),
        }
        .assess(cancel, progress)
    }
    fn repair(
        &self,
        _: &SystemRepairAction,
        control: &mut aethercore_system_repair::RepairControl<'_>,
        begin_mutation: &mut dyn FnMut() -> aethercore_system_repair::Result<()>,
        _: &mut dyn FnMut(RepairCheck),
    ) -> aethercore_system_repair::Result<()> {
        begin_mutation()?;
        if self.stop_on_cancel {
            self.reported.store(1, std::sync::atomic::Ordering::SeqCst);
            let deadline = Instant::now() + Duration::from_secs(5);
            while !control.cancel.load(std::sync::atomic::Ordering::SeqCst) {
                assert!(
                    Instant::now() < deadline,
                    "the cancel never reached the tool"
                );
                thread::sleep(Duration::from_millis(5));
            }
            return Err(RepairError::RepairStopped);
        }
        for (step, percent) in [None, Some(40), Some(100)].into_iter().enumerate() {
            (control.progress)(percent);
            self.reported
                .store(step + 1, std::sync::atomic::Ordering::SeqCst);
            self.wait(&self.allow, step + 1);
        }
        Ok(())
    }
    fn verify(
        &self,
        _: &SystemRepairAction,
        _control: &mut aethercore_system_repair::RepairControl<'_>,
        emit: &mut dyn FnMut(RepairCheck),
    ) -> aethercore_system_repair::Result<()> {
        for (id, code) in [
            ("verify-dism", "ComponentStoreHealthy"),
            ("verify-sfc", "SystemFilesHealthy"),
        ] {
            emit(RepairCheck {
                id: id.into(),
                title: id.into(),
                stage: "Completed".into(),
                result_code: code.into(),
                ..RepairCheck::default()
            });
        }
        Ok(())
    }
}

fn tool_plan(coordinator: &RepairCoordinator, engine: &OperationEngine) -> String {
    start_assessment(coordinator, OWNER).expect("start assessment");
    let assessment_id = wait_assessment(coordinator);
    let plan = coordinator
        .create_plan(OWNER, &assessment_id, false)
        .expect("plan");
    authorize(engine, &plan.id);
    start_repair(coordinator, OWNER, &plan.id).expect("start");
    plan.id
}

#[test]
fn a_tool_percent_reaches_the_status_and_no_percent_stays_unknown() {
    let root = temp_root("tool-progress");
    std::fs::create_dir_all(&root).expect("temp root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("db"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let platform = Arc::new(ToolRepair {
        reported: 0.into(),
        allow: 0.into(),
        stop_on_cancel: false,
    });
    let coordinator = RepairCoordinator::with_platform(engine.clone(), db, platform.clone());
    let plan = tool_plan(&coordinator, &engine);
    let status_after = |step: usize| {
        platform.wait(&platform.reported, step);
        coordinator
            .status(OWNER, Some(&plan))
            .expect("status")
            .expect("execution")
    };

    let unknown = status_after(1);
    assert!(
        !unknown.progress_known,
        "a tool that cannot say is indeterminate, not 0%: {unknown:?}"
    );
    platform.allow.store(1, std::sync::atomic::Ordering::SeqCst);
    let part = status_after(2);
    assert!(part.progress_known, "{part:?}");
    platform.allow.store(2, std::sync::atomic::Ordering::SeqCst);
    let full = status_after(3);
    assert!(
        full.progress_known && full.overall_percent > part.overall_percent,
        "progress only moves forward: {part:?} then {full:?}"
    );
    assert!(
        full.overall_percent < 100,
        "the tool finishing is not the plan finishing: verification is still to come"
    );
    platform.allow.store(3, std::sync::atomic::Ordering::SeqCst);
    let done = wait_terminal(&coordinator, &plan, "Completed");
    assert_eq!(done.overall_percent, 100);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_repair_stopped_after_the_barrier_needs_recovery_and_is_never_completed() {
    let root = temp_root("tool-stop");
    std::fs::create_dir_all(&root).expect("temp root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("db"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let platform = Arc::new(ToolRepair {
        reported: 0.into(),
        allow: 0.into(),
        stop_on_cancel: true,
    });
    let coordinator =
        RepairCoordinator::with_platform(engine.clone(), db.clone(), platform.clone());
    let plan = tool_plan(&coordinator, &engine);
    platform.wait(&platform.reported, 1);
    coordinator.cancel_repair(OWNER, &plan).expect("cancel");
    let status = wait_terminal(&coordinator, &plan, "Failed");
    assert!(
        status.mutation_started && status.recovery_required,
        "{status:?}"
    );
    assert_eq!(status.outcome, "FailedAfterMutation");
    assert!(
        status.failure_message.contains("stopped"),
        "{}",
        status.failure_message
    );
    assert!(
        db.recovery_records(20)
            .expect("recovery records")
            .iter()
            .any(|r| r.plan_id == plan),
        "the owner is told to review"
    );
    let _ = std::fs::remove_dir_all(root);
}

/// The real coordinator accepts a cancel while its first verification call is blocked.
/// The platform can honour it between checks, or return a late success regardless.
struct VerificationGate {
    entered: std::sync::mpsc::Sender<()>,
    release: Mutex<std::sync::mpsc::Receiver<()>>,
    next_checks: std::sync::atomic::AtomicUsize,
    ignores_cancel: bool,
}

impl RepairPlatform for VerificationGate {
    fn assess(
        &self,
        cancel: &Arc<std::sync::atomic::AtomicBool>,
        progress: &mut dyn FnMut(AssessStep<'_>),
    ) -> aethercore_system_repair::Result<(String, Vec<RepairCheck>)> {
        let (volume, mut checks) = FakeRepairPlatform {
            fail_before_mutation: false,
            fail_after_mutation: false,
            events: Mutex::new(Vec::new()),
        }
        .assess(cancel, progress)?;
        checks.push(RepairCheck {
            id: "sfc-verify".into(),
            title: "Protected file check".into(),
            result_code: "SystemFilesCorrupt".into(),
            stage: "Attention".into(),
            ..RepairCheck::default()
        });
        Ok((volume, checks))
    }
    fn repair(
        &self,
        _: &SystemRepairAction,
        _: &mut aethercore_system_repair::RepairControl<'_>,
        begin_mutation: &mut dyn FnMut() -> aethercore_system_repair::Result<()>,
        _: &mut dyn FnMut(RepairCheck),
    ) -> aethercore_system_repair::Result<()> {
        begin_mutation()
    }
    fn verify(
        &self,
        _: &SystemRepairAction,
        control: &mut aethercore_system_repair::RepairControl<'_>,
        emit: &mut dyn FnMut(RepairCheck),
    ) -> aethercore_system_repair::Result<()> {
        self.entered.send(()).unwrap();
        self.release.lock().unwrap().recv().unwrap();
        emit(RepairCheck {
            id: "verify-dism".into(),
            result_code: "ComponentStoreHealthy".into(),
            stage: "Completed".into(),
            ..RepairCheck::default()
        });
        if !self.ignores_cancel && control.cancel.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(RepairError::RepairStopped);
        }
        self.next_checks
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        emit(RepairCheck {
            id: "verify-sfc".into(),
            result_code: "SystemFilesHealthy".into(),
            stage: "Completed".into(),
            ..RepairCheck::default()
        });
        Ok(())
    }
}

fn cancel_during_verification(ignores_cancel: bool) {
    let root = temp_root("verification-cancel");
    std::fs::create_dir_all(&root).unwrap();
    let db = Arc::new(Database::open(root.join("state.db")).unwrap());
    let engine = Arc::new(OperationEngine::new(db.clone()));
    let (entered, entry) = std::sync::mpsc::channel();
    let (release, blocked) = std::sync::mpsc::channel();
    let platform = Arc::new(VerificationGate {
        entered,
        release: Mutex::new(blocked),
        next_checks: 0.into(),
        ignores_cancel,
    });
    let coordinator = RepairCoordinator::with_platform(engine.clone(), db, platform.clone());
    start_assessment(&coordinator, OWNER).unwrap();
    let assessment_id = wait_assessment(&coordinator);
    let plan = coordinator
        .create_plan(OWNER, &assessment_id, false)
        .unwrap();
    authorize(&engine, &plan.id);
    let supervisor = MutationSupervisor::new();
    let lease = supervisor
        .try_acquire(MutationWorkload::SystemRepair, &plan.id, OWNER)
        .unwrap();
    coordinator
        .start_with_lease(OWNER, &plan.id, lease)
        .unwrap();
    entry.recv_timeout(Duration::from_secs(5)).unwrap();
    coordinator.cancel_repair(OWNER, &plan.id).unwrap();
    assert!(
        supervisor.is_active(),
        "cancellation released a still-running verification worker"
    );
    assert!(
        supervisor
            .try_acquire(MutationWorkload::Cleanup, "another-plan", OWNER)
            .is_err()
    );
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        let status = coordinator.status(OWNER, Some(&plan.id)).unwrap().unwrap();
        if matches!(status.plan_state.as_str(), "Failed" | "Completed") {
            break status;
        }
        assert!(Instant::now() < deadline, "verification never completed");
        thread::sleep(Duration::from_millis(5));
    };
    if !ignores_cancel {
        assert_eq!(
            platform
                .next_checks
                .load(std::sync::atomic::Ordering::SeqCst),
            0,
            "a verification step started after cancellation at the previous safe boundary"
        );
    }
    assert_eq!(
        status.plan_state, "Failed",
        "cancelled verification claimed success"
    );
    assert_ne!(status.outcome, "SucceededVerified");
    assert_eq!(status.outcome, "FailedAfterMutation");
    assert!(status.mutation_started && status.recovery_required);
    assert_ne!(status.verification_state, "Verified");
    while supervisor.is_active() {
        assert!(
            Instant::now() < deadline,
            "the completed worker kept its mutation lease"
        );
        thread::yield_now();
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn p85_cancel_during_first_verification_prevents_the_next_check() {
    cancel_during_verification(false);
}

#[test]
fn p85_cancel_during_last_verification_rejects_a_late_success() {
    cancel_during_verification(true);
}
