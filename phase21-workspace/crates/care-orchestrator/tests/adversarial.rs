//! Phase 22 — adversarial tests for the One-Click Care orchestrator.
//!
//! Attacked here: missing consent, concurrent start attempts (single-flight),
//! hostile domain responses (fail / timeout / poisoned lock), plan-digest
//! determinism under reordering/duplication, and journal completeness.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use aethercore_care_orchestrator::{
    CareError, CareJournal, CarePlan, CareSafety, CareStep, StepOutcome, run_care_plan,
};
use aethercore_collector_runtime::CommitFence;
use aethercore_operation_kernel::{MutationSupervisor, MutationWorkload};

const OWNER: &str = "care-owner";

fn step(id: &str, kind: &str, safety: CareSafety) -> CareStep {
    CareStep {
        domain_plan_id: id.to_string(),
        domain_kind: kind.to_string(),
        safety,
        title_key: "care.step.title".to_string(),
    }
}

/// In-memory journal recording every durable transition in order.
#[derive(Default)]
struct MemoryJournal {
    events: Mutex<Vec<String>>,
    poisoned: AtomicBool,
}

impl MemoryJournal {
    fn poison(&self) {
        self.poisoned.store(true, Ordering::SeqCst);
    }
    fn log(&self, event: String) -> Result<(), CareError> {
        if self.poisoned.load(Ordering::SeqCst) {
            return Err(CareError::Journal("poisoned lock".into()));
        }
        self.events.lock().expect("events").push(event);
        Ok(())
    }
}

impl CareJournal for MemoryJournal {
    fn record_run_started(
        &self,
        run_id: &str,
        owner: &str,
        plan_digest: &str,
        steps_total: usize,
    ) -> Result<(), CareError> {
        self.log(format!(
            "started:{run_id}:{owner}:{plan_digest}:{steps_total}"
        ))
    }
    fn record_run_consent(&self, run_id: &str) -> Result<(), CareError> {
        self.log(format!("consent:{run_id}"))
    }
    fn record_step_state(
        &self,
        run_id: &str,
        step_index: usize,
        domain_plan_id: &str,
        _domain_kind: &str,
        _safety_level: i64,
        state: &str,
    ) -> Result<(), CareError> {
        self.log(format!(
            "step:{run_id}:{step_index}:{domain_plan_id}:{state}"
        ))
    }
    fn record_step_result(
        &self,
        run_id: &str,
        step_index: usize,
        state: &str,
        outcome: StepOutcome,
        verification_state: &str,
        failure_key: &str,
    ) -> Result<(), CareError> {
        self.log(format!(
            "result:{run_id}:{step_index}:{state}:{outcome:?}:{verification_state}:{failure_key}"
        ))
    }
    fn record_run_finished(
        &self,
        run_id: &str,
        state: &str,
        detail: &str,
    ) -> Result<(), CareError> {
        self.log(format!("finished:{run_id}:{state}:{detail}"))
    }
}

/// One scripted executor call: the outcome it returns, or the domain rejection it fails with.
type ScriptedCall = Result<(StepOutcome, String, String), CareError>;

/// Scriptable fake executor.
struct FakeExecutor {
    /// Per-call outcomes in order; None means the call fails with a domain rejection.
    script: Mutex<Vec<ScriptedCall>>,
    calls: AtomicU32,
}

impl FakeExecutor {
    fn always_verified() -> Self {
        Self {
            script: Mutex::new(Vec::new()),
            calls: AtomicU32::new(0),
        }
    }

    fn scripted(outcomes: Vec<ScriptedCall>) -> Self {
        Self {
            script: Mutex::new(outcomes),
            calls: AtomicU32::new(0),
        }
    }
}

