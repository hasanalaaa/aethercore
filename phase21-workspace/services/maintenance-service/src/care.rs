//! Phase 22 — service-side One-Click Care coordinator.
//!
//! Composes a deterministic plan from the owner's EXISTING eligible plans, enforces
//! the safety firewall, runs the orchestrator single-flight behind
//! `MutationWorkload::OneClickCare`, journals every transition to `care_runs` /
//! `care_steps`, and republishes status through the ordered event bus.
//!
//! Truth-first: the report cites each domain's own verification outcome. The
//! coordinator never derives aggregate claims beyond those citations.

use std::sync::{Arc, Mutex};

use aethercore_care_orchestrator::{
    CareJournal as JournalTrait, CarePlan, CareRunResult, CareSafety, CareStep, DomainStepExecutor,
    StepOutcome,
};
use aethercore_collector_runtime::CommitFence;
use aethercore_contracts::v1;
use aethercore_operation_kernel::MutationSupervisor;
use aethercore_persistence::{CareRunRecord, CareStepRecord, Database};

/// Session consent is in-memory per principal by design: consent that survives a
/// session restart would not be "one-time session consent".
#[derive(Default)]
pub struct SessionConsentRegistry {
    granted: Mutex<std::collections::BTreeSet<String>>,
}

impl SessionConsentRegistry {
    pub fn grant(&self, owner_principal_key: &str) {
        self.granted
            .lock()
            .expect("consent registry")
            .insert(owner_principal_key.to_string());
    }

    pub fn is_granted(&self, owner_principal_key: &str) -> bool {
        self.granted
            .lock()
            .expect("consent registry")
            .contains(owner_principal_key)
    }

    pub fn revoke(&self, owner_principal_key: &str) {
        self.granted
            .lock()
            .expect("consent registry")
            .remove(owner_principal_key);
    }
}

/// Safety classification of an existing domain plan. The mapping mirrors the
/// product contract: cleanup (exact allowlisted evidence) and startup disables
/// (reversible) may auto-run; everything else — driver installs, Windows integrity
/// repair, updates — is review-only and never executed by the orchestrator.
fn classify(domain_kind: &str) -> Option<CareSafety> {
    match domain_kind {
        "Cleanup" | "Startup" => Some(CareSafety::Auto),
        "DriverInstall" | "SystemRepair" => Some(CareSafety::ReviewOnly),
        _ => None,
    }
}

/// Composes the deterministic care plan from the owner's existing plans.
///
/// Only plans in `AwaitingAuthorization`-free terminal-eligible states are
/// considered: a plan must already exist, be digest-bound and owned by this
/// principal. The orchestrator never creates plans.
pub fn compose_plan(db: &Database, owner_principal_key: &str) -> CarePlan {
    // Eligible sources: completed scans leave plans behind only after their own
    // domain created them; we consider plans still awaiting authorization plus
    // recently created ones. Terminal plans are excluded — nothing left to run.
    let candidates = db
        .plans_in_states(&["ReadyForReview", "AwaitingAuthorization"])
        .unwrap_or_default();

    let steps = candidates
        .into_iter()
        .filter(|plan| plan.owner_principal_key == owner_principal_key)
        .filter_map(|plan| {
            classify(plan_kind_of(&plan)).map(|safety| CareStep {
                domain_plan_id: plan.id.clone(),
                domain_kind: plan_kind_of(&plan).to_string(),
                safety,
                title_key: format!("care.step.{}", kind_slug(&plan)),
            })
        })
        .collect();

    CarePlan::build(steps).unwrap_or_else(|| CarePlan {
        steps: Vec::new(),
        plan_digest_sha256: empty_plan_digest(),
    })
}

fn plan_kind_of(plan: &aethercore_persistence::PlanRecord) -> &'static str {
    // PlanRecord carries immutable_json whose actions tag the kind; the cheap stable
    // discriminator is the leading action tag inside the immutable document.
    let json = &plan.immutable_json;
    for (needle, kind) in [
        ("\"kind\":\"installWindowsDriver\"", "DriverInstall"),
        ("\"kind\":\"repairWindowsIntegrity\"", "SystemRepair"),
        ("\"kind\":\"deleteCleanupCandidate\"", "Cleanup"),
        ("\"kind\":\"changeStartupTarget\"", "Startup"),
    ] {
        if json.contains(needle) {
            return kind;
        }
    }
    ""
}

fn kind_slug(plan: &aethercore_persistence::PlanRecord) -> &'static str {
    match plan_kind_of(plan) {
        "DriverInstall" => "driver",
        "SystemRepair" => "repair",
        "Cleanup" => "cleanup",
        "Startup" => "startup",
        _ => "unknown",
    }
}

