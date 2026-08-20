#![deny(unsafe_op_in_unsafe_fn)]

use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Arc, Mutex, RwLock},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use aethercore_collector_runtime::{CancellationToken, CommitFence};
use aethercore_operation_engine::{
    now_ms, CleanupDeleteAction, CleanupFileEvidence, OperationEngine, PlanState, PlanView,
};
use aethercore_operation_kernel::{MutationLease, MutationWorkload, ProgressTelemetry, ProgressTelemetryStore, ReadBudgetLease, ReadWorkload};
use aethercore_persistence::{
    Database, MaintenanceExecutionRecord, MaintenanceItemRecord, RecoveryRecord,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

const MAX_FILES_PER_CANDIDATE: usize = 5_000;
const MAX_PLAN_FILES: usize = 20_000;

#[derive(Debug, Error)]
pub enum CleanerError {
    #[error("cleanup is only available on Windows")]
    UnsupportedPlatform,
    #[error("cleanup scan is already running")]
    Busy,
    #[error("cleanup inventory was cancelled before publication")]
    Cancelled,
    #[error("cleanup scan is not ready")]
    ScanNotReady,
    #[error("cleanup scan is stale")]
    StaleScan,
    #[error("cleanup scan belongs to a different Windows principal")]
    OwnershipMismatch,
    #[error("cleanup candidate is unknown or no longer selectable: {0}")]
    CandidateInvalid(String),
    #[error("cleanup selection exceeds the safety cap")]
    TooManyFiles,
    #[error("cleanup plan is not authorized")]
    AuthorizationRequired,
    #[error("cleanup execution is already running")]
    AlreadyRunning,
    #[error("another AetherCore maintenance mutation is active")]
    MutationBusy,
    #[error("cleanup safety validation failed: {0}")]
    Safety(String),
    #[error("cleanup I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("operation engine: {0}")]
    Engine(#[from] aethercore_operation_engine::EngineError),
    #[error("persistence: {0}")]
    Persistence(#[from] aethercore_persistence::PersistenceError),
}

pub type Result<T> = std::result::Result<T, CleanerError>;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CleanupScanState {
    Idle,
    Scanning,
    Ready,
    Failed,
}

impl CleanupScanState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Scanning => "Scanning",
            Self::Ready => "Ready",
            Self::Failed => "Failed",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupCandidate {
    pub candidate_id: String,
    pub provider: String,
    pub title: String,
    pub description: String,
    pub reclaimable_bytes: u64,
    pub file_count: u32,
    pub selected_by_default: bool,
    pub requires_explicit_confirmation: bool,
    pub truncated: bool,
    pub special_kind: String,
    #[serde(skip)]
    pub files: Vec<CleanupFileEvidence>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupSnapshot {
    pub scan_id: String,
    pub state: CleanupScanState,
    pub inventory_epoch: u64,
    pub started_unix_ms: i64,
    pub completed_unix_ms: i64,
    pub error_message: String,
    pub total_reclaimable_bytes: u64,
    pub total_file_count: u32,
    pub candidates: Vec<CleanupCandidate>,
    pub warnings: Vec<String>,
}

impl Default for CleanupSnapshot {
    fn default() -> Self {
        Self {
            scan_id: String::new(),
            state: CleanupScanState::Idle,
            inventory_epoch: 0,
            started_unix_ms: 0,
            completed_unix_ms: 0,
            error_message: String::new(),
            total_reclaimable_bytes: 0,
            total_file_count: 0,
            candidates: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupItemStatus {
    pub item_id: String,
    pub title: String,
    pub stage: String,
    pub result_code: String,
    pub bytes_affected: u64,
    pub detail: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupExecutionStatus {
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
    pub reclaimed_bytes: u64,
    pub skipped_bytes: u64,
    pub started_unix_ms: i64,
    pub updated_unix_ms: i64,
    pub completed_unix_ms: i64,
    pub items: Vec<CleanupItemStatus>,
}

pub trait CleanupPlatform: Send + Sync + 'static {
    fn scan(&self) -> Result<Vec<CleanupCandidate>>;
    /// Conservative inventory used only by the autonomous scheduler. Implementations may narrow
    /// scope further than the interactive scan to avoid reading profile-specific data.
    fn scan_passive(&self) -> Result<Vec<CleanupCandidate>> {
        self.scan().map(|items| items.into_iter().filter(|item| !item.requires_explicit_confirmation).collect())
    }
    fn delete_action(&self, action: &CleanupDeleteAction) -> Result<(u64, u64, String)>;
}

#[cfg(windows)]
mod windows_impl;
#[cfg(windows)]
use windows_impl::WindowsCleanupPlatform;

#[cfg(not(windows))]
struct WindowsCleanupPlatform;

#[cfg(not(windows))]
impl CleanupPlatform for WindowsCleanupPlatform {
    fn scan(&self) -> Result<Vec<CleanupCandidate>> {
        Err(CleanerError::UnsupportedPlatform)
    }

    fn delete_action(&self, _action: &CleanupDeleteAction) -> Result<(u64, u64, String)> {
        Err(CleanerError::UnsupportedPlatform)
    }
}

#[cfg(windows)]
fn acquire_cleanup_mutation_guard() -> Result<windows_impl::CleanupMutationGuard> {
    windows_impl::acquire_mutation_guard()
}

#[cfg(not(windows))]
struct NoopCleanupMutationGuard;

#[cfg(not(windows))]
fn acquire_cleanup_mutation_guard() -> Result<NoopCleanupMutationGuard> {
    Ok(NoopCleanupMutationGuard)
}

pub struct CleanupEngine {
    engine: Arc<OperationEngine>,
    db: Arc<Database>,
    platform: Arc<dyn CleanupPlatform>,
    snapshot: Arc<RwLock<CleanupSnapshot>>,
    snapshot_owner: Arc<RwLock<String>>,
    running: Arc<Mutex<Option<String>>>,
    telemetry: ProgressTelemetryStore,
}

impl CleanupEngine {
    pub fn new(engine: Arc<OperationEngine>, db: Arc<Database>) -> Self {
        Self::with_platform_and_telemetry(
            engine,
            db,
            Arc::new(WindowsCleanupPlatform),
            ProgressTelemetryStore::new(),
        )
    }

    pub fn with_platform(
        engine: Arc<OperationEngine>,
        db: Arc<Database>,
        platform: Arc<dyn CleanupPlatform>,
    ) -> Self {
        Self::with_platform_and_telemetry(
            engine,
            db,
            platform,
            ProgressTelemetryStore::new(),
        )
    }

    pub fn with_telemetry(
        engine: Arc<OperationEngine>,
        db: Arc<Database>,
        telemetry: ProgressTelemetryStore,
    ) -> Self {
        Self::with_platform_and_telemetry(engine, db, Arc::new(WindowsCleanupPlatform), telemetry)
    }

    pub fn with_platform_and_telemetry(
        engine: Arc<OperationEngine>,
        db: Arc<Database>,
        platform: Arc<dyn CleanupPlatform>,
        telemetry: ProgressTelemetryStore,
    ) -> Self {
        Self {
            engine,
            db,
            platform,
            snapshot: Arc::new(RwLock::new(CleanupSnapshot::default())),
            snapshot_owner: Arc::new(RwLock::new(String::new())),
            running: Arc::new(Mutex::new(None)),
            telemetry,
        }
    }

    pub fn passive_scan(&self, owner_principal_key:&str, token:CancellationToken)->Result<CleanupSnapshot>{
        self.passive_scan_with_fence(owner_principal_key, token, CommitFence::new())
    }

    pub fn passive_scan_with_fence(&self, owner_principal_key:&str, token:CancellationToken, commit_fence:CommitFence)->Result<CleanupSnapshot>{
        if owner_principal_key.trim().is_empty() || token.is_cancelled(){return Err(CleanerError::Cancelled)}
        let started=now_ms(); let scan_id=Uuid::new_v4().to_string();
        let epoch = { let current=self.snapshot.read().map_err(|_|CleanerError::Busy)?; if current.state==CleanupScanState::Scanning{return Err(CleanerError::Busy)} current.inventory_epoch.saturating_add(1).max(started.max(0) as u64) };
        let candidates=self.platform.scan_passive()?; if token.is_cancelled(){return Err(CleanerError::Cancelled)}
        let total_reclaimable_bytes=candidates.iter().map(|c|c.reclaimable_bytes).sum();
        let total_file_count=candidates.iter().map(|c|c.file_count).sum();
        let ready=CleanupSnapshot{scan_id,state:CleanupScanState::Ready,inventory_epoch:epoch,started_unix_ms:started,completed_unix_ms:now_ms(),error_message:String::new(),total_reclaimable_bytes,total_file_count,candidates,warnings:Vec::new()};
        let committed=commit_fence.try_commit_checked(||{
            let Ok(mut owner)=self.snapshot_owner.write() else{return None};
            let Ok(mut current)=self.snapshot.write() else{return None};
            if current.state==CleanupScanState::Scanning||token.is_cancelled(){return None}
            *owner=owner_principal_key.to_owned();*current=ready.clone();Some(())
        });
        if committed!=Some(()){return Err(CleanerError::Cancelled)}
        Ok(ready)
    }

    pub fn start_scan_with_lease(
        &self,
        owner_principal_key: &str,
        lease: ReadBudgetLease,
    ) -> Result<CleanupSnapshot> {
        if !lease.matches(ReadWorkload::CleanupDiscovery) {
            return Err(CleanerError::Safety("read budget lease identity mismatch".into()));
        }
        self.start_scan_inner(owner_principal_key, lease)
    }

    fn start_scan_inner(
        &self,
        owner_principal_key: &str,
        read_budget_lease: ReadBudgetLease,
    ) -> Result<CleanupSnapshot> {
        let scan_id = Uuid::new_v4().to_string();
        let started = now_ms();
        let initial = {
            let mut owner = self.snapshot_owner.write().map_err(|_| CleanerError::Busy)?;
            let mut current = self.snapshot.write().map_err(|_| CleanerError::Busy)?;
            if current.state == CleanupScanState::Scanning { return Err(CleanerError::Busy); }
            let epoch = current.inventory_epoch.saturating_add(1).max(started.max(0) as u64);
            let initial = CleanupSnapshot { scan_id:scan_id.clone(), state:CleanupScanState::Scanning, inventory_epoch:epoch, started_unix_ms:started, ..Default::default() };
            *owner = owner_principal_key.to_owned(); *current = initial.clone(); initial
        };
        let epoch = initial.inventory_epoch;

        let state = self.snapshot.clone();
        let platform = self.platform.clone();
        if let Err(error) = thread::Builder::new()
            .name("aether-cleanup-discovery".into())
            .spawn(move || {
                let _read_budget_lease = read_budget_lease;
                let completed = now_ms();
                let next = match platform.scan() {
                    Ok(candidates) => {
                        let total_reclaimable_bytes =
                            candidates.iter().map(|candidate| candidate.reclaimable_bytes).sum();
                        let total_file_count =
                            candidates.iter().map(|candidate| candidate.file_count).sum();
                        CleanupSnapshot {
                            scan_id,
                            state: CleanupScanState::Ready,
                            inventory_epoch: epoch,
                            started_unix_ms: started,
                            completed_unix_ms: completed,
                            error_message: String::new(),
                            total_reclaimable_bytes,
                            total_file_count,
                            candidates,
                            warnings: Vec::new(),
                        }
                    }
                    Err(error) => CleanupSnapshot {
                        scan_id,
                        state: CleanupScanState::Failed,
                        inventory_epoch: epoch,
                        started_unix_ms: started,
                        completed_unix_ms: completed,
                        error_message: error.to_string(),
                        ..Default::default()
                    },
                };
                if let Ok(mut guard) = state.write() {
                    *guard = next;
                }
            })
        {
            let detail = format!("cleanup discovery worker unavailable: {error}");
            if let Ok(mut current) = self.snapshot.write() {
                current.state = CleanupScanState::Failed;
                current.completed_unix_ms = now_ms();
                current.error_message = detail.clone();
            }
            return Err(CleanerError::Safety(detail));
        }

        Ok(initial)
    }

    fn snapshot(&self) -> CleanupSnapshot {
        self.snapshot
            .read()
            .map(|snapshot| snapshot.clone())
            .unwrap_or_default()
    }

    pub fn snapshot_for_owner(&self, owner_principal_key: &str) -> Result<CleanupSnapshot> {
        let owner = self.snapshot_owner.read().map_err(|_| CleanerError::Busy)?;
        if owner.as_str() != owner_principal_key {
            return Err(CleanerError::OwnershipMismatch);
        }
        Ok(self.snapshot())
    }

    pub fn create_plan(
        &self,
        owner_principal_key: &str,
        scan_id: &str,
        inventory_epoch: u64,
        candidate_ids: &[String],
    ) -> Result<PlanView> {
        let snapshot = self.snapshot_for_owner(owner_principal_key)?;
        if snapshot.state != CleanupScanState::Ready {
            return Err(CleanerError::ScanNotReady);
        }
        if snapshot.scan_id != scan_id || snapshot.inventory_epoch != inventory_epoch {
            return Err(CleanerError::StaleScan);
        }
        if candidate_ids.is_empty() {
            return Err(CleanerError::CandidateInvalid("empty selection".into()));
        }

        let mut unique = HashSet::new();
        let mut actions = Vec::new();
        let mut total_files = 0usize;
        for id in candidate_ids {
            if !unique.insert(id) {
                return Err(CleanerError::CandidateInvalid(format!(
                    "duplicate candidate: {id}"
                )));
            }
            let candidate = snapshot
                .candidates
                .iter()
                .find(|candidate| &candidate.candidate_id == id)
                .ok_or_else(|| CleanerError::CandidateInvalid(id.clone()))?;
            total_files += candidate.files.len();
            if total_files > MAX_PLAN_FILES {
                return Err(CleanerError::TooManyFiles);
            }
            actions.push(CleanupDeleteAction {
                candidate_id: candidate.candidate_id.clone(),
                scan_id: scan_id.into(),
                inventory_epoch,
                provider: candidate.provider.clone(),
                title: candidate.title.clone(),
                special_kind: candidate.special_kind.clone(),
                files: candidate.files.clone(),
                expected_bytes: candidate.reclaimable_bytes,
            });
        }

        self.engine
            .create_cleanup_plan(owner_principal_key, inventory_epoch, scan_id, actions)
            .map_err(Into::into)
    }

    pub fn start_with_lease(
        &self,
        owner_principal_key: &str,
        plan_id: &str,
        lease: MutationLease,
    ) -> Result<CleanupExecutionStatus> {
        if !lease.matches(MutationWorkload::Cleanup, plan_id, owner_principal_key) {
            return Err(CleanerError::Safety("mutation lease identity mismatch".into()));
        }
        self.start_inner(owner_principal_key, plan_id, lease)
    }

    fn start_inner(
        &self,
        owner_principal_key: &str,
        plan_id: &str,
        mutation_lease: MutationLease,
    ) -> Result<CleanupExecutionStatus> {
        let plan = self.engine.get_plan_for_owner(plan_id, owner_principal_key)?;
        if plan.state != PlanState::AwaitingAuthorization {
            return Err(CleanerError::AuthorizationRequired);
        }

        let mut running = self.running.lock().map_err(|_| CleanerError::AlreadyRunning)?;
        if running.is_some() {
            return Err(CleanerError::AlreadyRunning);
        }
        *running = Some(plan_id.into());
        drop(running);

        if let Err(error) = self.engine.consume_authorization_and_begin(
            plan_id, owner_principal_key, "one-shot consent consumed; cleanup entered preflight",
        ) {
            if let Ok(mut running) = self.running.lock() { *running = None; }
            return Err(error.into());
        }

        let now = now_ms();
        if let Err(error) = self.db
            .upsert_maintenance_execution(&MaintenanceExecutionRecord {
                plan_id: plan_id.into(),
                domain: "Cleanup".into(),
                stage: "Queued".into(),
                detail: "Reviewed cleanup queued".into(),
                started_unix_ms: now,
                updated_unix_ms: now,
                ..Default::default()
            })
        {
            let _ = self.engine.transition(
                plan_id,
                PlanState::Preflight,
                PlanState::Failed,
                "cleanup execution journal initialization failed after consent",
            );
            if let Ok(mut running) = self.running.lock() { *running = None; }
            return Err(error.into());
        }

        let engine = self.engine.clone();
        let db = self.db.clone();
        let platform = self.platform.clone();
        let running = self.running.clone();
        let telemetry = self.telemetry.clone();
        let owner = owner_principal_key.to_owned();
        let id = plan_id.to_string();
        let spawn = thread::Builder::new()
            .name("aether-cleanup-worker".into())
            .spawn(move || {
                let _mutation_lease = mutation_lease;
                if let Err(error) = run_cleanup(&engine, &db, platform.as_ref(), &owner, &telemetry, &id) {
                    let _ = fail_cleanup(&engine, &db, &id, error);
                }
                telemetry.clear_for_owner(&owner, &id);
                if let Ok(mut guard) = running.lock() {
                    *guard = None;
                }
            });
        if let Err(error) = spawn {
            let detail = format!("cleanup worker creation failed: {error}");
            let _ = fail_cleanup(&self.engine, &self.db, plan_id, CleanerError::Safety(detail.clone()));
            self.telemetry.clear_for_owner(owner_principal_key, plan_id);
            if let Ok(mut running) = self.running.lock() { *running = None; }
            return Err(CleanerError::Safety(detail));
        }

        self.status(owner_principal_key, Some(plan_id))?
            .ok_or_else(|| CleanerError::Safety("cleanup status missing".into()))
    }

    pub fn status(&self, owner_principal_key: &str, plan_id: Option<&str>) -> Result<Option<CleanupExecutionStatus>> {
        let record = match plan_id {
            Some(id) => {
                self.engine.get_plan_for_owner(id, owner_principal_key)?;
                self.db.get_maintenance_execution(id)?
            }
            None => self.db.latest_maintenance_execution_for_owner("Cleanup", owner_principal_key)?,
        };
        let Some(record) = record else {
            return Ok(None);
        };

        let plan = self.engine.get_plan_for_owner(&record.plan_id, owner_principal_key)?;
        let actions = self.engine.cleanup_actions(&record.plan_id)?;
        let expected: HashMap<&str, u64> = actions
            .iter()
            .map(|action| (action.candidate_id.as_str(), action.expected_bytes))
            .collect();
        let raw = self.db.maintenance_items(&record.plan_id)?;

        let mut reclaimed_bytes = 0u64;
        let mut skipped_bytes = 0u64;
        let mut items = Vec::with_capacity(raw.len());
        for item in raw {
            reclaimed_bytes = reclaimed_bytes.saturating_add(item.bytes_affected);
            if item.result_code != "Deleted" {
                let expected_bytes = expected.get(item.item_id.as_str()).copied().unwrap_or(0);
                skipped_bytes = skipped_bytes
                    .saturating_add(expected_bytes.saturating_sub(item.bytes_affected));
            }
            items.push(CleanupItemStatus {
                item_id: item.item_id,
                title: item.kind,
                stage: item.stage,
                result_code: item.result_code,
                bytes_affected: item.bytes_affected,
                detail: item.detail,
            });
        }

        let live = self
            .telemetry
            .get(&record.plan_id)
            .filter(|value| value.owner_principal_key == owner_principal_key && value.emitted_unix_ms >= record.updated_unix_ms);
        Ok(Some(CleanupExecutionStatus {
            plan_id: record.plan_id,
            plan_state: plan.state.as_str().into(),
            stage: live.as_ref().map(|value| value.stage.clone()).unwrap_or(record.stage),
            progress_known: live.as_ref().map(|value| value.progress_known).unwrap_or(record.progress_known),
            overall_percent: live.as_ref().map(|value| value.overall_percent).unwrap_or(record.overall_percent),
            current_candidate_id: live.as_ref().map(|value| value.current_item_id.clone()).unwrap_or(record.current_item_id),
            detail: live.as_ref().map(|value| value.detail.clone()).unwrap_or(record.detail),
            mutation_started: record.mutation_started,
            recovery_required: record.recovery_required,
            failure_message: record.failure_message,
            reclaimed_bytes,
            skipped_bytes,
            started_unix_ms: record.started_unix_ms,
            updated_unix_ms: live.as_ref().map(|value| value.emitted_unix_ms).unwrap_or(record.updated_unix_ms),
            completed_unix_ms: record.completed_unix_ms.unwrap_or(0),
            items,
        }))
    }

    pub fn recover_incomplete(&self) -> Result<()> {
        for plan in self.engine.recoverable_plans()? {
            if self.engine.cleanup_actions(&plan.id).is_err() {
                continue;
            }

            let mutated = matches!(plan.state, PlanState::Executing | PlanState::Verifying);
            let now = now_ms();
            let _ = self.engine.transition(
                &plan.id,
                plan.state,
                PlanState::Failed,
                "cleanup interrupted; immutable targets will not be replayed automatically",
            );
            self.db
                .upsert_maintenance_execution(&MaintenanceExecutionRecord {
                    plan_id: plan.id.clone(),
                    domain: "Cleanup".into(),
                    stage: "Interrupted".into(),
                    detail: if mutated {
                        "Cleanup stopped after deletion began. No deletion was replayed automatically."
                            .into()
                    } else {
                        "Cleanup stopped before deletion began. No files were changed.".into()
                    },
                    mutation_started: mutated,
                    recovery_required: mutated,
                    failure_message: "Service restart interrupted cleanup".into(),
                    started_unix_ms: plan.created_unix_ms,
                    updated_unix_ms: now,
                    completed_unix_ms: Some(now),
                    ..Default::default()
                })?;

            if mutated {
                self.db.add_recovery_record(&RecoveryRecord {
                    plan_id: plan.id,
                    severity: "Amber".into(),
                    kind: "CleanupInterrupted".into(),
                    summary: "Cleanup was interrupted after deletion began".into(),
                    detail: "AetherCore did not replay deletion after restart. Run a fresh cleanup scan before any further action."
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
        progress_known: true,
        overall_percent: percent,
        current_item_id: current_item_id.into(),
        detail: detail.into(),
        ..Default::default()
    });
}

fn run_cleanup(
    engine: &OperationEngine,
    db: &Database,
    platform: &dyn CleanupPlatform,
    owner_principal_key: &str,
    telemetry: &ProgressTelemetryStore,
    plan_id: &str,
) -> Result<()> {
    let actions = engine.cleanup_actions(plan_id)?;
    publish_progress(telemetry, owner_principal_key, plan_id, "Preflight", 5, "", "Revalidating allowlisted cleanup targets");
    update(
        db,
        plan_id,
        "Preflight",
        5,
        "Revalidating allowlisted cleanup targets",
        false,
        None,
    )?;

    let _mutation_guard = match acquire_cleanup_mutation_guard() {
        Ok(guard) => guard,
        Err(error) => return fail_cleanup(engine, db, plan_id, error),
    };

    engine.transition(
        plan_id,
        PlanState::Preflight,
        PlanState::Protected,
        "cleanup provider allowlist and immutable target set committed",
    )?;
    engine.transition(
        plan_id,
        PlanState::Protected,
        PlanState::Executing,
        "begin reviewed cleanup deletion",
    )?;
    publish_progress(telemetry, owner_principal_key, plan_id, "Executing", 10, "", "Deleting reviewed candidates with final-path validation");
    update(
        db,
        plan_id,
        "Executing",
        10,
        "Deleting reviewed candidates with final-path validation",
        true,
        None,
    )?;

    let count = actions.len().max(1);
    for (index, action) in actions.iter().enumerate() {
        let percent = 10 + (((index as f32 / count as f32) * 80.0) as u32);
        publish_progress(telemetry, owner_principal_key, plan_id, "Executing", percent, &action.candidate_id, &action.title);
        update_current(db, plan_id, &action.candidate_id, percent, &action.title)?;
        match platform.delete_action(action) {
            Ok((deleted, skipped, detail)) => {
                self_record_cleanup_item(db, plan_id, action, deleted, skipped, detail)?;
            }
            Err(error) => return fail_cleanup(engine, db, plan_id, error),
        }
    }

    engine.transition(
        plan_id,
        PlanState::Executing,
        PlanState::Verifying,
        "cleanup deletion completed; verify selected targets",
    )?;
    publish_progress(telemetry, owner_principal_key, plan_id, "Verifying", 95, "", "Verifying cleanup journal and reclaimed totals");
    update(
        db,
        plan_id,
        "Verifying",
        95,
        "Verifying cleanup journal and reclaimed totals",
        true,
        None,
    )?;
    engine.transition(
        plan_id,
        PlanState::Verifying,
        PlanState::Completed,
        "allowlisted cleanup completed",
    )?;
    let now = now_ms();
    publish_progress(telemetry, owner_principal_key, plan_id, "Completed", 100, "", "Reviewed cleanup completed. Locked or changed files were left untouched.");
    update(
        db,
        plan_id,
        "Completed",
        100,
        "Reviewed cleanup completed. Locked or changed files were left untouched.",
        true,
        Some(now),
    )?;
    Ok(())
}

fn self_record_cleanup_item(
    db: &Database,
    plan_id: &str,
    action: &CleanupDeleteAction,
    deleted: u64,
    skipped: u64,
    detail: String,
) -> Result<()> {
    db.upsert_maintenance_item(&MaintenanceItemRecord {
        plan_id: plan_id.into(),
        item_id: action.candidate_id.clone(),
        kind: action.title.clone(),
        stage: "Completed".into(),
        result_code: if skipped == 0 {
            "Deleted".into()
        } else {
            "Partial".into()
        },
        bytes_affected: deleted,
        detail: format!(
            "{detail}; skipped {skipped} bytes that changed, were locked, or failed validation"
        ),
        updated_unix_ms: now_ms(),
    })?;
    Ok(())
}

fn fail_cleanup(
    engine: &OperationEngine,
    db: &Database,
    plan_id: &str,
    error: CleanerError,
) -> Result<()> {
    let current = engine.get_plan(plan_id)?;
    if !current.state.is_terminal() {
        let _ = engine.transition(
            plan_id,
            current.state,
            PlanState::Failed,
            "cleanup execution failed",
        );
    }
    let now = now_ms();
    let mut record = db.get_maintenance_execution(plan_id)?.unwrap_or_default();
    record.plan_id = plan_id.into();
    record.domain = "Cleanup".into();
    record.stage = "Failed".into();
    record.failure_message = error.to_string();
    record.detail = "Cleanup stopped; no targets will be replayed automatically.".into();
    record.recovery_required = record.mutation_started;
    record.updated_unix_ms = now;
    record.completed_unix_ms = Some(now);
    db.upsert_maintenance_execution(&record)?;
    Err(error)
}

fn update(
    db: &Database,
    plan_id: &str,
    stage: &str,
    percent: u32,
    detail: &str,
    mutation: bool,
    completed: Option<i64>,
) -> Result<()> {
    let now = now_ms();
    let mut record = db
        .get_maintenance_execution(plan_id)?
        .unwrap_or_else(|| MaintenanceExecutionRecord {
            plan_id: plan_id.into(),
            domain: "Cleanup".into(),
            started_unix_ms: now,
            ..Default::default()
        });
    record.stage = stage.into();
    record.progress_known = true;
    record.overall_percent = percent;
    record.detail = detail.into();
    record.mutation_started |= mutation;
    record.updated_unix_ms = now;
    if completed.is_some() {
        record.completed_unix_ms = completed;
    }
    db.upsert_maintenance_execution(&record)?;
    Ok(())
}

fn update_current(
    db: &Database,
    plan_id: &str,
    candidate_id: &str,
    percent: u32,
    detail: &str,
) -> Result<()> {
    let mut record = db.get_maintenance_execution(plan_id)?.unwrap_or_default();
    record.current_item_id = candidate_id.into();
    record.overall_percent = percent;
    record.detail = detail.into();
    record.updated_unix_ms = now_ms();
    db.upsert_maintenance_execution(&record)?;
    Ok(())
}

pub(crate) fn evidence(
    path: &Path,
    root: &Path,
    root_final_path: &Path,
    size_bytes: u64,
    modified_unix_ms: i64,
    volume_serial_number: u64,
    file_id_128: String,
) -> CleanupFileEvidence {
    CleanupFileEvidence {
        path: path.to_string_lossy().into_owned(),
        root: root.to_string_lossy().into_owned(),
        root_final_path: root_final_path.to_string_lossy().into_owned(),
        size_bytes,
        modified_unix_ms,
        volume_serial_number,
        file_id_128,
    }
}

pub(crate) fn older_than(metadata: &std::fs::Metadata, age: Duration) -> bool {
    metadata
        .modified()
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .map(|elapsed| elapsed >= age)
        .unwrap_or(false)
}

pub(crate) fn candidate(
    provider: &str,
    title: &str,
    description: &str,
    files: Vec<CleanupFileEvidence>,
    selected: bool,
    explicit: bool,
    special_kind: &str,
    truncated: bool,
) -> CleanupCandidate {
    let reclaimable_bytes = files.iter().map(|file| file.size_bytes).sum();
    CleanupCandidate {
        candidate_id: Uuid::new_v4().to_string(),
        provider: provider.into(),
        title: title.into(),
        description: description.into(),
        reclaimable_bytes,
        file_count: files.len().try_into().unwrap_or(u32::MAX),
        selected_by_default: selected,
        requires_explicit_confirmation: explicit,
        truncated,
        special_kind: special_kind.into(),
        files,
    }
}

pub(crate) fn cap_files(
    mut files: Vec<CleanupFileEvidence>,
) -> (Vec<CleanupFileEvidence>, bool) {
    if files.len() > MAX_FILES_PER_CANDIDATE {
        files.sort_by_key(|file| file.modified_unix_ms);
        files.truncate(MAX_FILES_PER_CANDIDATE);
        (files, true)
    } else {
        (files, false)
    }
}