impl aethercore_care_orchestrator::DomainStepExecutor for FakeExecutor {
    fn execute_step(
        &self,
        _owner: &str,
        domain_plan_id: &str,
        domain_kind: &str,
        _lease: &aethercore_care_orchestrator::MutationLeaseGuard,
    ) -> Result<(StepOutcome, String, String), CareError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut script = self.script.lock().expect("script");
        let outcome = if script.is_empty() {
            Ok((
                StepOutcome::VerifiedByDomain,
                "Verified".to_string(),
                String::new(),
            ))
        } else {
            script.remove(0)
        };
        match outcome {
            Ok(value) => Ok(value),
            Err(CareError::DomainRejected { .. }) => Err(CareError::DomainRejected {
                domain_kind: domain_kind.to_string(),
                detail: format!("plan {domain_plan_id} rejected"),
            }),
            Err(other) => Err(other),
        }
    }
}

/// The consent an owner gives after being shown `plan`.
fn approved(plan: &CarePlan) -> Option<&str> {
    Some(&plan.plan_digest_sha256)
}

fn two_step_plan() -> CarePlan {
    CarePlan::build(vec![
        step("cleanup-plan-1", "Cleanup", CareSafety::Auto),
        step("startup-plan-1", "Startup", CareSafety::Auto),
    ])
    .expect("plan")
}

// ---------------------------------------------------------------------------
// Plan builder determinism + firewall classification
// ---------------------------------------------------------------------------

#[test]
fn plan_digest_is_deterministic_under_reordering_and_duplicates() {
    let a = CarePlan::build(vec![
        step("p1", "Cleanup", CareSafety::Auto),
        step("p2", "Startup", CareSafety::Auto),
    ])
    .unwrap();
    let b = CarePlan::build(vec![
        step("p2", "Startup", CareSafety::Auto),
        step("p1", "Cleanup", CareSafety::Auto),
        step("p1", "Cleanup", CareSafety::Auto), // duplicate collapses
    ])
    .unwrap();
    assert_eq!(a.plan_digest_sha256, b.plan_digest_sha256);
    assert_eq!(a.steps.len(), 2);

    // Different content must change the digest.
    let c = CarePlan::build(vec![
        step("p1", "Cleanup", CareSafety::Auto),
        step("p3", "Startup", CareSafety::Auto),
    ])
    .unwrap();
    assert_ne!(a.plan_digest_sha256, c.plan_digest_sha256);
}

#[test]
fn review_only_items_never_classify_as_auto() {
    let plan = CarePlan::build(vec![
        step("drv-plan-1", "DriverInstall", CareSafety::ReviewOnly),
        step("clean-plan-1", "Cleanup", CareSafety::Auto),
    ])
    .unwrap();
    let auto: Vec<_> = plan.auto_steps().collect();
    let review: Vec<_> = plan.review_steps().collect();
    assert_eq!(auto.len(), 1);
    assert_eq!(auto[0].domain_plan_id, "clean-plan-1");
    assert_eq!(review.len(), 1);
    assert_eq!(review[0].domain_plan_id, "drv-plan-1");
    assert!(
        review[0].safety.level() >= 2,
        "review work sits at level >= 2"
    );
}

#[test]
fn empty_plan_is_rejected_and_cap_is_enforced() {
    assert!(CarePlan::build(Vec::new()).is_none());
    let overflow: Vec<CareStep> = (0..=32)
        .map(|index| step(&format!("plan-{index}"), "Cleanup", CareSafety::Auto))
        .collect();
    assert!(CarePlan::build(overflow).is_none());
    assert!(CarePlan::build(vec![step("", "Cleanup", CareSafety::Auto)]).is_none());
}

// ---------------------------------------------------------------------------
// Execution semantics
// ---------------------------------------------------------------------------

#[test]
fn refuses_to_run_without_explicit_session_consent() {
    let supervisor = MutationSupervisor::new();
    let executor = FakeExecutor::always_verified();
    let journal = MemoryJournal::default();
    let error = run_care_plan(
        &supervisor,
        &executor,
        &journal,
        &CommitFence::new(),
        OWNER,
        "run-1",
        &two_step_plan(),
        None, // no consent — absence of refusal is NOT consent
    )
    .expect_err("must refuse");
    assert_eq!(error, CareError::ConsentRequired);
    assert_eq!(executor.calls.load(Ordering::SeqCst), 0, "nothing executed");
    assert!(
        journal.events.lock().unwrap().is_empty(),
        "nothing journaled"
    );
}