fn empty_plan_digest() -> String {
    use sha2::{Digest, Sha256};
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = Sha256::digest(b"aethercore.care.empty-plan");
    bytes.iter().fold(String::new(), |mut out, &b| {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
        out
    })
}

// ---------------------------------------------------------------------------
// Persistence-backed journal
// ---------------------------------------------------------------------------

struct PersistenceJournal {
    db: Arc<Database>,
}

impl JournalTrait for PersistenceJournal {
    fn record_run_started(
        &self,
        run_id: &str,
        owner: &str,
        plan_digest: &str,
        steps_total: usize,
    ) -> Result<(), aethercore_care_orchestrator::CareError> {
        let now = chrono_now();
        self.db
            .upsert_care_run(&CareRunRecord {
                run_id: run_id.into(),
                owner_principal_key: owner.into(),
                state: "Running".into(),
                stage: "Executing".into(),
                plan_digest_sha256: plan_digest.into(),
                session_consent_granted: true,
                steps_total: steps_total as u32,
                steps_done: 0,
                detail: String::new(),
                created_unix_ms: now,
                updated_unix_ms: now,
                completed_unix_ms: None,
            })
            .map_err(|error| aethercore_care_orchestrator::CareError::Journal(error.to_string()))
    }

    fn record_run_consent(
        &self,
        run_id: &str,
    ) -> Result<(), aethercore_care_orchestrator::CareError> {
        // Consent was already recorded at grant time via the registry; journal keeps
        // the flag consistent with the run row.
        if let Some(mut run) = self
            .db
            .get_care_run(run_id)
            .map_err(|e| aethercore_care_orchestrator::CareError::Journal(e.to_string()))?
        {
            run.session_consent_granted = true;
            run.updated_unix_ms = chrono_now();
            self.db
                .upsert_care_run(&run)
                .map_err(|e| aethercore_care_orchestrator::CareError::Journal(e.to_string()))?;
        }
        Ok(())
    }

    fn record_step_state(
        &self,
        run_id: &str,
        step_index: usize,
        domain_plan_id: &str,
        domain_kind: &str,
        safety_level: i64,
        state: &str,
    ) -> Result<(), aethercore_care_orchestrator::CareError> {
        let now = chrono_now();
        // Insert-or-update semantics keep crash-resume idempotent.
        let existing = self
            .db
            .care_steps_for_run(run_id)
            .map_err(|e| aethercore_care_orchestrator::CareError::Journal(e.to_string()))?;
        let known = existing
            .iter()
            .any(|step| step.step_index as usize == step_index);
        if !known {
            let mut rows: Vec<CareStepRecord> = existing.clone();
            rows.push(CareStepRecord {
                run_id: run_id.into(),
                step_index: step_index as u32,
                domain_plan_id: domain_plan_id.into(),
                domain_kind: domain_kind.into(),
                safety_level,
                state: state.into(),
                outcome: String::new(),
                verification_state: String::new(),
                failure_message: String::new(),
                started_unix_ms: now,
                updated_unix_ms: now,
            });
            self.db
                .replace_care_steps(&rows)
                .map_err(|e| aethercore_care_orchestrator::CareError::Journal(e.to_string()))?;
        } else {
            self.db
                .update_care_step(&CareStepRecord {
                    run_id: run_id.into(),
                    step_index: step_index as u32,
                    state: state.into(),
                    updated_unix_ms: now,
                    ..CareStepRecord::default()
                })
                .map_err(|e| aethercore_care_orchestrator::CareError::Journal(e.to_string()))?;
        }

        if let Some(mut run) = self
            .db
            .get_care_run(run_id)
            .map_err(|e| aethercore_care_orchestrator::CareError::Journal(e.to_string()))?
        {
            run.stage = format!("{domain_kind}:{state}");
            run.steps_done = existing
                .iter()
                .filter(|step| {
                    !step.state.is_empty() && step.state != "Pending" && step.state != "Executing"
                })
                .count()
                .min(u32::MAX as usize) as u32;
            run.updated_unix_ms = now;
            self.db
                .upsert_care_run(&run)
                .map_err(|e| aethercore_care_orchestrator::CareError::Journal(e.to_string()))?;
        }
        Ok(())
    }

