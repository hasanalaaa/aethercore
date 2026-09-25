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
    RepairAssessmentState, RepairCheck, RepairCoordinator, RepairError, RepairPlatform,
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
    fn assess(&self) -> aethercore_system_repair::Result<(String, Vec<RepairCheck>)> {
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