#[test]
fn second_concurrent_start_is_rejected_by_single_flight() {
    let supervisor = MutationSupervisor::new();
    // Someone else holds ANY mutation lease → machine busy.
    let foreign = supervisor
        .try_acquire(MutationWorkload::Cleanup, "cleanup-plan-9", OWNER)
        .expect("foreign lease");
    let executor = FakeExecutor::always_verified();
    let journal = MemoryJournal::default();
    let error = run_care_plan(
        &supervisor,
        &executor,
        &journal,
        &CommitFence::new(),
        OWNER,
        "run-2",
        &two_step_plan(),
        approved(&two_step_plan()),
    )
    .expect_err("machine is busy");
    assert_eq!(error, CareError::LeaseBusy);
    assert_eq!(executor.calls.load(Ordering::SeqCst), 0);
    drop(foreign);

    // With the lease free, the same run succeeds — proving the rejection was flight-safety.
    let result = run_care_plan(
        &supervisor,
        &executor,
        &journal,
        &CommitFence::new(),
        OWNER,
        "run-3",
        &two_step_plan(),
        approved(&two_step_plan()),
    )
    .expect("runs after lease released");
    assert!(result.all_steps_verified);
}

#[test]
fn hostile_domain_failure_stops_run_and_journals_evidence() {
    let supervisor = MutationSupervisor::new();
    let executor = FakeExecutor::scripted(vec![
        Ok((
            StepOutcome::VerifiedByDomain,
            "Verified".into(),
            String::new(),
        )),
        Err(CareError::DomainRejected {
            domain_kind: "Startup".into(),
            detail: "timeout".into(),
        }),
    ]);
    let journal = MemoryJournal::default();
    let error = run_care_plan(
        &supervisor,
        &executor,
        &journal,
        &CommitFence::new(),
        OWNER,
        "run-4",
        &two_step_plan(),
        approved(&two_step_plan()),
    )
    .expect_err("hostile domain stops the run");
    assert!(matches!(error, CareError::DomainRejected { .. }));
    let events = journal.events.lock().unwrap();
    assert!(
        events.iter().any(|event| event.contains(":Failed")),
        "failed step journaled as evidence"
    );
    assert!(
        events
            .iter()
            .any(|event| event.starts_with("finished:run-4:Failed")),
        "run closed as Failed before returning"
    );
    // Exactly two execute attempts happened; the run did not continue past the failure.
    assert_eq!(executor.calls.load(Ordering::SeqCst), 2);
}

#[test]
fn unverified_domain_outcome_never_counts_as_verified() {
    let supervisor = MutationSupervisor::new();
    let executor = FakeExecutor::scripted(vec![
        Ok((
            StepOutcome::VerifiedByDomain,
            "Verified".into(),
            String::new(),
        )),
        Ok((
            StepOutcome::CompletedUnverified,
            String::new(),
            String::new(),
        )),
    ]);
    let journal = MemoryJournal::default();
    let result = run_care_plan(
        &supervisor,
        &executor,
        &journal,
        &CommitFence::new(),
        OWNER,
        "run-5",
        &two_step_plan(),
        approved(&two_step_plan()),
    )
    .expect("run completes");
    assert!(
        !result.all_steps_verified,
        "no aggregate verified claim without per-domain verification"
    );
    assert_eq!(result.steps[1].outcome, StepOutcome::CompletedUnverified);
}

#[test]
fn revoked_fence_stops_remaining_steps_and_marks_them_skipped() {
    let supervisor = MutationSupervisor::new();
    let fence = CommitFence::new();
    fence.revoke(); // owner cancelled before anything ran
    let executor = FakeExecutor::always_verified();
    let journal = MemoryJournal::default();
    let result = run_care_plan(
        &supervisor,
        &executor,
        &journal,
        &fence,
        OWNER,
        "run-6",
        &two_step_plan(),
        approved(&two_step_plan()),
    )
    .expect("cancelled run still returns a report");
    assert!(result.stopped_for_consent);
    assert_eq!(executor.calls.load(Ordering::SeqCst), 0);
    assert_eq!(result.steps.len(), 2);
    assert!(
        result
            .steps
            .iter()
            .all(|step| step.outcome == StepOutcome::Skipped)
    );
}