    fn record_step_result(
        &self,
        run_id: &str,
        step_index: usize,
        state: &str,
        outcome: StepOutcome,
        verification_state: &str,
        failure_message_key: &str,
    ) -> Result<(), aethercore_care_orchestrator::CareError> {
        let outcome_str = match outcome {
            StepOutcome::VerifiedByDomain => "VerifiedByDomain",
            StepOutcome::CompletedUnverified => "CompletedUnverified",
            StepOutcome::Failed => "Failed",
            StepOutcome::Skipped => "Skipped",
        };
        let existing = self
            .db
            .care_steps_for_run(run_id)
            .map_err(|e| aethercore_care_orchestrator::CareError::Journal(e.to_string()))?;
        let mut step = existing
            .into_iter()
            .find(|step| step.step_index as usize == step_index)
            .unwrap_or_default();
        step.state = state.into();
        step.outcome = outcome_str.into();
        step.verification_state = verification_state.into();
        step.failure_message = failure_message_key.into();
        step.updated_unix_ms = chrono_now();
        self.db
            .update_care_step(&step)
            .map_err(|e| aethercore_care_orchestrator::CareError::Journal(e.to_string()))
    }

    fn record_run_finished(
        &self,
        run_id: &str,
        state: &str,
        detail: &str,
    ) -> Result<(), aethercore_care_orchestrator::CareError> {
        if let Some(mut run) = self
            .db
            .get_care_run(run_id)
            .map_err(|e| aethercore_care_orchestrator::CareError::Journal(e.to_string()))?
        {
            run.state = state.into();
            run.detail = detail.into();
            run.updated_unix_ms = chrono_now();
            run.completed_unix_ms = Some(chrono_now());
            self.db
                .upsert_care_run(&run)
                .map_err(|e| aethercore_care_orchestrator::CareError::Journal(e.to_string()))?;
        }
        Ok(())
    }
}

fn chrono_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

// ---------------------------------------------------------------------------
// Coordinator
// ---------------------------------------------------------------------------

/// Re-exported dispatch trait (owned by the care-orchestrator crate so service code
/// and tests share one shape). Implementations live in composition at startup.
pub use aethercore_care_orchestrator::DomainDispatch;

/// Poll cadence and ceiling for awaiting a dispatched domain plan.
pub(crate) const DISPATCH_POLL_MS: u64 = 100;
pub(crate) const DISPATCH_TIMEOUT_MS: u64 = 120_000;

/// Terminal plan states per the operation engine state machine.
pub(crate) fn is_terminal_plan_state(state: &str) -> bool {
    matches!(state, "Completed" | "Failed" | "RebootPending")
}

/// Real executor: forwards to the composition-provided [`DomainDispatch`].
struct ServiceExecutor {
    dispatch: std::sync::Arc<dyn aethercore_care_orchestrator::DomainDispatch>,
}

impl DomainStepExecutor for ServiceExecutor {
    fn execute_step(
        &self,
        owner: &str,
        domain_plan_id: &str,
        _domain_kind: &str,
        _lease: &aethercore_care_orchestrator::MutationLeaseGuard,
    ) -> Result<(StepOutcome, String, String), aethercore_care_orchestrator::CareError> {
        let (plan_state, verification_state, failure_key) = self
            .dispatch
            .start_and_await(owner, domain_plan_id)
            .map_err(
                |detail| aethercore_care_orchestrator::CareError::DomainRejected {
                    domain_kind: _domain_kind.to_string(),
                    detail,
                },
            )?;
        if is_terminal_plan_state(&plan_state) && plan_state != "Completed" {
            return Ok((StepOutcome::Failed, verification_state, failure_key));
        }
        if plan_state == "Completed" {
            if !verification_state.is_empty() {
                return Ok((
                    StepOutcome::VerifiedByDomain,
                    verification_state,
                    String::new(),
                ));
            }
            return Ok((
                StepOutcome::CompletedUnverified,
                String::new(),
                String::new(),
            ));
        }
        // Non-terminal after deadline (RebootPending counts as terminal-but-unverified).
        Err(aethercore_care_orchestrator::CareError::DomainRejected {
            domain_kind: _domain_kind.to_string(),
            detail: format!("plan {domain_plan_id} did not reach a terminal state in time"),
        })
    }
}

pub struct CareCoordinator {
    db: Arc<Database>,
    supervisor: MutationSupervisor,
    fence: CommitFence,
    pub consent: Arc<SessionConsentRegistry>,
    /// Composition-provided dispatch into the real domain coordinators (Part A).
    dispatch: std::sync::Arc<dyn aethercore_care_orchestrator::DomainDispatch>,
}

