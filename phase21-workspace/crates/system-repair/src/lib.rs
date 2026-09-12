#![deny(unsafe_op_in_unsafe_fn)]

use std::{
    sync::{Arc, Mutex, RwLock},
    thread,
};

use aethercore_operation_engine::{
    OperationEngine, PlanState, PlanView, SystemRepairAction, now_ms,
};
use aethercore_operation_kernel::{
    MutationLease, MutationWorkload, ProgressTelemetry, ProgressTelemetryStore, ReadBudgetLease,
    ReadWorkload,
};
use aethercore_persistence::{
    Database, MaintenanceExecutionRecord, MaintenanceItemRecord, RecoveryRecord,
    RepairRebootResumeRecord, RepairTimelineEventRecord,
};
use aethercore_windows_repair_intelligence::{
    DiagnosisConfidence, FactState, RecoveryReadiness, RepairActionKind, RepairDomain, RepairFact,
    RepairIntelligenceSnapshot, RepairObservationSet, RepairOutcome, RepairSafetyTier,
    analyze as analyze_windows_repair, canonical_machine_state_fingerprint, reboot_resume_token,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum RepairError {
    #[error("system repair is only available on Windows")]
    UnsupportedPlatform,
    #[error("repair assessment is already running")]
    Busy,
    #[error("repair assessment is not ready")]
    AssessmentNotReady,
    #[error("repair assessment is stale")]
    StaleAssessment,
    #[error("repair graph is invalid: {0}")]
    InvalidRepairGraph(String),
    #[error("no evidence-backed executable repair is currently recommended")]
    NoRepairRecommended,
    #[error("repair verification did not prove the intended state")]
    VerificationFailed,
    #[error("repair plan requires a reboot boundary before further mutation")]
    RebootBoundary,
    #[error("Windows repair source is required: {0}")]
    SourceRequired(String),
    #[error("recovery escalation is required or unavailable for this automatic path: {0}")]
    RecoveryUnavailable(String),
    #[error("repair assessment belongs to a different Windows principal")]
    OwnershipMismatch,
    #[error("repair plan is not authorized")]
    AuthorizationRequired,
    #[error("repair plan is already running")]
    AlreadyRunning,
    #[error("Windows servicing pipeline is busy")]
    ServicingBusy,
    #[error("Windows requires a restart before servicing can safely continue")]
    RebootPending,
    #[error("repair command failed: {0}")]
    Command(String),
    #[error("operation engine: {0}")]
    Engine(#[from] aethercore_operation_engine::EngineError),
    #[error("persistence: {0}")]
    Persistence(#[from] aethercore_persistence::PersistenceError),
}

pub type Result<T> = std::result::Result<T, RepairError>;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RepairAssessmentState {
    Idle,
    Scanning,
    Ready,
    Failed,
}

impl RepairAssessmentState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Scanning => "Scanning",
            Self::Ready => "Ready",
            Self::Failed => "Failed",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepairCheck {
    pub id: String,
    pub title: String,
    pub stage: String,
    pub result_code: String,
    pub exit_code: i32,
    pub detail: String,
    pub log_hint: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairAssessment {
    pub assessment_id: String,
    pub state: RepairAssessmentState,
    pub started_unix_ms: i64,
    pub completed_unix_ms: i64,
    pub error_message: String,
    pub system_volume: String,
    pub checks: Vec<RepairCheck>,
    /// Phase 19 typed diagnosis/repair graph; absent only while idle/scanning/failed.
    pub intelligence: Option<RepairIntelligenceSnapshot>,
}

impl Default for RepairAssessment {
    fn default() -> Self {
        Self {
            assessment_id: String::new(),
            state: RepairAssessmentState::Idle,
            started_unix_ms: 0,
            completed_unix_ms: 0,
            error_message: String::new(),
            system_volume: String::new(),
            checks: Vec::new(),
            intelligence: None,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairExecutionStatus {
    pub plan_id: String,
    pub plan_state: String,
    pub stage: String,
    pub progress_known: bool,
    pub overall_percent: u32,
    pub current_step_id: String,
    pub detail: String,
    pub mutation_started: bool,
    pub recovery_required: bool,
    pub failure_message: String,
    pub outcome: String,
    pub repair_graph_digest: String,
    pub machine_state_fingerprint: String,
    pub safety_tier: String,
    pub reboot_required: bool,
    pub verification_state: String,
    pub started_unix_ms: i64,
    pub updated_unix_ms: i64,
    /// DBT-P46-B7: None while the plan has not completed. Previously flattened
    /// to 0 here, which the wire could not tell apart from a plan that
    /// completed at unix epoch 0.
    pub completed_unix_ms: Option<i64>,
    pub steps: Vec<RepairCheck>,
}

pub trait RepairPlatform: Send + Sync + 'static {
    fn assess(&self) -> Result<(String, Vec<RepairCheck>)>;
    fn repair(
        &self,
        action: &SystemRepairAction,
        begin_mutation: &mut dyn FnMut() -> Result<()>,
        emit: &mut dyn FnMut(RepairCheck),
    ) -> Result<()>;
    fn verify(&self, action: &SystemRepairAction, emit: &mut dyn FnMut(RepairCheck)) -> Result<()>;
}

/// Takes one output-reader thread's join result, recording a note instead of an
/// empty string when the thread panicked.
///
/// DBT-P46-B9: `join().unwrap_or_default()` reported a reader thread that
/// *crashed* as a command that printed nothing. The process exit code is still
/// a real observation, so the check itself stands — but its detail must say the
/// stream was lost rather than let silence imply the command was silent.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn joined_stream(
    joined: std::thread::Result<String>,
    stream: &str,
    notes: &mut Vec<String>,
) -> String {
    match joined {
        Ok(text) => text,
        Err(_) => {
            notes.push(format!(
                "{stream} reader thread panicked; that stream was not captured for this check"
            ));
            String::new()
        }
    }
}

/// Trims `detail` to at most `limit` bytes, keeping the tail, without splitting
/// a character.
///
/// Adjacent to DBT-P46-B9 and found in the same four lines: the tail used to be
/// taken at a fixed byte offset. DISM and SFC output is localized, so that
/// offset can land mid-character — and `String` indexing panics there.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn trim_to_tail(detail: &mut String, limit: usize) {
    if detail.len() <= limit {
        return;
    }
    let mut start = detail.len() - limit;
    while !detail.is_char_boundary(start) {
        start += 1;
    }
    detail.drain(..start);
}

#[cfg(windows)]
mod dism_api;
#[cfg(windows)]
mod windows_impl;
#[cfg(windows)]
use windows_impl::WindowsRepairPlatform;

#[cfg(not(windows))]
struct WindowsRepairPlatform;

#[cfg(not(windows))]
impl RepairPlatform for WindowsRepairPlatform {
    fn assess(&self) -> Result<(String, Vec<RepairCheck>)> {
        Err(RepairError::UnsupportedPlatform)
    }

    fn repair(
        &self,
        _action: &SystemRepairAction,
        _begin_mutation: &mut dyn FnMut() -> Result<()>,
        _emit: &mut dyn FnMut(RepairCheck),
    ) -> Result<()> {
        Err(RepairError::UnsupportedPlatform)
    }

    fn verify(
        &self,
        _action: &SystemRepairAction,
        _emit: &mut dyn FnMut(RepairCheck),
    ) -> Result<()> {
        Err(RepairError::UnsupportedPlatform)
    }
}

pub struct RepairCoordinator {
    engine: Arc<OperationEngine>,
    db: Arc<Database>,
    platform: Arc<dyn RepairPlatform>,
    assessment: Arc<RwLock<RepairAssessment>>,
    assessment_owner: Arc<RwLock<String>>,
    running: Arc<Mutex<Option<String>>>,
    telemetry: ProgressTelemetryStore,
}

impl RepairCoordinator {
    pub fn new(engine: Arc<OperationEngine>, db: Arc<Database>) -> Self {
        Self::with_platform_and_telemetry(
            engine,
            db,
            Arc::new(WindowsRepairPlatform),
            ProgressTelemetryStore::new(),
        )
    }

    pub fn with_platform(
        engine: Arc<OperationEngine>,
        db: Arc<Database>,
        platform: Arc<dyn RepairPlatform>,
    ) -> Self {
        Self::with_platform_and_telemetry(engine, db, platform, ProgressTelemetryStore::new())
    }

    pub fn with_telemetry(
        engine: Arc<OperationEngine>,
        db: Arc<Database>,
        telemetry: ProgressTelemetryStore,
    ) -> Self {
        Self::with_platform_and_telemetry(engine, db, Arc::new(WindowsRepairPlatform), telemetry)
    }

    pub fn with_platform_and_telemetry(
        engine: Arc<OperationEngine>,
        db: Arc<Database>,
        platform: Arc<dyn RepairPlatform>,
        telemetry: ProgressTelemetryStore,
    ) -> Self {
        Self {
            engine,
            db,
            platform,
            assessment: Arc::new(RwLock::new(RepairAssessment::default())),
            assessment_owner: Arc::new(RwLock::new(String::new())),
            running: Arc::new(Mutex::new(None)),
            telemetry,
        }
    }

    pub fn start_assessment_with_lease(
        &self,
        owner_principal_key: &str,
        lease: ReadBudgetLease,
    ) -> Result<RepairAssessment> {
        if !lease.matches(ReadWorkload::RepairAssessment) {
            return Err(RepairError::Command(
                "read budget lease identity mismatch".into(),
            ));
        }
        self.start_assessment_inner(owner_principal_key, lease)
    }

    fn start_assessment_inner(
        &self,
        owner_principal_key: &str,
        read_budget_lease: ReadBudgetLease,
    ) -> Result<RepairAssessment> {
        let assessment_id = Uuid::new_v4().to_string();
        let started = now_ms();
        let initial = RepairAssessment {
            assessment_id: assessment_id.clone(),
            state: RepairAssessmentState::Scanning,
            started_unix_ms: started,
            ..RepairAssessment::default()
        };
        {
            let mut owner = self
                .assessment_owner
                .write()
                .map_err(|_| RepairError::Busy)?;
            let mut current = self.assessment.write().map_err(|_| RepairError::Busy)?;
            if current.state == RepairAssessmentState::Scanning {
                return Err(RepairError::Busy);
            }
            *owner = owner_principal_key.to_owned();
            *current = initial.clone();
        }

        let state = self.assessment.clone();
        let platform = self.platform.clone();
        let db = self.db.clone();
        let owner_for_worker = owner_principal_key.to_owned();
        if let Err(error) = thread::Builder::new()
            .name("aether-repair-assessment".into())
            .spawn(move || {
                let _read_budget_lease = read_budget_lease;
                let completed = now_ms();
                let next = match platform.assess() {
                    Ok((system_volume, checks)) => {
                        let intelligence = build_intelligence(&assessment_id, &checks);
                        // Reboot resume is deliberately a workflow marker, not mutation authority.
                        // Any prior marker is consumed by this fresh assessment; nothing continues
                        // from pre-reboot assumptions without rebuilding and revalidating the graph.
                        if let Ok(Some(previous)) =
                            db.latest_pending_repair_reboot_resume(&owner_for_worker)
                        {
                            if previous.assessment_id != assessment_id {
                                let _ = db.consume_repair_reboot_resume(
                                    &previous.token_sha256,
                                    "FreshAssessmentPerformed",
                                    completed,
                                );
                            }
                        }
                        if intelligence
                            .graph
                            .nodes
                            .iter()
                            .any(|node| node.action == RepairActionKind::Reboot)
                        {
                            let token = reboot_resume_token(
                                &assessment_id,
                                &intelligence.machine_state_fingerprint,
                                &intelligence.graph.digest_sha256,
                            );
                            let _ = db.upsert_repair_reboot_resume(&RepairRebootResumeRecord {
                                token_sha256: token,
                                owner_principal_key: owner_for_worker.clone(),
                                assessment_id: assessment_id.clone(),
                                machine_state_fingerprint: intelligence
                                    .machine_state_fingerprint
                                    .clone(),
                                repair_graph_digest: intelligence.graph.digest_sha256.clone(),
                                resume_policy: "FreshAssessmentRequiredAfterReboot".into(),
                                state: "AwaitingRebootReassessment".into(),
                                created_unix_ms: completed,
                                consumed_unix_ms: None,
                            });
                        }
                        RepairAssessment {
                            assessment_id,
                            state: RepairAssessmentState::Ready,
                            started_unix_ms: started,
                            completed_unix_ms: completed,
                            error_message: String::new(),
                            system_volume,
                            checks,
                            intelligence: Some(intelligence),
                        }
                    }
                    Err(error) => RepairAssessment {
                        assessment_id,
                        state: RepairAssessmentState::Failed,
                        started_unix_ms: started,
                        completed_unix_ms: completed,
                        error_message: error.to_string(),
                        system_volume: String::new(),
                        checks: Vec::new(),
                        intelligence: None,
                    },
                };
                if let Ok(mut guard) = state.write() {
                    *guard = next;
                }
            })
        {
            let detail = format!("repair assessment worker unavailable: {error}");
            if let Ok(mut current) = self.assessment.write() {
                current.state = RepairAssessmentState::Failed;
                current.completed_unix_ms = now_ms();
                current.error_message = detail.clone();
            }
            return Err(RepairError::Command(detail));
        }
        Ok(initial)
    }

    fn assessment(&self) -> RepairAssessment {
        // DBT-P46-B8: recover a poisoned lock's last-written value rather than
        // silently discarding it for an empty default — the pattern already
        // used throughout this codebase (e.g. diagnostic-engine, operation-kernel).
        self.assessment
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn assessment_for_owner(&self, owner_principal_key: &str) -> Result<RepairAssessment> {
        let owner = self
            .assessment_owner
            .read()
            .map_err(|_| RepairError::Busy)?;
        if owner.as_str() != owner_principal_key {
            return Err(RepairError::OwnershipMismatch);
        }
        Ok(self.assessment())
    }

    pub fn create_plan(
        &self,
        owner_principal_key: &str,
        assessment_id: &str,
        run_disk_scan: bool,
    ) -> Result<PlanView> {
        let current = self.assessment_for_owner(owner_principal_key)?;
        if current.state != RepairAssessmentState::Ready {
            return Err(RepairError::AssessmentNotReady);
        }
        if current.assessment_id != assessment_id {
            return Err(RepairError::StaleAssessment);
        }
        let intelligence = current
            .intelligence
            .as_ref()
            .ok_or(RepairError::AssessmentNotReady)?;
        intelligence
            .graph
            .validate_for_execution(&intelligence.recovery)
            .map_err(|e| RepairError::InvalidRepairGraph(e.to_string()))?;
        if intelligence
            .graph
            .nodes
            .iter()
            .any(|n| n.action == RepairActionKind::Reboot)
        {
            return Err(RepairError::RebootBoundary);
        }
        let executable_actions = intelligence
            .graph
            .nodes
            .iter()
            .filter(|n| {
                n.executable_automatically
                    && matches!(
                        n.action,
                        RepairActionKind::RepairComponentStore
                            | RepairActionKind::RepairSystemFiles
                            | RepairActionKind::StartRequiredService
                    )
            })
            .map(|n| n.action.canonical_id().to_owned())
            .collect::<Vec<_>>();
        let run_component_store = executable_actions
            .iter()
            .any(|id| id == "repair-component-store");
        let run_system_files = executable_actions
            .iter()
            .any(|id| id == "repair-system-files");
        if executable_actions.is_empty() {
            return Err(RepairError::NoRepairRecommended);
        }
        let graph_json = serde_json::to_string(&intelligence.graph)
            .map_err(|e| RepairError::Command(e.to_string()))?;
        let reboot_boundary_count = intelligence
            .graph
            .nodes
            .iter()
            .filter(|n| n.reboot_boundary_after)
            .count() as u32;
        let safety_tier = intelligence
            .graph
            .nodes
            .iter()
            .filter(|n| {
                n.executable_automatically
                    && executable_actions
                        .iter()
                        .any(|id| id == n.action.canonical_id())
            })
            .map(|n| n.safety)
            .max()
            .unwrap_or(RepairSafetyTier::Level2SensitiveRepair);
        let plan = self.engine.create_system_repair_plan(
            owner_principal_key,
            SystemRepairAction {
                assessment_id: assessment_id.into(),
                run_component_store,
                run_system_files,
                run_disk_scan: run_disk_scan
                    && current
                        .checks
                        .iter()
                        .any(|c| c.id == "disk-scan" && c.result_code != "NoErrors"),
                repair_action_ids: executable_actions,
                machine_state_fingerprint: intelligence.machine_state_fingerprint.clone(),
                repair_graph_digest: intelligence.graph.digest_sha256.clone(),
                repair_graph_json: graph_json,
                safety_tier: format!("{safety_tier:?}"),
                reboot_boundary_count,
            },
        )?;
        timeline_event(
            &self.db,
            owner_principal_key,
            &plan.id,
            assessment_id,
            "PlanSealed",
            "repair-graph",
            "AwaitingAuthorization",
            &format!(
                "graphDigest={};safety={safety_tier:?}",
                intelligence.graph.digest_sha256
            ),
            &intelligence.machine_state_fingerprint,
        );
        Ok(plan)
    }

    pub fn start_with_lease(
        &self,
        owner_principal_key: &str,
        plan_id: &str,
        lease: MutationLease,
    ) -> Result<RepairExecutionStatus> {
        if !lease.matches(MutationWorkload::SystemRepair, plan_id, owner_principal_key) {
            return Err(RepairError::Command(
                "mutation lease identity mismatch".into(),
            ));
        }
        self.start_inner(owner_principal_key, plan_id, lease)
    }

    fn start_inner(
        &self,
        owner_principal_key: &str,
        plan_id: &str,
        mutation_lease: MutationLease,
    ) -> Result<RepairExecutionStatus> {
        let plan = self
            .engine
            .get_plan_for_owner(plan_id, owner_principal_key)?;
        if plan.state != PlanState::AwaitingAuthorization {
            return Err(RepairError::AuthorizationRequired);
        }
        let sealed_action = self.engine.system_repair_action(plan_id)?;
        let current = self.assessment_for_owner(owner_principal_key)?;
        let current_intelligence = current
            .intelligence
            .as_ref()
            .ok_or(RepairError::StaleAssessment)?;
        if current.assessment_id != sealed_action.assessment_id
            || current_intelligence.machine_state_fingerprint
                != sealed_action.machine_state_fingerprint
            || current_intelligence.graph.digest_sha256 != sealed_action.repair_graph_digest
        {
            return Err(RepairError::StaleAssessment);
        }
        current_intelligence
            .graph
            .validate_for_execution(&current_intelligence.recovery)
            .map_err(|e| RepairError::InvalidRepairGraph(e.to_string()))?;

        {
            let mut running = self
                .running
                .lock()
                .map_err(|_| RepairError::AlreadyRunning)?;
            if running.is_some() {
                return Err(RepairError::AlreadyRunning);
            }
            *running = Some(plan_id.to_owned());
        }

        if let Err(error) = self.engine.consume_authorization_and_begin(
            plan_id,
            owner_principal_key,
            "one-shot consent consumed; system repair entered preflight",
        ) {
            if let Ok(mut running) = self.running.lock() {
                *running = None;
            }
            // A missing/expired one-shot consent must surface as the typed authorization error
            // instead of a generic engine wrapper, so callers can react to it specifically.
            if matches!(
                error,
                aethercore_operation_engine::EngineError::AuthorizationRequired
            ) {
                return Err(RepairError::AuthorizationRequired);
            }
            return Err(error.into());
        }

        let now = now_ms();
        if let Err(error) = self
            .db
            .upsert_maintenance_execution(&MaintenanceExecutionRecord {
                plan_id: plan_id.into(),
                domain: "SystemRepair".into(),
                stage: "Queued".into(),
                detail: "Authorized evidence-backed repair queued".into(),
                outcome: "Pending".into(),
                machine_state_fingerprint: sealed_action.machine_state_fingerprint.clone(),
                repair_graph_digest: sealed_action.repair_graph_digest.clone(),
                reboot_required: sealed_action.reboot_boundary_count > 0,
                verification_state: "Pending".into(),
                started_unix_ms: now,
                updated_unix_ms: now,
                ..Default::default()
            })
        {
            let _ = self.engine.transition(
                plan_id,
                PlanState::Preflight,
                PlanState::Failed,
                "repair execution journal initialization failed after consent",
            );
            if let Ok(mut running) = self.running.lock() {
                *running = None;
            }
            return Err(error.into());
        }

        timeline_event(
            &self.db,
            owner_principal_key,
            plan_id,
            &sealed_action.assessment_id,
            "AuthorizationConsumed",
            "repair-plan",
            "Queued",
            "One-shot consent consumed; immutable plan queued.",
            &sealed_action.machine_state_fingerprint,
        );

        let engine = self.engine.clone();
        let db = self.db.clone();
        let platform = self.platform.clone();
        let running = self.running.clone();
        let telemetry = self.telemetry.clone();
        let owner = owner_principal_key.to_owned();
        let id = plan_id.to_owned();
        let spawn = thread::Builder::new()
            .name("aether-system-repair-worker".into())
            .spawn(move || {
                let _mutation_lease = mutation_lease;
                if let Err(error) =
                    run_worker(&engine, &db, platform.as_ref(), &owner, &telemetry, &id)
                {
                    let _ = fail_repair(&engine, &db, &id, error);
                }
                telemetry.clear_for_owner(&owner, &id);
                if let Ok(mut guard) = running.lock() {
                    *guard = None;
                }
            });
        if let Err(error) = spawn {
            let detail = format!("repair worker creation failed: {error}");
            let _ = fail_repair(
                &self.engine,
                &self.db,
                plan_id,
                RepairError::Command(detail.clone()),
            );
            self.telemetry.clear_for_owner(owner_principal_key, plan_id);
            if let Ok(mut running) = self.running.lock() {
                *running = None;
            }
            return Err(RepairError::Command(detail));
        }

        self.status(owner_principal_key, Some(plan_id))?
            .ok_or_else(|| RepairError::Command("repair status missing".into()))
    }

    pub fn status(
        &self,
        owner_principal_key: &str,
        plan_id: Option<&str>,
    ) -> Result<Option<RepairExecutionStatus>> {
        let record = match plan_id {
            Some(id) => {
                self.engine.get_plan_for_owner(id, owner_principal_key)?;
                self.db.get_maintenance_execution(id)?
            }
            None => self
                .db
                .latest_maintenance_execution_for_owner("SystemRepair", owner_principal_key)?,
        };
        let Some(record) = record else {
            return Ok(None);
        };

        let plan = self
            .engine
            .get_plan_for_owner(&record.plan_id, owner_principal_key)?;
        let steps = self
            .db
            .maintenance_items(&record.plan_id)?
            .into_iter()
            .map(|item| RepairCheck {
                id: item.item_id,
                title: item.kind,
                stage: item.stage,
                result_code: item.result_code,
                exit_code: 0,
                detail: item.detail,
                log_hint: String::new(),
            })
            .collect();

        let live = self
            .telemetry
            .get_for_owner(owner_principal_key, &record.plan_id)
            .filter(|value| {
                value.owner_principal_key == owner_principal_key
                    && value.emitted_unix_ms >= record.updated_unix_ms
            });
        Ok(Some(RepairExecutionStatus {
            plan_id: record.plan_id,
            plan_state: plan.state.as_str().into(),
            stage: live
                .as_ref()
                .map(|value| value.stage.clone())
                .unwrap_or(record.stage),
            progress_known: live
                .as_ref()
                .map(|value| value.progress_known)
                .unwrap_or(record.progress_known),
            overall_percent: live
                .as_ref()
                .map(|value| value.overall_percent)
                .unwrap_or(record.overall_percent),
            current_step_id: live
                .as_ref()
                .map(|value| value.current_item_id.clone())
                .unwrap_or(record.current_item_id),
            detail: live
                .as_ref()
                .map(|value| value.detail.clone())
                .unwrap_or(record.detail),
            mutation_started: record.mutation_started,
            recovery_required: record.recovery_required,
            failure_message: record.failure_message,
            outcome: record.outcome,
            repair_graph_digest: record.repair_graph_digest,
            machine_state_fingerprint: record.machine_state_fingerprint,
            safety_tier: self
                .engine
                .system_repair_action(&plan.id)
                .map(|a| a.safety_tier)
                .unwrap_or_default(),
            reboot_required: record.reboot_required,
            verification_state: record.verification_state,
            started_unix_ms: record.started_unix_ms,
            updated_unix_ms: live
                .as_ref()
                .map(|value| value.emitted_unix_ms)
                .unwrap_or(record.updated_unix_ms),
            completed_unix_ms: record.completed_unix_ms,
            steps,
        }))
    }

    pub fn recover_incomplete(&self) -> Result<()> {
        for plan in self.engine.recoverable_plans()? {
            if self.engine.system_repair_action(&plan.id).is_err() {
                continue;
            }

            let mutated = matches!(plan.state, PlanState::Executing | PlanState::Verifying);
            let now = now_ms();
            let _ = self.engine.transition(
                &plan.id,
                plan.state,
                PlanState::Failed,
                "service restarted during system repair; automatic replay is intentionally disabled",
            );
            self.db
                .upsert_maintenance_execution(&MaintenanceExecutionRecord {
                    plan_id: plan.id.clone(),
                    domain: "SystemRepair".into(),
                    stage: "Interrupted".into(),
                    detail: if mutated {
                        "Repair was interrupted after mutation began. AetherCore will not replay DISM/SFC automatically."
                            .into()
                    } else {
                        "Repair was interrupted before mutation began. No repair command was replayed."
                            .into()
                    },
                    mutation_started: mutated,
                    recovery_required: mutated,
                    failure_message: "Service restart interrupted repair".into(),
                    outcome: if mutated { "FailedAfterMutation".into() } else { "FailedBeforeMutation".into() },
                    verification_state: if mutated { "VerificationRequiredAfterFreshAssessment".into() } else { "NotStarted".into() },
                    started_unix_ms: plan.created_unix_ms,
                    updated_unix_ms: now,
                    completed_unix_ms: Some(now),
                    ..Default::default()
                })?;

            if mutated {
                self.db.add_recovery_record(&RecoveryRecord {
                    plan_id: plan.id,
                    severity: "Amber".into(),
                    kind: "SystemRepairInterrupted".into(),
                    summary: "System repair needs review".into(),
                    detail: "No repair command was replayed automatically after restart. Review CBS/DISM logs and run a fresh assessment."
                        .into(),
                    created_unix_ms: now,
                    ..Default::default()
                })?;
            }
        }
        Ok(())
    }
}

fn publish_progress(
    telemetry: &ProgressTelemetryStore,
    owner_principal_key: &str,
    plan_id: &str,
    stage: &str,
    percent: u32,
    current_item_id: &str,
    detail: &str,
) {
    telemetry.publish(ProgressTelemetry {
        owner_principal_key: owner_principal_key.into(),
        plan_id: plan_id.into(),
        stage: stage.into(),
        // DISM/SFC/WUA do not expose a trustworthy common percent. Keep active repair
        // indeterminate and expose the real task/stage instead; only terminal completion is 100%.
        progress_known: percent >= 100,
        overall_percent: if percent >= 100 { 100 } else { 0 },
        current_item_id: current_item_id.into(),
        detail: detail.into(),
        ..Default::default()
    });
}

fn build_intelligence(assessment_id: &str, checks: &[RepairCheck]) -> RepairIntelligenceSnapshot {
    let facts = checks.iter().filter_map(check_to_fact).collect::<Vec<_>>();
    let recovery = recovery_from_checks(checks);
    let fingerprint = canonical_machine_state_fingerprint(&facts, &recovery);
    analyze_windows_repair(&RepairObservationSet {
        observation_id: assessment_id.to_owned(),
        machine_state_fingerprint: fingerprint,
        facts,
        recovery,
    })
}

fn check_to_fact(check: &RepairCheck) -> Option<RepairFact> {
    let (domain, state, confidence) = match check.id.as_str() {
        id if id.starts_with("dism") || id == "verify-dism" => {
            let state = match check.result_code.as_str() {
                "ComponentStoreHealthy" => FactState::Healthy,
                "ComponentStoreRepairable" | "ComponentStoreCorruptionDetected" => {
                    FactState::CorruptionDetected
                }
                "ComponentStoreNonRepairable" => FactState::RepairFailed,
                "SourceRequired" => FactState::SourceRequired,
                "RepairFailed" => FactState::RepairFailed,
                _ => FactState::Unknown,
            };
            let confidence = if state == FactState::Unknown {
                DiagnosisConfidence::Low
            } else {
                DiagnosisConfidence::Confirmed
            };
            (RepairDomain::ComponentStore, state, confidence)
        }
        id if id.starts_with("sfc") || id == "verify-sfc" => {
            let state = match check.result_code.as_str() {
                "SystemFilesHealthy" => FactState::Healthy,
                "SystemFilesCorrupt" | "SystemFilesRepairable" => FactState::CorruptionDetected,
                "SystemFilesRepairFailed" => FactState::RepairFailed,
                _ => FactState::Unknown,
            };
            let confidence = if state == FactState::Unknown {
                DiagnosisConfidence::Low
            } else {
                DiagnosisConfidence::Confirmed
            };
            (RepairDomain::SystemFiles, state, confidence)
        }
        "disk-scan" | "verify-disk" => {
            let state = if check.result_code == "NoErrors" {
                FactState::Healthy
            } else if check.result_code.starts_with("ChkdskExit") {
                FactState::Repairable
            } else {
                FactState::Unknown
            };
            let confidence = if state == FactState::Unknown {
                DiagnosisConfidence::Low
            } else {
                DiagnosisConfidence::High
            };
            (RepairDomain::Filesystem, state, confidence)
        }
        "servicing-state" => {
            let state = match check.result_code.as_str() {
                "ServicingBusy" => FactState::Active,
                "RebootPending" => FactState::RebootRequired,
                "ServicingAvailable" => FactState::Healthy,
                _ => FactState::Unknown,
            };
            let domain = if state == FactState::RebootRequired {
                RepairDomain::Reboot
            } else {
                RepairDomain::Servicing
            };
            (domain, state, DiagnosisConfidence::High)
        }
        "windows-update" => {
            let state = match check.result_code.as_str() {
                "UpdateHealthy" => FactState::Healthy,
                "UpdateOffline" => FactState::Offline,
                "UpdateFailure" => FactState::Failure,
                _ => FactState::Unknown,
            };
            (
                RepairDomain::WindowsUpdate,
                state,
                DiagnosisConfidence::High,
            )
        }
        "required-service" => {
            let state = match check.result_code.as_str() {
                "ServiceRunning" => FactState::Healthy,
                "ServiceStopped" => FactState::Stopped,
                "ServiceDisabled" => FactState::Disabled,
                _ => FactState::Unknown,
            };
            (RepairDomain::Services, state, DiagnosisConfidence::High)
        }
        "network-connectivity" => (
            RepairDomain::Network,
            if check.result_code == "NetworkHealthy" {
                FactState::Healthy
            } else if check.result_code == "NetworkOffline" {
                FactState::Offline
            } else {
                FactState::Unknown
            },
            DiagnosisConfidence::High,
        ),
        "dns-resolution" => (
            RepairDomain::Dns,
            if check.result_code == "DnsHealthy" {
                FactState::Healthy
            } else if check.result_code == "DnsFailure" {
                FactState::Failure
            } else {
                FactState::Unknown
            },
            DiagnosisConfidence::High,
        ),
        "proxy-state" => (
            RepairDomain::Proxy,
            if check.result_code == "ProxyHealthy" {
                FactState::Healthy
            } else if check.result_code == "ProxyUnexpected" {
                FactState::UnexpectedConfiguration
            } else {
                FactState::Unknown
            },
            DiagnosisConfidence::Medium,
        ),
        "winre-state" => (
            RepairDomain::Recovery,
            if check.result_code == "WinReAvailable" {
                FactState::Available
            } else if check.result_code == "WinReUnavailable" {
                FactState::Unavailable
            } else {
                FactState::Unknown
            },
            DiagnosisConfidence::High,
        ),
        _ => return None,
    };
    Some(RepairFact {
        id: format!("fact:{}", check.id),
        domain,
        state,
        resource: check.id.clone(),
        evidence_code: check.result_code.clone(),
        technical_code: if check.exit_code != 0 {
            format!("exit:{}", check.exit_code)
        } else {
            String::new()
        },
        detail: check.detail.clone(),
        observed_unix_ms: now_ms(),
        confidence,
    })
}

fn recovery_from_checks(checks: &[RepairCheck]) -> RecoveryReadiness {
    let mut recovery = RecoveryReadiness::default();
    if let Some(check) = checks.iter().find(|c| c.id == "winre-state") {
        recovery.win_re = match check.result_code.as_str() {
            "WinReAvailable" => FactState::Available,
            "WinReUnavailable" => FactState::Unavailable,
            _ => FactState::Unknown,
        };
    }
    if let Some(check) = checks.iter().find(|c| c.id == "restore-state") {
        recovery.system_restore = match check.result_code.as_str() {
            "RestoreAvailable" => FactState::Available,
            "RestoreUnavailable" => FactState::Unavailable,
            _ => FactState::Unknown,
        };
        recovery.restore_point_creation = recovery.system_restore;
    }
    recovery
}

fn verification_proves_success(
    action: &SystemRepairAction,
    steps: &[MaintenanceItemRecord],
) -> bool {
    let result = |id: &str| {
        steps
            .iter()
            .find(|s| s.item_id == id)
            .map(|s| s.result_code.as_str())
    };
    let component_ok = !action.run_component_store
        || matches!(
            result("verify-dism"),
            Some("ComponentStoreHealthy") | Some("VerifiedHealthy")
        );
    let system_ok = !action.run_system_files
        || matches!(
            result("verify-sfc"),
            Some("SystemFilesHealthy") | Some("VerifiedHealthy")
        );
    let disk_ok = !action.run_disk_scan
        || matches!(
            result("verify-disk"),
            Some("NoErrors") | Some("VerifiedHealthy")
        );
    let service_selected = action
        .repair_action_ids
        .iter()
        .any(|id| id == "start-required-service");
    let service_ok =
        !service_selected || matches!(result("verify-required-service"), Some("ServiceRunning"));
    let update_ok =
        !service_selected || matches!(result("verify-windows-update"), Some("UpdateHealthy"));
    component_ok && system_ok && disk_ok && service_ok && update_ok
}

fn timeline_event(
    db: &Database,
    owner: &str,
    plan_id: &str,
    assessment_id: &str,
    kind: &str,
    action_id: &str,
    outcome: &str,
    detail: &str,
    fingerprint: &str,
) {
    let _ = db.insert_repair_timeline_event(&RepairTimelineEventRecord {
        event_id: Uuid::new_v4().to_string(),
        plan_id: plan_id.into(),
        assessment_id: assessment_id.into(),
        owner_principal_key: owner.into(),
        event_kind: kind.into(),
        domain: "WindowsRepair".into(),
        action_id: action_id.into(),
        diagnosis_code: String::new(),
        outcome: outcome.into(),
        detail: detail.into(),
        machine_state_fingerprint: fingerprint.into(),
        created_unix_ms: now_ms(),
    });
}

fn run_worker(
    engine: &OperationEngine,
    db: &Database,
    platform: &dyn RepairPlatform,
    owner_principal_key: &str,
    telemetry: &ProgressTelemetryStore,
    plan_id: &str,
) -> Result<()> {
    let action = engine.system_repair_action(plan_id)?;
    timeline_event(
        db,
        owner_principal_key,
        plan_id,
        &action.assessment_id,
        "ExecutionStarted",
        "repair-plan",
        "Preflight",
        "Execution began from the sealed repair graph.",
        &action.machine_state_fingerprint,
    );
    publish_progress(
        telemetry,
        owner_principal_key,
        plan_id,
        "Preflight",
        5,
        "",
        "Checking servicing safety",
    );
    update_exec(
        db,
        plan_id,
        "Preflight",
        5,
        "Checking servicing safety",
        false,
        None,
    )?;
    engine.transition(
        plan_id,
        PlanState::Preflight,
        PlanState::Protected,
        "fixed executable/argument allowlist committed",
    )?;
    publish_progress(
        telemetry,
        owner_principal_key,
        plan_id,
        "Protected",
        8,
        "",
        "Repair workflow is frozen; waiting for the servicing mutation barrier",
    );
    update_exec(
        db,
        plan_id,
        "Protected",
        8,
        "Repair workflow is frozen; waiting for the servicing mutation barrier",
        false,
        None,
    )?;

    let mutation_started = std::cell::Cell::new(false);
    let mut begin_mutation = || -> Result<()> {
        if mutation_started.replace(true) {
            return Ok(());
        }
        engine.transition(
            plan_id,
            PlanState::Protected,
            PlanState::Executing,
            "servicing lock acquired; begin Windows integrity repair",
        )?;
        publish_progress(
            telemetry,
            owner_principal_key,
            plan_id,
            "Executing",
            10,
            "",
            "Running supported Windows repair tools",
        );
        update_exec(
            db,
            plan_id,
            "Executing",
            10,
            "Running supported Windows repair tools",
            true,
            None,
        )?;
        timeline_event(
            db,
            owner_principal_key,
            plan_id,
            &action.assessment_id,
            "MutationStarted",
            "repair-plan",
            "Executing",
            "Durable mutation boundary crossed.",
            &action.machine_state_fingerprint,
        );
        Ok(())
    };

    let mut index = 0u32;
    let mut emit = |step: RepairCheck| {
        index += 1;
        let _ = db.upsert_maintenance_item(&MaintenanceItemRecord {
            plan_id: plan_id.into(),
            item_id: step.id.clone(),
            kind: step.title.clone(),
            stage: step.stage.clone(),
            result_code: step.result_code.clone(),
            detail: step.detail.clone(),
            updated_unix_ms: now_ms(),
            ..Default::default()
        });
        timeline_event(
            db,
            owner_principal_key,
            plan_id,
            &action.assessment_id,
            "StepRecorded",
            &step.id,
            &step.result_code,
            &step.title,
            &action.machine_state_fingerprint,
        );
        let percent = (10 + index * 12).min(75);
        let mutated = mutation_started.get();
        let stage = if mutated { "Executing" } else { "Protected" };
        publish_progress(
            telemetry,
            owner_principal_key,
            plan_id,
            stage,
            percent,
            &step.id,
            &step.title,
        );
        let _ = update_exec(db, plan_id, stage, percent, &step.title, mutated, None);
    };

    if let Err(error) = platform.repair(&action, &mut begin_mutation, &mut emit) {
        timeline_event(
            db,
            owner_principal_key,
            plan_id,
            &action.assessment_id,
            "ExecutionError",
            "repair-plan",
            "Failed",
            &error.to_string(),
            &action.machine_state_fingerprint,
        );
        return fail_repair(engine, db, plan_id, error);
    }
    if !mutation_started.get() {
        return fail_repair(
            engine,
            db,
            plan_id,
            RepairError::Command("repair platform did not enter the mutation barrier".into()),
        );
    }

    engine.transition(
        plan_id,
        PlanState::Executing,
        PlanState::Verifying,
        "repair commands completed; begin verification",
    )?;
    publish_progress(
        telemetry,
        owner_principal_key,
        plan_id,
        "Verifying",
        80,
        "",
        "Verifying component store and protected files",
    );
    update_exec(
        db,
        plan_id,
        "Verifying",
        80,
        "Verifying component store and protected files",
        true,
        None,
    )?;
    if let Err(error) = platform.verify(&action, &mut emit) {
        timeline_event(
            db,
            owner_principal_key,
            plan_id,
            &action.assessment_id,
            "VerificationError",
            "repair-plan",
            "VerificationFailed",
            &error.to_string(),
            &action.machine_state_fingerprint,
        );
        return fail_repair(engine, db, plan_id, error);
    }
    let verification_steps = db.maintenance_items(plan_id)?;
    if !verification_proves_success(&action, &verification_steps) {
        let mut record = db.get_maintenance_execution(plan_id)?.unwrap_or_default();
        record.verification_state = "FailedOrUnknown".into();
        record.outcome = format!("{:?}", RepairOutcome::MutationSucceededVerificationFailed);
        record.recovery_required = true;
        record.failure_message = RepairError::VerificationFailed.to_string();
        record.updated_unix_ms = now_ms();
        db.upsert_maintenance_execution(&record)?;
        timeline_event(
            db,
            owner_principal_key,
            plan_id,
            &action.assessment_id,
            "VerificationFailed",
            "repair-plan",
            &record.outcome,
            "Mutation completed but evidence-specific verification did not prove resolution.",
            &action.machine_state_fingerprint,
        );
        return fail_repair(engine, db, plan_id, RepairError::VerificationFailed);
    }

    engine.transition(
        plan_id,
        PlanState::Verifying,
        PlanState::Completed,
        "Windows integrity repair verified",
    )?;
    let now = now_ms();
    let mut record = db.get_maintenance_execution(plan_id)?.unwrap_or_default();
    record.stage = "Completed".into();
    record.progress_known = true;
    record.overall_percent = 100;
    record.detail =
        "Repair workflow completed and evidence-specific verification proved the intended state."
            .into();
    record.outcome = format!("{:?}", RepairOutcome::SucceededVerified);
    record.verification_state = "Verified".into();
    record.updated_unix_ms = now;
    record.completed_unix_ms = Some(now);
    db.upsert_maintenance_execution(&record)?;
    timeline_event(
        db,
        owner_principal_key,
        plan_id,
        &action.assessment_id,
        "VerificationSucceeded",
        "repair-plan",
        &record.outcome,
        "Evidence-specific post-repair verification proved the intended state.",
        &action.machine_state_fingerprint,
    );
    publish_progress(
        telemetry,
        owner_principal_key,
        plan_id,
        "Completed",
        100,
        "",
        &record.detail,
    );
    Ok(())
}

fn fail_repair(
    engine: &OperationEngine,
    db: &Database,
    plan_id: &str,
    error: RepairError,
) -> Result<()> {
    let current = engine.get_plan(plan_id)?;
    if !current.state.is_terminal() {
        let _ = engine.transition(
            plan_id,
            current.state,
            PlanState::Failed,
            "system repair failed",
        );
    }

    let now = now_ms();
    let mut record = db.get_maintenance_execution(plan_id)?.unwrap_or_default();
    record.plan_id = plan_id.into();
    record.domain = "SystemRepair".into();
    record.stage = "Failed".into();
    record.failure_message = error.to_string();
    record.detail = "Repair stopped. No command will be replayed automatically.".into();
    record.recovery_required = record.mutation_started;
    if record.outcome.is_empty() || record.outcome == "Pending" {
        record.outcome = format!(
            "{:?}",
            if record.mutation_started {
                RepairOutcome::FailedAfterMutation
            } else {
                RepairOutcome::FailedBeforeMutation
            }
        );
    }
    if record.verification_state.is_empty() {
        record.verification_state = "NotVerified".into();
    }
    record.updated_unix_ms = now;
    record.completed_unix_ms = Some(now);
    db.upsert_maintenance_execution(&record)?;
    Err(error)
}

fn update_exec(
    db: &Database,
    plan_id: &str,
    stage: &str,
    percent: u32,
    detail: &str,
    mutation: bool,
    completed: Option<i64>,
) -> Result<()> {
    let now = now_ms();
    let mut record =
        db.get_maintenance_execution(plan_id)?
            .unwrap_or_else(|| MaintenanceExecutionRecord {
                plan_id: plan_id.into(),
                domain: "SystemRepair".into(),
                started_unix_ms: now,
                ..Default::default()
            });
    record.stage = stage.into();
    record.progress_known = percent >= 100;
    record.overall_percent = if percent >= 100 { 100 } else { 0 };
    record.detail = detail.into();
    record.mutation_started |= mutation;
    record.updated_unix_ms = now;
    if completed.is_some() {
        record.completed_unix_ms = completed;
    }
    db.upsert_maintenance_execution(&record)?;
    Ok(())
}

#[cfg(test)]
mod dbt_p46_b8_tests {
    use super::*;
    use std::sync::RwLock;

    // DBT-P46-B8: assessment() read a poisoned lock (left behind by a prior
    // panic while holding the write lock) as `.unwrap_or_default()` — silently
    // discarding the last-known-good assessment in favor of an empty one. This
    // crate has no existing test scaffolding to construct a full RepairEngine
    // (needs OperationEngine + Database), so this proves the exact recovery
    // mechanism the fix applies, on the same lock type the real field uses.
    #[test]
    fn poisoned_assessment_lock_recovers_the_last_written_value_not_a_default() {
        let lock: RwLock<RepairAssessment> = RwLock::new(RepairAssessment {
            assessment_id: "real-assessment".into(),
            ..RepairAssessment::default()
        });
        let poison_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = lock.write().unwrap();
            panic!("simulated panic while holding the write lock");
        }));
        assert!(
            poison_result.is_err(),
            "the panic must have actually happened"
        );
        assert!(lock.is_poisoned(), "the lock must now be poisoned");

        // Old behavior: .read().map(|a| a.clone()).unwrap_or_default()
        let old_behavior = lock.read().map(|a| a.clone()).unwrap_or_default();
        assert_eq!(
            old_behavior.assessment_id, "",
            "documents the bug this fix removes: a poisoned lock silently became an empty default"
        );

        // New behavior, as applied in `assessment()`.
        let recovered = lock
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        assert_eq!(
            recovered.assessment_id, "real-assessment",
            "a poisoned lock must recover the last-written value, not silently default"
        );
    }
}

#[cfg(test)]
mod dbt_p46_b9_tests {
    use super::*;

    /// DBT-P46-B9. `run_with_accepted_codes` is Windows-only, so this proves
    /// the exact mechanism the fix applies, against a thread that really does
    /// panic — the same shape B8's test uses for the same reason.
    #[test]
    fn a_panicking_reader_thread_is_reported_not_read_as_silence() {
        let handle = std::thread::spawn(|| -> String {
            panic!("simulated panic inside the output reader");
        });
        let joined = handle.join();
        assert!(
            joined.is_err(),
            "the reader thread must have actually panicked"
        );

        // Old behavior: join().unwrap_or_default() — a crash became "no output".
        let mut notes = Vec::new();
        let text = joined_stream(joined, "stdout", &mut notes);
        assert_eq!(text, "", "there genuinely is no captured output");
        assert_eq!(notes.len(), 1, "but the reason must not be lost");
        assert!(
            notes[0].contains("stdout") && notes[0].contains("panicked"),
            "note must name the stream and say it crashed: {}",
            notes[0]
        );
    }

    #[test]
    fn a_healthy_reader_thread_adds_no_note() {
        let handle = std::thread::spawn(|| "command output".to_string());
        let mut notes = Vec::new();
        assert_eq!(
            joined_stream(handle.join(), "stdout", &mut notes),
            "command output"
        );
        assert!(notes.is_empty());
    }

    /// Adjacent defect found in the same four lines: the 48_000-byte tail was
    /// sliced at a fixed byte offset. DISM and SFC output is localized, so that
    /// offset can land mid-character, and `String` indexing panics there. The
    /// old expression is written out below to show what this now avoids.
    #[test]
    fn tail_truncation_survives_a_multibyte_boundary() {
        // 2-byte characters then one ASCII byte, so len - 48_000 is odd and
        // therefore lands inside a character.
        let mut detail = format!("{}x", "ة".repeat(30_000));
        assert_eq!(detail.len(), 60_001);
        assert!(
            !detail.is_char_boundary(detail.len() - 48_000),
            "this input must actually land mid-character, or it proves nothing"
        );
        trim_to_tail(&mut detail, 48_000);
        assert!(detail.len() <= 48_000);
        assert!(
            detail.chars().all(|c| c == 'ة' || c == 'x'),
            "no character was cut in half"
        );
    }

    #[test]
    fn tail_truncation_leaves_short_output_alone() {
        let mut detail = "short".to_string();
        trim_to_tail(&mut detail, 48_000);
        assert_eq!(detail, "short");
    }
}