#[test]
fn poisoned_journal_surfaces_as_typed_error_not_silent_progress() {
    let supervisor = MutationSupervisor::new();
    let executor = FakeExecutor::always_verified();
    let journal = MemoryJournal::default();
    journal.poison();
    let error = run_care_plan(
        &supervisor,
        &executor,
        &journal,
        &CommitFence::new(),
        OWNER,
        "run-7",
        &two_step_plan(),
        approved(&two_step_plan()),
    )
    .expect_err("journal poisoning must surface");
    assert!(matches!(error, CareError::Journal(_)));
    assert_eq!(executor.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn journal_records_every_transition_in_order_for_a_clean_run() {
    let supervisor = MutationSupervisor::new();
    let executor = FakeExecutor::always_verified();
    let journal = MemoryJournal::default();
    let plan = two_step_plan();
    let result = run_care_plan(
        &supervisor,
        &executor,
        &journal,
        &CommitFence::new(),
        OWNER,
        "run-8",
        &plan,
        approved(&plan),
    )
    .expect("clean run");
    assert!(result.all_steps_verified);
    let events = journal.events.lock().unwrap().clone();
    let digest = &plan.plan_digest_sha256;
    assert_eq!(events[0], format!("started:run-8:{OWNER}:{digest}:2"));
    assert_eq!(events[1], "consent:run-8");
    assert_eq!(events[2], "step:run-8:0:cleanup-plan-1:Executing");
    assert!(events[3].starts_with("result:run-8:0:Completed:VerifiedByDomain:Verified"));
    assert_eq!(events[4], "step:run-8:1:startup-plan-1:Executing");
    assert!(events[5].starts_with("result:run-8:1:Completed:VerifiedByDomain:Verified"));
    assert!(events[6].starts_with("finished:run-8:Completed:care.status.completedVerified"));
}

// ---------------------------------------------------------------------------
// P75 care-consent: review work is listed, never run, and never runs first
// ---------------------------------------------------------------------------

fn mixed_plan() -> CarePlan {
    CarePlan::build(vec![
        step("drv-plan-1", "DriverInstall", CareSafety::ReviewOnly),
        step("clean-plan-1", "Cleanup", CareSafety::Auto),
    ])
    .expect("plan")
}

#[test]
fn auto_work_is_ordered_before_review_work() {
    let plan = mixed_plan();
    assert_eq!(plan.steps[0].domain_plan_id, "clean-plan-1");
    assert_eq!(plan.steps[1].domain_plan_id, "drv-plan-1");
}

#[test]
fn review_only_steps_are_never_executed() {
    let supervisor = MutationSupervisor::new();
    let executor = FakeExecutor::always_verified();
    let journal = MemoryJournal::default();
    let plan = mixed_plan();
    let result = run_care_plan(
        &supervisor,
        &executor,
        &journal,
        &CommitFence::new(),
        OWNER,
        "run-review",
        &plan,
        approved(&plan),
    )
    .expect("the auto step runs");
    assert_eq!(
        executor.calls.load(Ordering::SeqCst),
        1,
        "only the auto step reaches a domain"
    );
    let review = result
        .steps
        .iter()
        .find(|report| report.domain_plan_id == "drv-plan-1")
        .expect("the review step is still reported");
    assert_eq!(review.outcome, StepOutcome::Skipped);
    assert!(
        !journal
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|event| event.contains("drv-plan-1")),
        "a step that never ran is not journaled as executing"
    );
}

#[test]
fn consent_given_for_another_plan_is_refused() {
    let supervisor = MutationSupervisor::new();
    let executor = FakeExecutor::always_verified();
    let journal = MemoryJournal::default();
    let error = run_care_plan(
        &supervisor,
        &executor,
        &journal,
        &CommitFence::new(),
        OWNER,
        "run-stale",
        &two_step_plan(),
        approved(&mixed_plan()),
    )
    .expect_err("the owner never saw this plan");
    assert_eq!(error, CareError::DigestChanged);
    assert_eq!(executor.calls.load(Ordering::SeqCst), 0);
    assert!(journal.events.lock().unwrap().is_empty());
}