impl CareCoordinator {
    pub fn new(
        db: Arc<Database>,
        dispatch: std::sync::Arc<dyn aethercore_care_orchestrator::DomainDispatch>,
    ) -> Self {
        Self {
            db,
            supervisor: MutationSupervisor::new(),
            fence: CommitFence::new(),
            consent: Arc::new(SessionConsentRegistry::default()),
            dispatch,
        }
    }

    /// Deterministic preview of what a care run would do right now.
    pub fn plan_preview(&self, owner_principal_key: &str) -> v1::CareRunStatus {
        let plan = compose_plan(&self.db, owner_principal_key);
        status_proto(
            "Idle",
            "Preview",
            self.consent.is_granted(owner_principal_key),
            &plan,
            &[],
        )
    }

    /// Grants one-time session consent for auto-level work.
    pub fn grant_session_consent(&self, owner_principal_key: &str) {
        self.consent.grant(owner_principal_key);
    }

    /// Cancels an active run at the next step boundary.
    pub fn cancel(&self) {
        self.fence.revoke();
    }

    /// Runs the composed plan. Requires prior session consent.
    pub fn start_run(
        &self,
        owner_principal_key: &str,
        run_id: &str,
    ) -> Result<v1::CareRunStatus, aethercore_care_orchestrator::CareError> {
        let plan = compose_plan(&self.db, owner_principal_key);
        let journal = PersistenceJournal {
            db: self.db.clone(),
        };
        let executor = ServiceExecutor {
            dispatch: self.dispatch.clone(),
        };
        let result: Result<CareRunResult, aethercore_care_orchestrator::CareError> =
            aethercore_care_orchestrator::run_care_plan(
                &self.supervisor,
                &executor,
                &journal,
                &self.fence,
                owner_principal_key,
                run_id,
                &plan,
                self.consent.is_granted(owner_principal_key),
            );
        match result {
            Ok(report) => Ok(status_proto(
                "Completed",
                "Report",
                true,
                &plan,
                &report.steps,
            )),
            Err(error) => {
                // Consent refusal before anything ran: surface as awaiting-consent state.
                if error == aethercore_care_orchestrator::CareError::ConsentRequired {
                    return Ok(status_proto(
                        "AwaitingConsent",
                        "Consent",
                        false,
                        &plan,
                        &[],
                    ));
                }
                let _ = journal.record_run_finished(run_id, "Failed", "care.error.domainFailure");
                Err(error)
            }
        }
    }
}

/// Builds wire status from plan + step reports.
pub(crate) fn status_proto(
    state: &str,
    stage: &str,
    consent_granted: bool,
    plan: &CarePlan,
    reports: &[aethercore_care_orchestrator::CareStepReport],
) -> v1::CareRunStatus {
    let mut steps: Vec<v1::CareStepReport> = Vec::new();
    for (index, step) in plan.steps.iter().enumerate() {
        let report = reports.iter().find(|r| r.step_index == index);
        steps.push(v1::CareStepReport {
            step_index: index as u32,
            domain_plan_id: step.domain_plan_id.clone(),
            domain_kind: step.domain_kind.clone(),
            safety_level: step.safety.level(),
            state: report
                .map(|r| match r.outcome {
                    StepOutcome::Failed => "Failed",
                    StepOutcome::Skipped => "Skipped",
                    _ => "Completed",
                })
                .unwrap_or(if matches!(state, "Completed" | "Failed") {
                    "Skipped"
                } else {
                    "Pending"
                })
                .to_string(),
            outcome: report
                .map(|r| match r.outcome {
                    StepOutcome::VerifiedByDomain => "VerifiedByDomain",
                    StepOutcome::CompletedUnverified => "CompletedUnverified",
                    StepOutcome::Failed => "Failed",
                    StepOutcome::Skipped => "Skipped",
                })
                .unwrap_or("Pending")
                .to_string(),
            domain_verification_state: report
                .map(|r| r.domain_verification_state.clone())
                .unwrap_or_default(),
            failure_message_key: report
                .map(|r| r.failure_message_key.clone())
                .unwrap_or_default(),
        });
    }
    let summary_key = if !consent_granted && state == "Idle" {
        "care.summary.needsConsent"
    } else {
        match state {
            "Failed" => "care.summary.failed",
            "Cancelled" => "care.summary.cancelled",
            "AwaitingConsent" => "care.summary.needsConsent",
            _ => "care.summary.completed",
        }
    };
    v1::CareRunStatus {
        run_id: String::new(),
        state: state.to_string(),
        stage: stage.to_string(),
        session_consent_granted: consent_granted,
        plan_digest_sha256: plan.plan_digest_sha256.clone(),
        steps,
        updated_unix_ms: chrono_now(),
        summary_key: summary_key.to_string(),
    }
}
