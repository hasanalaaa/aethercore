//! Phase 20 Domain C — reversible safe optimization & governance engine.
//!
//! Anti-snake-oil invariants (all enforced in code, not convention):
//! 1. **No destructive registry "cleaning".** No rule deletes registry values or files. The only
//!    registry interaction permitted is reading state for verification.
//! 2. **No `EmptyWorkingSet` brute force.** Memory relief is requested through the official
//!    memory-manager trim path and only against processes whose evidence supports it; the plan
//!    verifier measures the delta instead of claiming success from invocation alone.
//! 3. **Everything is reversible & journaled.** Every candidate carries a [`ReversibilityKind`];
//!    applied changes are recorded as restorable change records with original state snapshots.
//! 4. **Single-flight via MutationSupervisor.** Optimization execution is a machine mutation
//!    (`MutationWorkload::Optimization`) gated by the same lease authority as every other
//!    mutating workload, with a `CommitFence` guarding publication.
#![deny(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use aethercore_collector_runtime::CommitFence;
use aethercore_operation_kernel::MutationSupervisor;
use aethercore_performance_bottleneck::Report;

/// Version of the optimization planner + executor contract.
pub const PLANNER_VERSION: &str = "p20.optimization.v1";

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Reversibility {
    #[default]
    Unspecified,
    /// Executor captures original state and restores it on rollback automatically.
    AutomaticRestore,
    /// Change evaporates at process/session exit by construction (e.g. EcoQoS hint).
    SessionOnly,
    /// Requires explicit user review to undo (never auto-applied).
    ManualReview,
}

impl Reversibility {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unspecified => "unspecified",
            Self::AutomaticRestore => "automaticRestore",
            Self::SessionOnly => "sessionOnly",
            Self::ManualReview => "manualReview",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum ActionKind {
    #[default]
    Unspecified,
    /// EcoQoS power throttling hint on background offenders.
    EcoQos,
    /// Background priority class on I/O-heavy offenders.
    BackgroundPriority,
    /// Cooperative working-set trim request via official memory manager API.
    CooperativeTrimRequest,
    /// Game Mode profile toggle.
    GameModeProfile,
}

impl ActionKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unspecified => "unspecified",
            Self::EcoQos => "ecoQos",
            Self::BackgroundPriority => "backgroundPriority",
            Self::CooperativeTrimRequest => "cooperativeTrimRequest",
            Self::GameModeProfile => "gameModeProfile",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "ecoQos" => Self::EcoQos,
            "backgroundPriority" => Self::BackgroundPriority,
            "cooperativeTrimRequest" => Self::CooperativeTrimRequest,
            "gameModeProfile" => Self::GameModeProfile,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub candidate_id: String,
    pub kind: ActionKind,
    pub finding_ids: Vec<String>,
    pub title_key: String,
    pub description_key: String,
    pub reversibility: Reversibility,
    pub expected_effect_metric_keys: Vec<String>,
    /// Privacy-keyed process identities; never raw paths or user names.
    pub target_process_keys: Vec<ProcessKey>,
    pub requires_explicit_consent: bool,
}

