//! P78-02A: an assessment check that never returns must end as an unknown check with a reason, must
//! not pile up workers under repeated assessments, and must not write over a newer assessment when it
//! finally does return. Deadlines here are tens of milliseconds; production ones are minutes.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use aethercore_operation_engine::{OperationEngine, SystemRepairAction};
use aethercore_operation_kernel::{ReadBudgetManager, ReadWorkload};
use aethercore_persistence::Database;
use aethercore_system_repair::{
    AssessStep, RepairAssessment, RepairAssessmentState, RepairCheck, RepairCoordinator,
    RepairPlatform, Result,
    bounded::{ProviderSlot, run_check},
};

const OWNER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
type Assess = dyn Fn(&Arc<AtomicBool>, &mut dyn FnMut(AssessStep<'_>)) -> Result<Vec<RepairCheck>>
    + Send
    + Sync;

/// A platform whose assessment is the closure; it never repairs or verifies.
struct Scripted(Box<Assess>);

impl RepairPlatform for Scripted {
    fn assess(
        &self,
        cancel: &Arc<AtomicBool>,
        progress: &mut dyn FnMut(AssessStep<'_>),
    ) -> Result<(String, Vec<RepairCheck>)> {
        Ok(("C:".into(), (self.0)(cancel, progress)?))
    }
    fn repair(
        &self,
        _: &SystemRepairAction,
        _: &mut dyn FnMut() -> Result<()>,
        _: &mut dyn FnMut(RepairCheck),
    ) -> Result<()> {
        Ok(())
    }
    fn verify(&self, _: &SystemRepairAction, _: &mut dyn FnMut(RepairCheck)) -> Result<()> {
        Ok(())
    }
}

fn done(id: &str, code: &str) -> RepairCheck {
    RepairCheck {
        id: id.into(),
        title: id.into(),
        stage: "Completed".into(),
        result_code: code.into(),
        ..RepairCheck::default()
    }
}

fn coordinator(label: &str, assess: Box<Assess>) -> (RepairCoordinator, std::path::PathBuf) {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "aethercore-p78-{label}-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("temp root");
    let db = Arc::new(Database::open(root.join("state.db")).expect("db"));
    let engine = Arc::new(OperationEngine::new(db.clone()));
    (
        RepairCoordinator::with_platform(engine, db, Arc::new(Scripted(assess))),
        root,
    )
}

fn start(coordinator: &RepairCoordinator) {
    let lease = ReadBudgetManager::new(4)
        .try_acquire(ReadWorkload::RepairAssessment)
        .expect("read budget lease");
    coordinator
        .start_assessment_with_lease(OWNER, lease)
        .expect("start");
}

fn terminal(coordinator: &RepairCoordinator) -> RepairAssessment {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let current = coordinator.assessment_for_owner(OWNER).expect("snapshot");
        if current.state != RepairAssessmentState::Scanning || Instant::now() > deadline {
            return current;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn codes(assessment: &RepairAssessment) -> Vec<&str> {
    assessment
        .checks
        .iter()
        .map(|check| check.result_code.as_str())
        .collect()
}

#[test]
fn a_check_that_never_returns_is_unknown_does_not_pile_up_and_cannot_write_late() {
    static SECOND: ProviderSlot = ProviderSlot::new();
    static STARTS: AtomicUsize = AtomicUsize::new(0);
    // The second check ignores its stop flag for 400 ms, as a Windows API can.
    let (coordinator, root) = coordinator(
        "stuck",
        Box::new(|cancel, progress| {
            let mut checks = vec![done("first", "Fine")];
            progress(AssessStep::Finished(&checks[0]));
            progress(AssessStep::Started("second"));
            checks.push(run_check(
                &SECOND,
                ("second", "Second", "log"),
                Duration::from_millis(100),
                cancel,
                |_| {
                    STARTS.fetch_add(1, Ordering::SeqCst);
                    thread::sleep(Duration::from_millis(400));
                    done("second", "Late")
                },
            )?);
            Ok(checks)
        }),
    );

    // 1. The first check is kept; the second reaches its deadline and reads unknown.
    let began = Instant::now();
    start(&coordinator);
    let first_run = terminal(&coordinator);
    assert_eq!(
        first_run.state,
        RepairAssessmentState::Ready,
        "{first_run:?}"
    );
    assert!(
        began.elapsed() < Duration::from_millis(350),
        "the assessment waited for the stuck check"
    );
    assert_eq!(codes(&first_run), ["Fine", "CheckTimedOut"]);
    assert_eq!(first_run.checks[1].stage, "Unknown");
    assert_eq!(STARTS.load(Ordering::SeqCst), 1);

    // 2. Asked again while the first attempt is still inside its API: no second worker.
    start(&coordinator);
    let pressed = terminal(&coordinator);
    assert_eq!(codes(&pressed), ["Fine", "CheckStillRunning"]);
    assert_eq!(
        STARTS.load(Ordering::SeqCst),
        1,
        "a second worker was started beside the stuck one"
    );

    // 3. The stuck attempt finally returns "Late": nothing of it reaches the newer assessment.
    thread::sleep(Duration::from_millis(600));
    let after = coordinator.assessment_for_owner(OWNER).expect("snapshot");
    assert_eq!(after.assessment_id, pressed.assessment_id);
    assert_eq!(
        codes(&after),
        ["Fine", "CheckStillRunning"],
        "a late result changed a newer assessment"
    );

    // 4. The slot is free again once the thread really returned.
    start(&coordinator);
    assert_eq!(codes(&terminal(&coordinator)), ["Fine", "CheckTimedOut"]);
    assert_eq!(STARTS.load(Ordering::SeqCst), 2);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn the_owners_cancel_stops_a_check_long_before_its_deadline() {
    static WATCHED: ProviderSlot = ProviderSlot::new();
    static SAW_STOP: AtomicBool = AtomicBool::new(false);
    let (coordinator, root) = coordinator(
        "cancel",
        Box::new(|cancel, _| {
            Ok(vec![run_check(
                &WATCHED,
                ("watched", "Watched", "log"),
                Duration::from_secs(30),
                cancel,
                |stop| {
                    let end = Instant::now() + Duration::from_secs(5);
                    while !stop.load(Ordering::SeqCst) && Instant::now() < end {
                        thread::sleep(Duration::from_millis(5));
                    }
                    SAW_STOP.store(stop.load(Ordering::SeqCst), Ordering::SeqCst);
                    done("watched", "Stopped")
                },
            )?])
        }),
    );
    let began = Instant::now();
    start(&coordinator);
    thread::sleep(Duration::from_millis(60));
    coordinator.cancel_assessment(OWNER, "").expect("cancel");
    assert_eq!(
        terminal(&coordinator).state,
        RepairAssessmentState::Cancelled
    );
    assert!(
        began.elapsed() < Duration::from_secs(2),
        "the cancel waited for the 30 s deadline"
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    while !SAW_STOP.load(Ordering::SeqCst) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        SAW_STOP.load(Ordering::SeqCst),
        "the running check was never told to stop"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_check_that_panics_is_unknown_and_frees_its_slot() {
    static PANICKY: ProviderSlot = ProviderSlot::new();
    let never = AtomicBool::new(false);
    let ask = |work: fn() -> RepairCheck| {
        run_check(
            &PANICKY,
            ("boom", "Boom", "log"),
            Duration::from_secs(2),
            &never,
            move |_| work(),
        )
        .expect("check")
    };
    let panicked = ask(|| panic!("the API blew up"));
    assert_eq!(
        (panicked.result_code.as_str(), panicked.stage.as_str()),
        ("ProbeUnavailable", "Unknown")
    );
    assert_eq!(
        ask(|| done("boom", "Fine")).result_code,
        "Fine",
        "the slot stayed taken after a panic"
    );
}
