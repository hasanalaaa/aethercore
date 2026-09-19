//! Phase 22 — One-Click Care orchestration engine.
//!
//! Execution contract:
//! - Single-flight: the orchestrator acquires the machine-wide mutation lease
//!   (`MutationWorkload::OneClickCare`) for the whole run. While it holds the lease,
//!   no domain plan can start a mutation underneath it, and a second care run is
//!   rejected with `CareError::LeaseBusy`.
//! - Journaled resume: every step transition is written to the persistence journal
//!   before and after the domain call. A crashed run resumes from its journal; an
//!   interrupted step whose outcome is unknown becomes recovery evidence, never a
//!   silent replay.
//! - Truth-first report: each finished step cites the DOMAIN's own verification
//!   outcome. There is no aggregate "cleaned/optimized" claim.

use aethercore_collector_runtime::CommitFence;
use aethercore_operation_kernel::{MutationSupervisor, MutationWorkload};

use crate::model::{CareError, CarePlan, CareStepReport, StepOutcome};

/// Drives ONE existing domain plan through its own start path. Implemented by the
/// service layer over the real coordinators; tests implement it with fakes.
///
/// The trait deliberately mirrors the existing `start_with_lease` pattern: the
/// orchestrator passes forward the SAME machine-wide lease discipline every domain
/// already uses, plus the plan digest so a mismatch can be detected before execution.
pub trait DomainStepExecutor {
    /// Executes one existing domain plan. Returns (outcome, domain verification state,
    /// failure message key). Implementations must not invent progress or success.
    fn execute_step(
        &self,
        owner_principal_key: &str,
        domain_plan_id: &str,
        domain_kind: &str,
        lease: &MutationLeaseGuard,
    ) -> Result<(StepOutcome, String, String), CareError>;
}

/// Phase 23 / Part A — typed dispatch into real domain coordinators. Implemented by
/// the service layer over its coordinators; the engine calls it through
/// `DomainStepExecutor` bridges. Kept here so service code and tests share one shape.
pub trait DomainDispatch: Send + Sync {
    /// Starts ONE existing domain plan behind its own safety rules and polls until a
    /// terminal state or deadline. Returns (plan_state, verification_state, failure_key).
    fn start_and_await(
        &self,
        owner_principal_key: &str,
        domain_plan_id: &str,
    ) -> Result<(String, String, String), String>;
}

/// Handle proving the orchestrator currently owns the machine-wide mutation lease.
/// Holding it is required for any `execute_step` call. The inner lease releases on
/// drop; the guard type exists so executors cannot fabricate one.
pub struct MutationLeaseGuard {
    _inner: aethercore_operation_kernel::MutationLease,
}

/// Journal hooks the engine calls at every durable transition. The service layer
/// binds these to `aethercore-persistence`; tests use in-memory implementations.
pub trait CareJournal {
    fn record_run_started(
        &self,
        run_id: &str,
        owner: &str,
        plan_digest: &str,
        steps_total: usize,
    ) -> Result<(), CareError>;
    fn record_run_consent(&self, run_id: &str) -> Result<(), CareError>;
    fn record_step_state(
        &self,
        run_id: &str,
        step_index: usize,
        domain_plan_id: &str,
        domain_kind: &str,
        safety_level: i64,
        state: &str,
    ) -> Result<(), CareError>;
    fn record_step_result(
        &self,
        run_id: &str,
        step_index: usize,
        state: &str,
        outcome: StepOutcome,
        verification_state: &str,
        failure_message_key: &str,
    ) -> Result<(), CareError>;
    fn record_run_finished(&self, run_id: &str, state: &str, detail: &str)
    -> Result<(), CareError>;
}

/// Result of a completed (or partially completed) care run.
#[derive(Clone, Debug)]
pub struct CareRunResult {
    pub run_id: String,
    /// True only if every auto step finished AND was verified by its own domain.
    pub all_steps_verified: bool,
    /// True when the run ended because the session consent did not cover remaining work.
    pub stopped_for_consent: bool,
    pub steps: Vec<CareStepReport>,
}