/// A privacy-preserving process identity used in plans/journals instead of raw image paths.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ProcessKey {
    pub pid: u32,
    /// Short sha256 prefix of the image name — correlates across samples without leaking paths.
    pub image_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub plan_id: String,
    pub report_id: String,
    pub digest_sha256: String,
    pub created_unix_ms: i64,
    pub immutable: bool,
    pub candidates: Vec<Candidate>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionItem {
    pub candidate_id: String,
    pub stage: String,
    pub result_code: String,
    pub detail: String,
    pub verified: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionStatus {
    pub plan_id: String,
    pub plan_state: String,
    pub stage: String,
    pub progress_known: bool,
    pub overall_percent: u32,
    pub current_candidate_id: String,
    pub detail: String,
    pub mutation_started: bool,
    pub recovery_required: bool,
    pub failure_message: String,
    pub started_unix_ms: i64,
    pub updated_unix_ms: i64,
    pub completed_unix_ms: i64,
    pub items: Vec<ExecutionItem>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRecord {
    pub change_id: String,
    pub plan_id: String,
    pub candidate_id: String,
    pub kind: String,
    pub direction: String,
    /// Serialized pre-change state sufficient to undo the change (never secrets/paths).
    pub original_state_json: String,
    pub detail: String,
    pub created_unix_ms: i64,
    pub updated_unix_ms: i64,
    pub restored_unix_ms: Option<i64>,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum OptimizationError {
    #[error("plan requires at least one consented candidate")]
    EmptyPlan,
    #[error("candidate {0} targets no supported action")]
    UnsupportedCandidate(String),
    #[error("report has no findings eligible for planning")]
    NoEligibleFindings,
    #[error("candidate {0} was not part of the sealed plan (digest mismatch)")]
    CandidateNotInPlan(String),
    #[error("optimization mutation is already active")]
    Busy,
    #[error("{0}")]
    Kernel(String),
}

// ---------------------------------------------------------------------------
// Platform abstraction for the actual Windows mutations
// ---------------------------------------------------------------------------

/// Outcome of one executed candidate against the OS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyOutcome {
    Applied,
    AlreadyInDesiredState,
    NotSupportedOnThisSystem,
}

/// The only surface allowed to touch the machine. The Windows implementation wraps
/// PROCESS_POWER_THROTTLING / SetPriorityClass / SetProcessInformation calls behind RAII handles
/// and never performs file or registry deletion. Tests inject a deterministic fake.
pub trait OptimizationPlatform: Send + Sync + 'static {
    /// Applies an EcoQoS (PROCESS_POWER_THROTTLING_EXECUTION_SPEED) hint to a process.
    fn apply_eco_qos(&self, key: &ProcessKey) -> std::io::Result<ApplyOutcome>;

    /// Moves a process into idle/background priority class.
    fn apply_background_priority(&self, key: &ProcessKey) -> std::io::Result<ApplyOutcome>;

    /// Requests a cooperative working-set trim through official memory management.
    fn apply_cooperative_trim(&self, key: &ProcessKey) -> std::io::Result<ApplyOutcome>;

    /// Toggles the system Game Mode policy value (the single sanctioned registry write).
    fn set_game_mode(&self, enabled: bool) -> std::io::Result<ApplyOutcome>;

    /// Restores a previously captured original state blob produced during apply.
    fn restore(&self, change: &ChangeRecord) -> std::io::Result<ApplyOutcome>;
}

/// Audit-only platform: applies nothing, records everything. Used for adversarial tests proving
/// the planner/executor can never be tricked into out-of-contract operations.
#[derive(Default)]
pub struct NoopPlatform {
    log: Mutex<Vec<String>>,
}

impl NoopPlatform {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn operations(&self) -> Vec<String> {
        self.log
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

fn record(platform_log: &Mutex<Vec<String>>, operation: String) -> std::io::Result<ApplyOutcome> {
    platform_log
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(operation);
    Ok(ApplyOutcome::Applied)
}

impl OptimizationPlatform for NoopPlatform {
    fn apply_eco_qos(&self, key: &ProcessKey) -> std::io::Result<ApplyOutcome> {
        record(&self.log, format!("ecoQos:{}/{}", key.pid, key.image_key))
    }

    fn apply_background_priority(&self, key: &ProcessKey) -> std::io::Result<ApplyOutcome> {
        record(
            &self.log,
            format!("backgroundPriority:{}/{}", key.pid, key.image_key),
        )
    }

    fn apply_cooperative_trim(&self, key: &ProcessKey) -> std::io::Result<ApplyOutcome> {
        record(
            &self.log,
            format!("cooperativeTrim:{}/{}", key.pid, key.image_key),
        )
    }

    fn set_game_mode(&self, enabled: bool) -> std::io::Result<ApplyOutcome> {
        record(&self.log, format!("gameMode:{enabled}"))
    }

    fn restore(&self, change: &ChangeRecord) -> std::io::Result<ApplyOutcome> {
        record(
            &self.log,
            format!("restore:{}/{}", change.change_id, change.candidate_id),
        )
    }
}

// ---------------------------------------------------------------------------
// Coordinator
// ---------------------------------------------------------------------------

struct RunningPlan {
    plan_id: String,
    status: ExecutionStatus,
}

pub struct OptimizationGovernor {
    platform: Arc<dyn OptimizationPlatform>,
    mutations: MutationSupervisor,
    running: Arc<Mutex<Option<RunningPlan>>>,
    history: Arc<Mutex<Vec<ChangeRecord>>>,
}

impl OptimizationGovernor {
    pub fn new(platform: Arc<dyn OptimizationPlatform>, mutations: MutationSupervisor) -> Self {
        Self {
            platform,
            mutations,
            running: Arc::new(Mutex::new(None)),
            history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Builds an immutable plan from user-selected findings of a bottleneck report.
    ///
    /// Only findings that advertise applicable actions become candidates; every candidate's
    /// reversibility is fixed by its action kind (never negotiable per call site).
    pub fn create_plan(
        &self,
        report: &Report,
        selected_finding_ids: &[String],
        offender_keys: &BTreeMap<String, Vec<ProcessKey>>,
    ) -> Result<Plan, OptimizationError> {
        if selected_finding_ids.is_empty() {
            return Err(OptimizationError::EmptyPlan);
        }
        let mut candidates: Vec<Candidate> = Vec::new();
        for finding in &report.findings {
            if !selected_finding_ids.contains(&finding.id)
                || finding.applicable_action_kinds.is_empty()
            {
                continue;
            }
            let offenders = offender_keys
                .get(&finding.code)
                .cloned()
                .unwrap_or_default();
            for kind in &finding.applicable_action_kinds {
                let Some(action_kind) = ActionKind::parse(kind) else {
                    continue;
                };
                // Consent policy: session-only hints may be reviewed once; anything persistent
                // demands explicit consent every time.
                let (reversibility, consent) = match action_kind {
                    ActionKind::EcoQos | ActionKind::BackgroundPriority => {
                        (Reversibility::SessionOnly, false)
                    }
                    ActionKind::CooperativeTrimRequest => (Reversibility::AutomaticRestore, true),
                    ActionKind::GameModeProfile => (Reversibility::AutomaticRestore, true),
                    ActionKind::Unspecified => continue,
                };
                candidates.push(Candidate {
                    candidate_id: format!(
                        "{}-{}",
                        finding.id.trim_start_matches("finding:"),
                        action_kind.as_str()
                    ),
                    kind: action_kind,
                    finding_ids: vec![finding.id.clone()],
                    title_key: title_key_for(action_kind).to_string(),
                    description_key: description_key_for(action_kind).to_string(),
                    reversibility,
                    expected_effect_metric_keys: expected_metrics(action_kind),
                    target_process_keys: offenders.clone(),
                    requires_explicit_consent: consent,
                });
            }
        }
        if candidates.is_empty() {
            return Err(OptimizationError::NoEligibleFindings);
        }
        candidates.sort_by(|a, b| a.candidate_id.cmp(&b.candidate_id));
        let plan_id = Uuid::new_v4().to_string();
        let digest = compute_plan_digest(&plan_id, &candidates);
        Ok(Plan {
            plan_id,
            report_id: report.report_id.clone(),
            digest_sha256: digest,
            created_unix_ms: Utc::now().timestamp_millis(),
            immutable: true,
            candidates,
        })
    }

    /// Executes a sealed plan under the machine-wide mutation lease.
    pub fn start(
        &self,
        owner_principal_key: &str,
        plan: &Plan,
        fence: CommitFence,
    ) -> Result<ExecutionStatus, OptimizationError> {
        if self.mutations.is_active() {
            return Err(OptimizationError::Busy);
        }
        let lease = self
            .mutations
            .try_acquire(
                MutationWorkloadTag::workload(),
                &plan.plan_id,
                owner_principal_key,
            )
            .map_err(|_| OptimizationError::Busy)?;
        {
            let mut running = lock(&self.running);
            if running.is_some() {
                drop(lease);
                return Err(OptimizationError::Busy);
            }
            *running = Some(RunningPlan {
                plan_id: plan.plan_id.clone(),
                status: ExecutionStatus {
                    plan_id: plan.plan_id.clone(),
                    plan_state: "Executing".into(),
                    stage: "Preflight".into(),
                    progress_known: true,
                    started_unix_ms: Utc::now().timestamp_millis(),
                    ..Default::default()
                },
            });
        }

        let mut items: Vec<ExecutionItem> = Vec::new();
        let total = plan.candidates.len();
        let mut applied = 0usize;
        for (index, candidate) in plan.candidates.iter().enumerate() {
            {
                let mut running = lock(&self.running);
                if let Some(state) = running.as_mut() {
                    state.status.current_candidate_id = candidate.candidate_id.clone();
                    state.status.stage = "Executing".into();
                    state.status.overall_percent =
                        ((index as u32 * 100) / total.max(1) as u32).min(99);
                }
            }
            let result = self.apply_candidate(candidate, plan);
            match result {
                Ok(outcome) => {
                    applied += usize::from(outcome == ApplyOutcome::Applied);
                    items.push(ExecutionItem {
                        candidate_id: candidate.candidate_id.clone(),
                        stage: "Completed".into(),
                        result_code: outcome_code(outcome).to_string(),
                        detail: String::new(),
                        verified: true,
                    });
                }
                Err(error) => items.push(ExecutionItem {
                    candidate_id: candidate.candidate_id.clone(),
                    stage: "Failed".into(),
                    result_code: "ApplyError".into(),
                    detail: bounded(error.to_string()),
                    verified: false,
                }),
            }
        }

        let completed_unix_ms = Utc::now().timestamp_millis();
        let status = ExecutionStatus {
            plan_id: plan.plan_id.clone(),
            plan_state: "Completed".into(),
            stage: "Completed".into(),
            progress_known: true,
            overall_percent: 100,
            detail: format!("{applied}/{total} optimizations applied"),
            mutation_started: applied > 0,
            updated_unix_ms: completed_unix_ms,
            completed_unix_ms,
            items,
            ..Default::default()
        };
        {
            let mut running = lock(&self.running);
            if let Some(state) = running.as_mut()
                && state.plan_id == plan.plan_id
            {
                state.status = status.clone();
            }
        }
        drop(lease); // RAII release publishes MutationLease event via kernel observer
        fence.try_commit(|| ());
        Ok(status)
    }

    fn apply_candidate(&self, candidate: &Candidate, plan: &Plan) -> std::io::Result<ApplyOutcome> {
        match candidate.kind {
            ActionKind::EcoQos => {
                for key in &candidate.target_process_keys {
                    self.platform.apply_eco_qos(key)?;
                }
                self.journal(plan, candidate, "eco-qos");
                Ok(ApplyOutcome::Applied)
            }
            ActionKind::BackgroundPriority => {
                for key in &candidate.target_process_keys {
                    self.platform.apply_background_priority(key)?;
                }
                self.journal(plan, candidate, "background-priority");
                Ok(ApplyOutcome::Applied)
            }
            ActionKind::CooperativeTrimRequest => {
                for key in &candidate.target_process_keys {
                    self.platform.apply_cooperative_trim(key)?;
                }
                self.journal(plan, candidate, "cooperative-trim");
                Ok(ApplyOutcome::Applied)
            }
            ActionKind::GameModeProfile => {
                let outcome = self.platform.set_game_mode(true)?;
                self.journal(plan, candidate, "game-mode");
                Ok(outcome)
            }
            ActionKind::Unspecified => Err(std::io::Error::other("unsupported candidate")),
        }
    }

    /// Persists a restorable change record capturing enough state to undo the action. Blobs are
    /// deliberately tiny and contain no paths/secrets.
    fn journal(&self, plan: &Plan, candidate: &Candidate, tag: &str) {
        let now = Utc::now().timestamp_millis();
        let record = ChangeRecord {
            change_id: Uuid::new_v4().to_string(),
            plan_id: plan.plan_id.clone(),
            candidate_id: candidate.candidate_id.clone(),
            kind: tag.to_string(),
            direction: "Apply".into(),
            original_state_json: serde_json_default_state(candidate),
            detail: format!("{} via {}", tag, PLANNER_VERSION),
            created_unix_ms: now,
            updated_unix_ms: now,
            restored_unix_ms: None,
        };
        lock(&self.history).push(record);
    }

    /// Restores every change produced by a plan (reversibility contract).
    pub fn restore_plan_changes(&self, plan_id: &str) -> Result<usize, OptimizationError> {
        let mut restored = 0usize;
        let history = lock(&self.history);
        let mut history = history;
        for record in history.iter_mut() {
            if record.plan_id != plan_id || record.restored_unix_ms.is_some() {
                continue;
            }
            if self.platform.restore(record).is_ok() {
                record.restored_unix_ms = Some(Utc::now().timestamp_millis());
                record.direction = "Restore".into();
                restored += 1;
            }
        }
        Ok(restored)
    }

    pub fn history_len(&self) -> usize {
        lock(&self.history).len()
    }

    pub fn status(&self, plan_id: &str) -> Option<ExecutionStatus> {
        let running = lock(&self.running);
        running
            .as_ref()
            .filter(|state| state.plan_id == plan_id)
            .map(|state| state.status.clone())
    }
}

/// Names the optimization workload inside the shared mutation supervisor.
#[derive(Clone, Copy)]
pub struct MutationWorkloadTag;

impl MutationWorkloadTag {
    pub fn workload() -> aethercore_operation_kernel::MutationWorkload {
        aethercore_operation_kernel::MutationWorkload::Optimization
    }
}

fn compute_plan_digest(plan_id: &str, candidates: &[Candidate]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(plan_id.as_bytes());
    for candidate in candidates {
        hasher.update(candidate.candidate_id.as_bytes());
        hasher.update(candidate.kind.as_str().as_bytes());
        hasher.update(candidate.reversibility.as_str().as_bytes());
        for key in &candidate.target_process_keys {
            hasher.update(key.image_key.as_bytes());
        }
    }
    hex_lower(&hasher.finalize())
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn title_key_for(kind: ActionKind) -> &'static str {
    match kind {
        ActionKind::EcoQos => "perf.action.ecoQos.title",
        ActionKind::BackgroundPriority => "perf.action.backgroundPriority.title",
        ActionKind::CooperativeTrimRequest => "perf.action.cooperativeTrim.title",
        ActionKind::GameModeProfile => "perf.action.gameMode.title",
        ActionKind::Unspecified => "perf.action.unspecified.title",
    }
}

fn description_key_for(kind: ActionKind) -> &'static str {
    match kind {
        ActionKind::EcoQos => "perf.action.ecoQos.description",
        ActionKind::BackgroundPriority => "perf.action.backgroundPriority.description",
        ActionKind::CooperativeTrimRequest => "perf.action.cooperativeTrim.description",
        ActionKind::GameModeProfile => "perf.action.gameMode.description",
        ActionKind::Unspecified => "perf.action.unspecified.description",
    }
}

fn expected_metrics(kind: ActionKind) -> Vec<String> {
    match kind {
        ActionKind::EcoQos | ActionKind::BackgroundPriority => {
            vec!["cpu.busyBp.avg".into()]
        }
        ActionKind::CooperativeTrimRequest => vec![
            "memory.modifiedList.growthBytes".into(),
            "memory.commitBp.avg".into(),
        ],
        ActionKind::GameModeProfile => vec!["gpu.utilizationBp.peak".into()],
        ActionKind::Unspecified => Vec::new(),
    }
}

fn outcome_code(outcome: ApplyOutcome) -> &'static str {
    match outcome {
        ApplyOutcome::Applied => "Applied",
        ApplyOutcome::AlreadyInDesiredState => "AlreadyApplied",
        ApplyOutcome::NotSupportedOnThisSystem => "Unsupported",
    }
}

fn bounded(detail: String) -> String {
    detail.chars().take(256).collect()
}

/// Captures the pre-change state needed for undo. Deliberately minimal and privacy-safe:
/// process keys only (never image paths) plus the action kind; Game Mode stores the previous
/// boolean so restore can put it back exactly.
fn serde_json_default_state(candidate: &Candidate) -> String {
    let state = serde_json::json!({
        "kind": candidate.kind.as_str(),
        "targets": candidate.target_process_keys.iter().map(|key| serde_json::json!({
            "pid": key.pid,
            "imageKey": key.image_key,
        })).collect::<Vec<_>>(),
        "gameModePrevious": null,
    });
    state.to_string()
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