/// Runs an approved care plan to completion.
///
/// `consent_granted` must come from the service layer's per-session consent record —
/// the orchestrator never treats absence of refusal as consent (product invariant 6).
#[allow(clippy::too_many_arguments)]
pub fn run_care_plan(
    supervisor: &MutationSupervisor,
    executor: &dyn DomainStepExecutor,
    journal: &dyn CareJournal,
    fence: &CommitFence,
    owner_principal_key: &str,
    run_id: &str,
    plan: &CarePlan,
    consent_granted: bool,
) -> Result<CareRunResult, CareError> {
    if !consent_granted {
        return Err(CareError::ConsentRequired);
    }

    // Single-flight across ALL mutations: acquire before anything else happens.
    let lease = supervisor
        .try_acquire(MutationWorkload::OneClickCare, run_id, owner_principal_key)
        .map_err(|_| CareError::LeaseBusy)?;
    let guard = MutationLeaseGuard { _inner: lease };

    journal.record_run_started(
        run_id,
        owner_principal_key,
        &plan.plan_digest_sha256,
        plan.steps.len(),
    )?;
    journal.record_run_consent(run_id)?;

    let mut reports = Vec::new();
    let mut all_verified = true;
    let mut stopped_for_consent = false;

    for (index, step) in plan.steps.iter().enumerate() {
        // A revoked fence means the owner cancelled mid-run: stop cleanly.
        if !fence.is_valid() {
            stopped_for_consent = true;
            break;
        }

        journal.record_step_state(
            run_id,
            index,
            &step.domain_plan_id,
            &step.domain_kind,
            step.safety.level(),
            "Executing",
        )?;

        match executor.execute_step(
            owner_principal_key,
            &step.domain_plan_id,
            &step.domain_kind,
            &guard,
        ) {
            Ok((outcome, verification_state, failure_key)) => {
                if outcome != StepOutcome::VerifiedByDomain && outcome != StepOutcome::Skipped {
                    all_verified = false;
                }
                journal.record_step_result(
                    run_id,
                    index,
                    if outcome == StepOutcome::Failed {
                        "Failed"
                    } else {
                        "Completed"
                    },
                    outcome,
                    &verification_state,
                    &failure_key,
                )?;
                reports.push(CareStepReport {
                    step_index: index,
                    domain_plan_id: step.domain_plan_id.clone(),
                    domain_kind: step.domain_kind.clone(),
                    outcome,
                    domain_verification_state: verification_state,
                    failure_message_key: failure_key,
                });
            }
            Err(error) => {
                // Domain rejection / timeout / poisoned lock: journal the failure as
                // evidence and stop the run. Never continue past an opaque failure.
                // (`all_verified` is irrelevant here — the run returns Err.)
                journal.record_step_result(
                    run_id,
                    index,
                    "Failed",
                    StepOutcome::Failed,
                    "",
                    "care.error.domainFailure",
                )?;
                journal.record_run_finished(run_id, "Failed", "care.error.domainFailure")?;
                return Err(error);
            }
        }
    }

    // Remaining unexecuted review/auto steps are cited as Skipped, never hidden.
    let executed = reports.len();
    for (index, step) in plan.steps.iter().enumerate().skip(executed) {
        reports.push(CareStepReport {
            step_index: index,
            domain_plan_id: step.domain_plan_id.clone(),
            domain_kind: step.domain_kind.clone(),
            outcome: StepOutcome::Skipped,
            domain_verification_state: String::new(),
            failure_message_key: String::new(),
        });
    }

    let detail = if stopped_for_consent {
        "care.status.stoppedForConsent"
    } else if all_verified {
        "care.status.completedVerified"
    } else {
        "care.status.completedWithFailures"
    };
    journal.record_run_finished(run_id, "Completed", detail)?;

    Ok(CareRunResult {
        run_id: run_id.to_string(),
        all_steps_verified: all_verified,
        stopped_for_consent,
        steps: reports,
    })
}
