#![deny(unsafe_op_in_unsafe_fn)]

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use aethercore_driver_backup::{BackupEvidence, candidate_backup_root, plan_backup_root};
use aethercore_driver_hub::{DriverHub, InstallSelection};
use aethercore_operation_engine::{
    DriverEvidence, DriverInstallAction, OperationEngine, PlanState, PlanView,
};
use aethercore_operation_kernel::{
    MutationLease, MutationWorkload, ProgressTelemetry, ProgressTelemetryStore,
};
use aethercore_persistence::{Database, ExecutionRecord, InstallItemRecord, RecoveryRecord};
use aethercore_restore_point::RestorePointEvidence;
use aethercore_windows_pnp::{DeviceVerification, InstalledDriver, normalize_pnp_id};
use aethercore_windows_update::{ExecutionStage, UpdateIdentity, WuaExecutionResult, WuaProgress};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InstallError {
    #[error("driver installation is already active")]
    Busy,
    #[error("authorization is required")]
    AuthorizationRequired,
    #[error("plan is not a driver installation plan")]
    InvalidPlan,
    #[error("preflight rejected: {0}")]
    Preflight(String),
    #[error("system protection failed: {0}")]
    Protection(String),
    #[error("Windows Update execution failed: {0}")]
    Execution(String),
    #[error("post-install verification failed: {0}")]
    Verification(String),
    #[error("operation engine: {0}")]
    Engine(#[from] aethercore_operation_engine::EngineError),
    #[error("database: {0}")]
    Database(#[from] aethercore_persistence::PersistenceError),
    #[error("driver hub: {0}")]
    Hub(#[from] aethercore_driver_hub::HubError),
}

pub type Result<T> = std::result::Result<T, InstallError>;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstallStatus {
    pub plan_id: String,
    pub plan_state: String,
    pub stage: String,
    pub progress_known: bool,
    pub overall_percent: u32,
    pub current_candidate_id: String,
    /// DBT-P46-B16: None means no progress tick ever determined a figure;
    /// Some(0) means a tick determined that nothing had transferred yet.
    pub bytes_downloaded: Option<u64>,
    /// DBT-P46-B16: None means the total size is unknown — a different fact
    /// from a zero-length download.
    pub bytes_total: Option<u64>,
    pub detail: String,
    pub reboot_required: bool,
    pub restore_point_verified: bool,
    pub restore_point_sequence: i64,
    pub backup_root: String,
    pub mutation_started: bool,
    pub recovery_required: bool,
    pub failure_message: String,
    pub started_unix_ms: i64,
    pub updated_unix_ms: i64,
    /// DBT-P46-B11: None while the install has not completed. See the same
    /// field on system-repair's RepairExecutionStatus — one shape, four crates.
    pub completed_unix_ms: Option<i64>,
    pub items: Vec<InstallItemStatus>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstallItemStatus {
    pub candidate_id: String,
    pub instance_id: String,
    pub title: String,
    pub stage: String,
    pub progress_known: bool,
    pub progress_percent: u32,
    pub result_code: String,
    pub hresult: i32,
    pub reboot_required: bool,
    pub verified: bool,
    pub before_version: String,
    pub after_version: String,
    pub before_problem_code: u32,
    pub after_problem_code: u32,
    pub backup_path: String,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryEntry {
    pub seq: i64,
    pub plan_id: String,
    pub severity: String,
    pub kind: String,
    pub summary: String,
    pub detail: String,
    pub restore_point_sequence: i64,
    pub backup_root: String,
    pub created_unix_ms: i64,
}

pub trait InstallPlatform: Send + Sync + 'static {
    fn boot_marker_ms(&self) -> std::result::Result<i64, String>;
    fn verify_devices(
        &self,
        instance_ids: &[String],
    ) -> std::result::Result<Vec<DeviceVerification>, String>;
    fn begin_restore(&self, plan_id: &str) -> std::result::Result<RestorePointEvidence, String>;
    fn end_restore(&self, sequence: i64, description: &str) -> std::result::Result<(), String>;
    fn cancel_restore(&self, sequence: i64, description: &str) -> std::result::Result<(), String>;
    fn backup_driver(
        &self,
        inf: &str,
        destination: &Path,
    ) -> std::result::Result<BackupEvidence, String>;
    fn execute_wua(
        &self,
        identities: &[UpdateIdentity],
        progress: &mut dyn FnMut(WuaProgress),
        before_install: &mut dyn FnMut() -> std::result::Result<(), String>,
    ) -> std::result::Result<WuaExecutionResult, String>;
}

#[derive(Default)]
pub struct WindowsInstallPlatform;
impl InstallPlatform for WindowsInstallPlatform {
    fn boot_marker_ms(&self) -> std::result::Result<i64, String> {
        platform::boot_marker_ms()
    }
    fn verify_devices(
        &self,
        ids: &[String],
    ) -> std::result::Result<Vec<DeviceVerification>, String> {
        aethercore_windows_pnp::verify_device_instances(ids).map_err(|e| e.to_string())
    }
    fn begin_restore(&self, plan_id: &str) -> std::result::Result<RestorePointEvidence, String> {
        aethercore_restore_point::begin_driver_install(plan_id).map_err(|e| e.to_string())
    }
    fn end_restore(&self, seq: i64, desc: &str) -> std::result::Result<(), String> {
        aethercore_restore_point::end_driver_install(seq, desc).map_err(|e| e.to_string())
    }
    fn cancel_restore(&self, seq: i64, desc: &str) -> std::result::Result<(), String> {
        aethercore_restore_point::cancel_driver_install(seq, desc).map_err(|e| e.to_string())
    }
    fn backup_driver(
        &self,
        inf: &str,
        destination: &Path,
    ) -> std::result::Result<BackupEvidence, String> {
        aethercore_driver_backup::export_driver_package(inf, destination).map_err(|e| e.to_string())
    }
    fn execute_wua(
        &self,
        ids: &[UpdateIdentity],
        progress: &mut dyn FnMut(WuaProgress),
        before: &mut dyn FnMut() -> std::result::Result<(), String>,
    ) -> std::result::Result<WuaExecutionResult, String> {
        aethercore_windows_update::execute_driver_updates(ids, progress, before)
            .map_err(|e| e.to_string())
    }
}

#[derive(Clone)]
pub struct DriverInstallCoordinator {
    engine: Arc<OperationEngine>,
    hub: Arc<DriverHub>,
    db: Arc<Database>,
    platform: Arc<dyn InstallPlatform>,
    product_data_root: PathBuf,
    running: Arc<AtomicBool>,
    telemetry: ProgressTelemetryStore,
}

impl DriverInstallCoordinator {
    pub fn new(
        engine: Arc<OperationEngine>,
        hub: Arc<DriverHub>,
        db: Arc<Database>,
        product_data_root: PathBuf,
    ) -> Self {
        Self::with_platform_and_telemetry(
            engine,
            hub,
            db,
            product_data_root,
            Arc::new(WindowsInstallPlatform),
            ProgressTelemetryStore::new(),
        )
    }

    pub fn with_platform(
        engine: Arc<OperationEngine>,
        hub: Arc<DriverHub>,
        db: Arc<Database>,
        product_data_root: PathBuf,
        platform: Arc<dyn InstallPlatform>,
    ) -> Self {
        Self::with_platform_and_telemetry(
            engine,
            hub,
            db,
            product_data_root,
            platform,
            ProgressTelemetryStore::new(),
        )
    }

    pub fn with_platform_and_telemetry(
        engine: Arc<OperationEngine>,
        hub: Arc<DriverHub>,
        db: Arc<Database>,
        product_data_root: PathBuf,
        platform: Arc<dyn InstallPlatform>,
        telemetry: ProgressTelemetryStore,
    ) -> Self {
        Self {
            engine,
            hub,
            db,
            platform,
            product_data_root,
            running: Arc::new(AtomicBool::new(false)),
            telemetry,
        }
    }

    pub fn create_plan(
        &self,
        owner_principal_key: &str,
        scan_id: &str,
        inventory_epoch: u64,
        candidate_ids: &[String],
    ) -> Result<PlanView> {
        let selection = self.hub.resolve_install_selection(
            owner_principal_key,
            scan_id,
            inventory_epoch,
            candidate_ids,
        )?;
        let actions = selection.into_iter().map(action_from_selection).collect();
        Ok(self.engine.create_driver_install_plan(
            owner_principal_key,
            inventory_epoch,
            scan_id,
            actions,
        )?)
    }

    pub fn start_with_lease(
        &self,
        owner_principal_key: &str,
        plan_id: &str,
        lease: MutationLease,
    ) -> Result<InstallStatus> {
        if !lease.matches(
            MutationWorkload::DriverInstall,
            plan_id,
            owner_principal_key,
        ) {
            return Err(InstallError::InvalidPlan);
        }
        self.start_inner(owner_principal_key, plan_id, lease)
    }

    fn start_inner(
        &self,
        owner_principal_key: &str,
        plan_id: &str,
        mutation_lease: MutationLease,
    ) -> Result<InstallStatus> {
        // Resolve and validate ownership before consuming one-shot authorization. Production IPC
        // additionally hands the machine-wide MutationLease into the execution worker itself, so
        // telemetry/watch-thread failure cannot release mutation authority early.
        let plan = self
            .engine
            .get_plan_for_owner(plan_id, owner_principal_key)?;
        if plan.state != PlanState::AwaitingAuthorization {
            return Err(InstallError::AuthorizationRequired);
        }
        let actions = self.engine.driver_install_actions(plan_id)?;
        if actions.is_empty() {
            return Err(InstallError::InvalidPlan);
        }

        if self.running.swap(true, Ordering::SeqCst) {
            return Err(InstallError::Busy);
        }

        if let Err(error) = self.engine.consume_authorization_and_begin(
            plan_id,
            owner_principal_key,
            "one-shot consent consumed; driver installation entered preflight",
        ) {
            self.running.store(false, Ordering::SeqCst);
            return Err(error.into());
        }

        let now = now_ms();
        if let Err(error) = self.initialize_records(plan_id, &actions, now) {
            let _ = self.fail_safely(plan_id, &error.to_string());
            self.running.store(false, Ordering::SeqCst);
            return Err(error);
        }

        let worker = self.clone();
        let id = plan_id.to_owned();
        let owner = owner_principal_key.to_owned();
        let spawn = thread::Builder::new()
            .name("aether-driver-install-worker".into())
            .spawn(move || {
                let _mutation_lease = mutation_lease;
                if let Err(error) = worker.run_plan(&owner, &id, actions) {
                    let _ = worker.fail_safely(&id, &error.to_string());
                }
                worker.telemetry.clear_for_owner(&owner, &id);
                worker.running.store(false, Ordering::SeqCst);
            });
        if let Err(error) = spawn {
            let detail = format!("driver installation worker creation failed: {error}");
            let _ = self.fail_safely(plan_id, &detail);
            self.telemetry.clear_for_owner(owner_principal_key, plan_id);
            self.running.store(false, Ordering::SeqCst);
            return Err(InstallError::Execution(detail));
        }
        self.status(owner_principal_key, Some(plan_id))?
            .ok_or(InstallError::InvalidPlan)
    }

    pub fn status(
        &self,
        owner_principal_key: &str,
        plan_id: Option<&str>,
    ) -> Result<Option<InstallStatus>> {
        let execution = match plan_id {
            Some(id) => {
                self.engine.get_plan_for_owner(id, owner_principal_key)?;
                self.db.get_execution(id)?
            }
            None => self.db.latest_execution_for_owner(owner_principal_key)?,
        };
        let Some(execution) = execution else {
            return Ok(None);
        };
        let plan = self
            .engine
            .get_plan_for_owner(&execution.plan_id, owner_principal_key)?;
        let items = self
            .db
            .install_items(&execution.plan_id)?
            .into_iter()
            .map(item_status)
            .collect();
        let live = self
            .telemetry
            .get_for_owner(owner_principal_key, &execution.plan_id)
            .filter(|v| v.emitted_unix_ms >= execution.updated_unix_ms);
        Ok(Some(InstallStatus {
            plan_id: execution.plan_id.clone(),
            plan_state: plan.state.as_str().into(),
            stage: live
                .as_ref()
                .map(|v| v.stage.clone())
                .unwrap_or(execution.stage),
            progress_known: live
                .as_ref()
                .map(|v| v.progress_known)
                .unwrap_or(execution.progress_known),
            overall_percent: live
                .as_ref()
                .map(|v| v.overall_percent)
                .unwrap_or(execution.overall_percent),
            current_candidate_id: live
                .as_ref()
                .map(|v| v.current_item_id.clone())
                .unwrap_or(execution.current_candidate_id),
            bytes_downloaded: live
                .as_ref()
                .map(|v| Some(v.bytes_completed))
                .unwrap_or(execution.bytes_downloaded),
            bytes_total: live
                .as_ref()
                .map(|v| Some(v.bytes_total))
                .unwrap_or(execution.bytes_total),
            detail: live
                .as_ref()
                .map(|v| v.detail.clone())
                .unwrap_or(execution.detail),
            reboot_required: execution.reboot_required,
            restore_point_verified: execution.restore_point_sequence.is_some(),
            restore_point_sequence: execution.restore_point_sequence.unwrap_or(0),
            backup_root: execution.backup_root,
            mutation_started: execution.mutation_started,
            recovery_required: execution.recovery_required,
            failure_message: execution.failure_message,
            started_unix_ms: execution.started_unix_ms,
            updated_unix_ms: execution.updated_unix_ms,
            completed_unix_ms: execution.completed_unix_ms,
            items,
        }))
    }

    pub fn recovery_history(
        &self,
        owner_principal_key: &str,
        limit: usize,
    ) -> Result<Vec<RecoveryEntry>> {
        Ok(self
            .db
            .recovery_records_for_owner(owner_principal_key, limit)?
            .into_iter()
            .map(|r| RecoveryEntry {
                seq: r.seq,
                plan_id: r.plan_id,
                severity: r.severity,
                kind: r.kind,
                summary: r.summary,
                detail: r.detail,
                restore_point_sequence: r.restore_point_sequence.unwrap_or(0),
                backup_root: r.backup_root,
                created_unix_ms: r.created_unix_ms,
            })
            .collect())
    }

    /// Called once during service startup. Never repeats a driver mutation. RebootPending is only
    /// resumed after the boot marker proves that Windows actually restarted.
    pub fn recover_incomplete(&self) -> Result<()> {
        let current_boot = self.platform.boot_marker_ms().unwrap_or_default();
        for plan in self.engine.recoverable_plans()? {
            let Some(mut execution) = self.db.get_execution(&plan.id)? else {
                continue;
            };
            match plan.state {
                PlanState::RebootPending
                    if execution.reboot_boot_marker_ms != 0
                        && boot_changed(execution.reboot_boot_marker_ms, current_boot) =>
                {
                    self.engine.transition(
                        &plan.id,
                        PlanState::RebootPending,
                        PlanState::Resuming,
                        "Windows reboot detected; resuming verification only",
                    )?;
                    self.engine.transition(
                        &plan.id,
                        PlanState::Resuming,
                        PlanState::Verifying,
                        "re-interrogating devices after reboot",
                    )?;
                    let actions = self.engine.driver_install_actions(&plan.id)?;
                    self.finish_verification(&plan.id, &actions, &mut execution, true)?;
                }
                PlanState::Executing | PlanState::Verifying => {
                    execution.recovery_required = true;
                    execution.failure_message = "Service restarted after driver mutation; AetherCore will not replay installation automatically.".into();
                    execution.stage = "RecoveryRequired".into();
                    execution.updated_unix_ms = now_ms();
                    self.db.upsert_execution(&execution)?;
                    let _ = self.engine.transition(
                        &plan.id,
                        plan.state,
                        PlanState::Failed,
                        "interrupted mutation was not replayed",
                    );
                    self.add_recovery(&execution, "warning", "interrupted-mutation", "Driver installation was interrupted", "Installation was not replayed. Review the restore point and exported driver backup before further action.")?;
                }
                PlanState::Preflight | PlanState::Protected => {
                    if let Some(seq) = execution.restore_point_sequence
                        && !execution.mutation_started
                    {
                        let _ = self.platform.cancel_restore(
                            seq,
                            &aethercore_restore_point::description_for_plan(&plan.id),
                        );
                    }
                    execution.recovery_required = false;
                    execution.failure_message =
                        "Installation stopped before driver mutation.".into();
                    execution.stage = "FailedSafe".into();
                    execution.updated_unix_ms = now_ms();
                    execution.completed_unix_ms = Some(now_ms());
                    self.db.upsert_execution(&execution)?;
                    let _ = self.engine.transition(
                        &plan.id,
                        plan.state,
                        PlanState::Failed,
                        "service restart before driver mutation",
                    );
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn initialize_records(
        &self,
        plan_id: &str,
        actions: &[DriverInstallAction],
        now: i64,
    ) -> Result<()> {
        let execution = ExecutionRecord {
            plan_id: plan_id.into(),
            stage: "Preflight".into(),
            detail: "Revalidating selected devices before any change.".into(),
            started_unix_ms: now,
            updated_unix_ms: now,
            ..Default::default()
        };
        self.db.upsert_execution(&execution)?;
        for action in actions {
            self.db.upsert_install_item(&InstallItemRecord {
                plan_id: plan_id.into(),
                candidate_id: action.candidate_id.clone(),
                update_id: action.update_id.clone(),
                revision: action.revision,
                instance_id: action.instance_id.clone(),
                title: action.update_title.clone(),
                stage: "Queued".into(),
                before_driver_json: serde_json::to_string(&action.current_driver)
                    .unwrap_or_default(),
                before_problem_code: action.current_problem_code,
                updated_unix_ms: now,
                ..Default::default()
            })?;
        }
        self.db
            .add_checkpoint(plan_id, "", "preflight-started", "{}", now)?;
        Ok(())
    }

    fn run_plan(
        &self,
        owner_principal_key: &str,
        plan_id: &str,
        actions: Vec<DriverInstallAction>,
    ) -> Result<()> {
        self.preflight(plan_id, &actions)?;
        let mut execution = self
            .db
            .get_execution(plan_id)?
            .ok_or(InstallError::InvalidPlan)?;
        execution.stage = "Downloading".into();
        execution.detail = "Revalidating and downloading selected Windows-recommended drivers. No driver mutation has started.".into();
        execution.updated_unix_ms = now_ms();
        self.db.upsert_execution(&execution)?;
        self.db
            .add_checkpoint(plan_id, "", "preflight-complete", "{}", now_ms())?;

        // A single WUA update can legitimately service more than one selected devnode. Execute
        // each update identity once, then fan its result back out to every service-minted action.
        let identities = unique_identities(&actions);
        let platform = self.platform.clone();
        let backup_root = plan_backup_root(&self.product_data_root, plan_id)
            .map_err(|e| InstallError::Protection(e.to_string()))?;
        let mut restore_evidence: Option<RestorePointEvidence> = None;
        let mut before_called = false;

        // WUA performs its online revalidation and download before invoking this barrier. Everything
        // below must durably succeed before execute_driver_updates is allowed to call BeginInstall.
        let mut before = || -> std::result::Result<(), String> {
            if before_called {
                return Err("install mutation barrier invoked more than once".into());
            }
            before_called = true;

            let restore = self
                .platform
                .begin_restore(plan_id)
                .map_err(|e| format!("restore point: {e}"))?;
            // Set immediately so an error in any subsequent persistence/backup step can still pair
            // BEGIN_SYSTEM_CHANGE with CANCELLED_OPERATION in the outer failure path.
            restore_evidence = Some(restore.clone());

            let mut r = self
                .db
                .get_execution(plan_id)
                .map_err(|e| e.to_string())?
                .ok_or("execution record missing")?;
            r.restore_point_sequence = Some(restore.sequence_number);
            r.backup_root = backup_root.to_string_lossy().into_owned();
            r.stage = "BackingUpDrivers".into();
            r.detail = "Fresh restore point verified. Exporting currently bound OEM driver packages immediately before installation.".into();
            r.updated_unix_ms = now_ms();
            self.db.upsert_execution(&r).map_err(|e| e.to_string())?;
            self.db
                .add_checkpoint(
                    plan_id,
                    "",
                    "restore-point-verified",
                    &serde_json::to_string(&restore).unwrap_or_default(),
                    now_ms(),
                )
                .map_err(|e| e.to_string())?;

            self.backup_all(plan_id, &actions, &backup_root)
                .map_err(|e| e.to_string())?;
            self.engine
                .transition(
                    plan_id,
                    PlanState::Preflight,
                    PlanState::Protected,
                    "fresh restore point and pre-install driver backups verified",
                )
                .map_err(|e| e.to_string())?;
            self.db
                .add_checkpoint(plan_id, "", "protected", "{}", now_ms())
                .map_err(|e| e.to_string())?;

            // This durable flag deliberately means the irreversible mutation barrier was crossed,
            // not that a specific INF was already changed. A crash after this point is therefore
            // handled conservatively and is never replayed automatically.
            let mut r = self
                .db
                .get_execution(plan_id)
                .map_err(|e| e.to_string())?
                .ok_or("execution record missing")?;
            r.mutation_started = true;
            r.stage = "Installing".into();
            r.detail = "Windows Update installation is authorized to start.".into();
            r.updated_unix_ms = now_ms();
            self.db.upsert_execution(&r).map_err(|e| e.to_string())?;
            self.db
                .add_checkpoint(plan_id, "", "mutation-started", "{}", now_ms())
                .map_err(|e| e.to_string())?;
            self.engine
                .transition(
                    plan_id,
                    PlanState::Protected,
                    PlanState::Executing,
                    "WUA BeginInstall authorized after durable protection checkpoint",
                )
                .map_err(|e| e.to_string())?;
            Ok(())
        };

        let mut on_progress = |p: WuaProgress| {
            let _ = self.record_progress(owner_principal_key, plan_id, &actions, &identities, p);
        };
        let wua = platform
            .execute_wua(&identities, &mut on_progress, &mut before)
            .map_err(InstallError::Execution);
        let mut execution = self
            .db
            .get_execution(plan_id)?
            .ok_or(InstallError::InvalidPlan)?;

        match wua {
            Ok(result) => {
                if !execution.mutation_started {
                    return Err(InstallError::Execution(
                        "WUA returned without crossing the mutation barrier".into(),
                    ));
                }
                self.apply_wua_results(plan_id, &actions, &result)?;
                self.engine.transition(
                    plan_id,
                    PlanState::Executing,
                    PlanState::Verifying,
                    "WUA execution finished; re-interrogating devnodes",
                )?;
                execution.reboot_required = result.reboot_required;
                execution.stage = "Verifying".into();
                execution.detail = "Checking device health and bound drivers.".into();
                execution.updated_unix_ms = now_ms();
                self.db.upsert_execution(&execution)?;
                if let Some(restore) = restore_evidence.as_ref() {
                    self.close_restore_transaction(plan_id, &mut execution, restore, false)?;
                } else {
                    return Err(InstallError::Protection(
                        "restore-point evidence was lost after the mutation barrier".into(),
                    ));
                }
                self.finish_verification(plan_id, &actions, &mut execution, false)
            }
            Err(error) => {
                if let Some(restore) = restore_evidence.as_ref() {
                    let skip_restore = !execution.mutation_started;
                    self.close_restore_transaction(plan_id, &mut execution, restore, skip_restore)?;
                }
                if execution.mutation_started {
                    execution.recovery_required = true;
                    execution.failure_message = error.to_string();
                    execution.stage = "FailedAfterMutation".into();
                    execution.updated_unix_ms = now_ms();
                    execution.completed_unix_ms = Some(now_ms());
                    self.db.upsert_execution(&execution)?;
                    let _ = self.engine.transition(
                        plan_id,
                        PlanState::Executing,
                        PlanState::Failed,
                        "WUA failed after driver mutation began",
                    );
                    self.add_recovery(
                        &execution,
                        "warning",
                        "install-failure",
                        "Driver installation did not complete cleanly",
                        "A verified restore point and exported driver package evidence were preserved. No automatic rollback was attempted.",
                    )?;
                } else {
                    let current = self.engine.get_plan(plan_id)?.state;
                    if current == PlanState::Protected {
                        let _ = self.engine.transition(
                            plan_id,
                            PlanState::Protected,
                            PlanState::Failed,
                            "WUA failed before mutation",
                        );
                    } else if current == PlanState::Preflight {
                        let _ = self.engine.transition(
                            plan_id,
                            PlanState::Preflight,
                            PlanState::Failed,
                            "WUA failed before protection/mutation",
                        );
                    }
                    execution.failure_message = error.to_string();
                    execution.stage = "FailedBeforeMutation".into();
                    execution.updated_unix_ms = now_ms();
                    execution.completed_unix_ms = Some(now_ms());
                    self.db.upsert_execution(&execution)?;
                }
                Err(error)
            }
        }
    }

    fn close_restore_transaction(
        &self,
        plan_id: &str,
        execution: &mut ExecutionRecord,
        restore: &RestorePointEvidence,
        cancel: bool,
    ) -> Result<()> {
        let outcome = if cancel {
            self.platform
                .cancel_restore(restore.sequence_number, &restore.description)
        } else {
            self.platform
                .end_restore(restore.sequence_number, &restore.description)
        };
        match outcome {
            Ok(()) => {
                self.db.add_checkpoint(
                    plan_id,
                    "",
                    if cancel {
                        "restore-point-cancelled"
                    } else {
                        "restore-point-ended"
                    },
                    &format!(r#"{{"sequence":{}}}"#, restore.sequence_number),
                    now_ms(),
                )?;
            }
            Err(error) => {
                execution.recovery_required = true;
                execution.detail = format!(
                    "{} System Restore transaction close failed: {}",
                    execution.detail, error
                )
                .trim()
                .to_owned();
                execution.updated_unix_ms = now_ms();
                self.db.upsert_execution(execution)?;
                self.db.add_checkpoint(
                    plan_id,
                    "",
                    "restore-point-close-failed",
                    &serde_json::json!({
                        "sequence": restore.sequence_number,
                        "cancel": cancel,
                        "error": error.clone(),
                    })
                    .to_string(),
                    now_ms(),
                )?;
                self.add_recovery(
                    execution,
                    "warning",
                    "restore-transaction-close-failure",
                    "System Restore protection needs review",
                    "The restore point begin call succeeded, but Windows did not confirm the matching end/cancel call. Driver backup evidence remains preserved.",
                )?;
                return Err(InstallError::Protection(format!(
                    "System Restore transaction close was not confirmed: {error}"
                )));
            }
        }
        Ok(())
    }

    fn preflight(&self, _plan_id: &str, actions: &[DriverInstallAction]) -> Result<()> {
        let ids = actions
            .iter()
            .map(|a| a.instance_id.clone())
            .collect::<Vec<_>>();
        let fresh = self
            .platform
            .verify_devices(&ids)
            .map_err(InstallError::Preflight)?;
        if fresh.len() != actions.len() {
            return Err(InstallError::Preflight("device inventory changed".into()));
        }
        for action in actions {
            if action.driver_class.eq_ignore_ascii_case("firmware") {
                return Err(InstallError::Preflight(
                    "firmware remains protected from generic driver mutation".into(),
                ));
            }
            if action.installation_mode != "WindowsManaged"
                || action.authority_type != "WindowsUpdate"
                || action.trust_state != "WindowsManaged"
                || !matches!(
                    action.recommendation_state.as_str(),
                    "Recommended" | "Optional"
                )
            {
                return Err(InstallError::Preflight(format!(
                    "sealed driver authority is not executable: {} / {}",
                    action.authority_type, action.installation_mode
                )));
            }
            if action.provenance_digest != action_provenance_digest(action) {
                return Err(InstallError::Preflight(format!(
                    "sealed driver provenance changed: {}",
                    action.candidate_id
                )));
            }
            let v = fresh
                .iter()
                .find(|v| v.instance_id.eq_ignore_ascii_case(&action.instance_id))
                .ok_or_else(|| {
                    InstallError::Preflight(format!("device disappeared: {}", action.instance_id))
                })?;
            if !v.present {
                return Err(InstallError::Preflight(format!(
                    "device is no longer present: {}",
                    action.instance_id
                )));
            }
            if v.class_name.eq_ignore_ascii_case("firmware") {
                return Err(InstallError::Preflight(
                    "device class changed to protected firmware".into(),
                ));
            }
            if !action.driver_class.is_empty()
                && !v.class_name.eq_ignore_ascii_case(&action.driver_class)
            {
                return Err(InstallError::Preflight(format!(
                    "device class changed since scan: {}",
                    action.instance_id
                )));
            }
            let wanted = normalize_pnp_id(&action.matched_hardware_id);
            let id_still_bound = v
                .hardware_ids
                .iter()
                .chain(v.compatible_ids.iter())
                .any(|id| normalize_pnp_id(id) == wanted);
            if wanted.is_empty() || !id_still_bound {
                return Err(InstallError::Preflight(format!(
                    "matched hardware identity changed since scan: {}",
                    action.instance_id
                )));
            }
            if !drivers_equivalent(&v.driver, &action.current_driver) {
                return Err(InstallError::Preflight(format!(
                    "bound driver changed since scan: {}",
                    action.instance_id
                )));
            }
            let current_problem = if v.status.has_problem {
                v.status.problem_code
            } else {
                0
            };
            if current_problem != action.current_problem_code {
                return Err(InstallError::Preflight(format!(
                    "device problem state changed since scan: {}",
                    action.instance_id
                )));
            }
        }
        Ok(())
    }

    fn backup_all(
        &self,
        plan_id: &str,
        actions: &[DriverInstallAction],
        root: &Path,
    ) -> Result<()> {
        for action in actions {
            let mut item = self
                .db
                .install_items(plan_id)?
                .into_iter()
                .find(|i| i.candidate_id == action.candidate_id)
                .ok_or(InstallError::InvalidPlan)?;
            let Some(driver) = &action.current_driver else {
                item.stage = "BackupNotApplicable".into();
                item.detail = "No currently bound driver package exists to export.".into();
                item.updated_unix_ms = now_ms();
                self.db.upsert_install_item(&item)?;
                self.db.add_checkpoint(
                    plan_id,
                    &action.candidate_id,
                    "driver-backup-not-applicable",
                    r#"{"reason":"no-bound-driver"}"#,
                    now_ms(),
                )?;
                continue;
            };
            if !aethercore_driver_backup::validate_oem_inf_name(&driver.inf_path) {
                item.stage = "BackupNotApplicable".into();
                item.detail="The bound driver is not an OEM Driver Store INF, so PnPUtil export is not applicable.".into();
                item.updated_unix_ms = now_ms();
                self.db.upsert_install_item(&item)?;
                self.db.add_checkpoint(
                    plan_id,
                    &action.candidate_id,
                    "driver-backup-not-applicable",
                    r#"{"reason":"non-oem-inf"}"#,
                    now_ms(),
                )?;
                continue;
            }
            let destination = candidate_backup_root(root, &action.candidate_id)
                .map_err(|e| InstallError::Protection(e.to_string()))?;
            let evidence = self
                .platform
                .backup_driver(&driver.inf_path, &destination)
                .map_err(InstallError::Protection)?;
            item.backup_path = evidence.backup_directory.clone();
            item.stage = "BackedUp".into();
            item.detail = format!("Exported {} files before mutation.", evidence.file_count);
            item.updated_unix_ms = now_ms();
            self.db.upsert_install_item(&item)?;
            self.db.add_checkpoint(
                plan_id,
                &action.candidate_id,
                "driver-backup-verified",
                &serde_json::to_string(&evidence).unwrap_or_default(),
                now_ms(),
            )?;
        }
        Ok(())
    }

    fn record_progress(
        &self,
        owner_principal_key: &str,
        plan_id: &str,
        actions: &[DriverInstallAction],
        identities: &[UpdateIdentity],
        p: WuaProgress,
    ) -> Result<()> {
        let mut execution = self
            .db
            .get_execution(plan_id)?
            .ok_or(InstallError::InvalidPlan)?;
        let now = now_ms();
        let stage: String = match p.stage {
            ExecutionStage::Revalidating => "Revalidating",
            ExecutionStage::Downloading => "Downloading",
            ExecutionStage::Installing => "Installing",
        }
        .into();
        let candidate = identities
            .get(p.current_update_index as usize)
            .and_then(|identity| actions.iter().find(|a| same_update(a, identity)))
            .map(|a| a.candidate_id.clone())
            .unwrap_or_default();
        // Phase 10: publish every WUA callback to the in-memory telemetry plane first. Durable
        // SQLite remains a crash-safety ledger, not a UI animation transport.
        // DBT-P46-B16: only claim a byte figure when this tick actually
        // determined one. A failed WUA conversion keeps the previous detail
        // rather than announcing a download that rewound to zero.
        let detail = match (
            p.stage == ExecutionStage::Downloading,
            p.bytes_downloaded,
            p.bytes_total,
        ) {
            (true, Some(done), Some(total)) if total > 0 => {
                format!("Downloaded {done} of {total} bytes.")
            }
            _ => execution.detail.clone(),
        };
        self.telemetry.publish(ProgressTelemetry {
            owner_principal_key: owner_principal_key.into(),
            plan_id: plan_id.into(),
            stage: stage.clone(),
            progress_known: true,
            overall_percent: p.percent,
            current_item_id: candidate.clone(),
            detail: detail.clone(),
            bytes_completed: p
                .bytes_downloaded
                .or(execution.bytes_downloaded)
                .unwrap_or(0),
            bytes_total: p.bytes_total.or(execution.bytes_total).unwrap_or(0),
            emitted_unix_ms: now,
        });
        let percent_delta = execution.overall_percent.abs_diff(p.percent);
        let durable_due = execution.stage != stage
            || execution.current_candidate_id != candidate
            || percent_delta >= 5
            || now.saturating_sub(execution.updated_unix_ms) >= 2_000;
        if !durable_due {
            return Ok(());
        }
        execution.progress_known = true;
        execution.overall_percent = p.percent;
        execution.stage = stage;
        execution.updated_unix_ms = now;
        if !candidate.is_empty() {
            execution.current_candidate_id = candidate;
        }
        if let Some(identity) = identities.get(p.current_update_index as usize) {
            let matched = actions
                .iter()
                .filter(|a| same_update(a, identity))
                .collect::<Vec<_>>();
            let items = self.db.install_items(plan_id)?;
            for action in matched {
                let mut item = items
                    .iter()
                    .find(|i| i.candidate_id == action.candidate_id)
                    .cloned()
                    .ok_or(InstallError::InvalidPlan)?;
                item.stage = execution.stage.clone();
                item.progress_known = true;
                item.progress_percent = p.current_update_percent;
                item.updated_unix_ms = now;
                self.db.upsert_install_item(&item)?;
            }
        }
        if p.stage == ExecutionStage::Downloading {
            if p.bytes_downloaded.is_some() {
                execution.bytes_downloaded = p.bytes_downloaded;
            }
            if p.bytes_total.is_some() {
                execution.bytes_total = p.bytes_total;
            }
            execution.detail = detail;
        }
        self.db.upsert_execution(&execution)?;
        Ok(())
    }

    fn apply_wua_results(
        &self,
        plan_id: &str,
        actions: &[DriverInstallAction],
        result: &WuaExecutionResult,
    ) -> Result<()> {
        let items = self.db.install_items(plan_id)?;
        for action in actions {
            let identity = UpdateIdentity {
                update_id: action.update_id.clone(),
                revision: action.revision,
            };
            let w = result
                .updates
                .iter()
                .find(|w| w.identity == identity)
                .ok_or_else(|| {
                    InstallError::Execution(format!(
                        "WUA did not return a per-update result for {} revision {}",
                        action.update_id, action.revision
                    ))
                })?;
            let mut item = items
                .iter()
                .find(|i| i.candidate_id == action.candidate_id)
                .cloned()
                .ok_or(InstallError::InvalidPlan)?;
            item.result_code = w.result_code.clone();
            item.hresult = w.hresult;
            item.reboot_required = w.reboot_required;
            item.stage = "Installed".into();
            item.progress_known = true;
            item.progress_percent = 100;
            item.updated_unix_ms = now_ms();
            self.db.upsert_install_item(&item)?;
        }
        Ok(())
    }

    fn finish_verification(
        &self,
        plan_id: &str,
        actions: &[DriverInstallAction],
        execution: &mut ExecutionRecord,
        after_reboot: bool,
    ) -> Result<()> {
        let ids = actions
            .iter()
            .map(|a| a.instance_id.clone())
            .collect::<Vec<_>>();
        let fresh = self
            .platform
            .verify_devices(&ids)
            .map_err(InstallError::Verification)?;
        let mut all_healthy = true;
        for action in actions {
            let v = fresh
                .iter()
                .find(|v| v.instance_id.eq_ignore_ascii_case(&action.instance_id));
            let mut item = self
                .db
                .install_items(plan_id)?
                .into_iter()
                .find(|i| i.candidate_id == action.candidate_id)
                .ok_or(InstallError::InvalidPlan)?;
            if let Some(v) = v {
                item.after_driver_json = serde_json::to_string(&v.driver).unwrap_or_default();
                item.after_problem_code = v.status.problem_code;
                item.verified = v.present && !v.status.has_problem;
                item.detail = if item.verified {
                    "PnP reports the device present without a problem code."
                } else {
                    "PnP reports a device problem after installation."
                }
                .into();
            } else {
                item.verified = false;
                item.detail = "Device is not present after installation.".into();
            }
            item.stage = "Verified".into();
            item.updated_unix_ms = now_ms();
            all_healthy &= item.verified && result_code_success(&item.result_code);
            self.db.upsert_install_item(&item)?;
        }
        if execution.reboot_required && !after_reboot {
            execution.stage = "RebootPending".into();
            execution.detail =
                "Windows requires a restart before final verification can complete.".into();
            execution.reboot_boot_marker_ms = self.platform.boot_marker_ms().unwrap_or_default();
            execution.updated_unix_ms = now_ms();
            self.db.upsert_execution(execution)?;
            self.engine.transition(
                plan_id,
                PlanState::Verifying,
                PlanState::RebootPending,
                "WUA or a selected update requires reboot",
            )?;
            return Ok(());
        }
        execution.progress_known = true;
        execution.overall_percent = 100;
        execution.updated_unix_ms = now_ms();
        execution.completed_unix_ms = Some(now_ms());
        execution.reboot_required = false;
        if all_healthy {
            execution.stage = "Completed".into();
            execution.detail = "All selected devices passed post-install PnP verification.".into();
            self.db.upsert_execution(execution)?;
            self.engine.transition(
                plan_id,
                PlanState::Verifying,
                PlanState::Completed,
                "all selected devices verified healthy",
            )?;
        } else {
            execution.stage = "FailedVerification".into();
            execution.recovery_required = true;
            execution.failure_message =
                "One or more devices did not pass post-install verification.".into();
            self.db.upsert_execution(execution)?;
            self.engine.transition(
                plan_id,
                PlanState::Verifying,
                PlanState::Failed,
                "post-install device verification failed",
            )?;
            self.add_recovery(execution,"warning","verification-failure","A device needs recovery review","The installation result or PnP health check failed. The restore point and driver backup evidence were preserved.")?;
        }
        Ok(())
    }

    fn fail_safely(&self, plan_id: &str, message: &str) -> Result<()> {
        let state = self.engine.get_plan(plan_id).ok().map(|p| p.state);
        if let Some(mut r) = self.db.get_execution(plan_id)?
            && r.completed_unix_ms.is_none()
        {
            let restore_description = aethercore_restore_point::description_for_plan(plan_id);
            let close_error = match (r.restore_point_sequence, r.mutation_started) {
                (Some(seq), false) => self
                    .platform
                    .cancel_restore(seq, &restore_description)
                    .err(),
                (Some(seq), true) => self.platform.end_restore(seq, &restore_description).err(),
                _ => None,
            };
            r.failure_message = message.chars().take(1024).collect();
            r.recovery_required = r.mutation_started || close_error.is_some();
            r.stage = if r.mutation_started {
                "FailedAfterMutation"
            } else {
                "FailedSafe"
            }
            .into();
            r.detail = if r.mutation_started {
                "The operation stopped after the mutation barrier. AetherCore will not replay installation automatically.".into()
            } else {
                "The operation stopped before driver mutation. Any fresh restore point was cancelled when possible.".into()
            };
            if let Some(close_error) = close_error {
                r.detail.push_str(&format!(
                    " Restore transaction close also failed: {close_error}"
                ));
            }
            r.updated_unix_ms = now_ms();
            r.completed_unix_ms = Some(now_ms());
            self.db.upsert_execution(&r)?;
            if r.recovery_required {
                self.add_recovery(
                    &r,
                    "warning",
                    "safe-failure",
                    "Driver installation needs recovery review",
                    &r.detail,
                )?;
            }
        }
        if let Some(state) = state
            && matches!(
                state,
                PlanState::Preflight
                    | PlanState::Protected
                    | PlanState::Executing
                    | PlanState::Verifying
                    | PlanState::Resuming
            )
        {
            let _ = self.engine.transition(
                plan_id,
                state,
                PlanState::Failed,
                "driver install worker failed safely",
            );
        }
        Ok(())
    }

    fn add_recovery(
        &self,
        r: &ExecutionRecord,
        severity: &str,
        kind: &str,
        summary: &str,
        detail: &str,
    ) -> Result<()> {
        self.db.add_recovery_record(&RecoveryRecord {
            plan_id: r.plan_id.clone(),
            severity: severity.into(),
            kind: kind.into(),
            summary: summary.into(),
            detail: detail.into(),
            restore_point_sequence: r.restore_point_sequence,
            backup_root: r.backup_root.clone(),
            created_unix_ms: now_ms(),
            ..Default::default()
        })?;
        Ok(())
    }
}

fn unique_identities(actions: &[DriverInstallAction]) -> Vec<UpdateIdentity> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for action in actions {
        let identity = UpdateIdentity {
            update_id: action.update_id.clone(),
            revision: action.revision,
        };
        if seen.insert(identity.clone()) {
            out.push(identity);
        }
    }
    out
}
fn same_update(action: &DriverInstallAction, identity: &UpdateIdentity) -> bool {
    action.revision == identity.revision
        && action.update_id.eq_ignore_ascii_case(&identity.update_id)
}

fn action_from_selection(v: InstallSelection) -> DriverInstallAction {
    let mut action = DriverInstallAction {
        candidate_id: v.candidate.candidate_id,
        update_id: v.candidate.update_id,
        revision: v.candidate.revision,
        instance_id: v.instance_id,
        display_name: v.display_name,
        matched_hardware_id: v.candidate.matched_hardware_id,
        match_quality: v.candidate.match_quality,
        current_driver: v.current_driver.map(|d| DriverEvidence {
            provider: d.provider,
            version: d.version,
            inf_path: d.inf_path,
            date: d.date,
        }),
        current_problem_code: v.problem_code,
        update_title: v.candidate.title,
        provider: v.candidate.provider,
        driver_class: v.candidate.driver_class,
        min_download_bytes: v.candidate.min_download_bytes,
        max_download_bytes: v.candidate.max_download_bytes,
        target_version: v.candidate.target_version,
        authority_type: v.candidate.authority_type,
        authority_provider_id: v.candidate.authority_provider_id,
        official_source: v.candidate.official_source,
        recommendation_state: v.candidate.recommendation_state,
        installation_mode: v.candidate.installation_mode,
        trust_state: v.candidate.trust_state,
        provenance_digest: String::new(),
    };
    action.provenance_digest = action_provenance_digest(&action);
    action
}
fn action_provenance_digest(action: &DriverInstallAction) -> String {
    let material = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        action.candidate_id,
        action.update_id,
        action.revision,
        normalize_pnp_id(&action.instance_id),
        normalize_pnp_id(&action.matched_hardware_id),
        action.target_version,
        action.authority_type,
        action.authority_provider_id,
        action.official_source,
        action.installation_mode,
        action.trust_state
    );
    hex::encode(Sha256::digest(material.as_bytes()))
}
fn norm(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}
fn drivers_equivalent(actual: &Option<InstalledDriver>, expected: &Option<DriverEvidence>) -> bool {
    match (actual, expected) {
        (None, None) => true,
        (Some(a), Some(e)) => {
            norm(&a.provider) == norm(&e.provider)
                && norm(&a.version) == norm(&e.version)
                && norm(&a.inf_path) == norm(&e.inf_path)
                && norm(&a.date) == norm(&e.date)
        }
        _ => false,
    }
}
fn result_code_success(v: &str) -> bool {
    v == "orcSucceeded"
}
fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}
fn boot_changed(previous: i64, current: i64) -> bool {
    previous > 0 && current > 0 && (previous - current).abs() > 30_000
}
/// DBT-P46-B10: an empty stored record is the normal "no driver was recorded"
/// case; a record that will not parse is a fault, and the two must not both
/// collapse into an empty version string.
fn json_driver_version(raw: &str, which: &str, notes: &mut Vec<String>) -> String {
    if raw.trim().is_empty() {
        return String::new();
    }
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(value) => value
            .get("version")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_owned(),
        Err(error) => {
            notes.push(format!("stored {which}-install driver record is unreadable ({error}); its version is unknown, not absent"));
            String::new()
        }
    }
}
fn item_status(v: InstallItemRecord) -> InstallItemStatus {
    let mut notes = Vec::new();
    let before_version = json_driver_version(&v.before_driver_json, "before", &mut notes);
    let after_version = json_driver_version(&v.after_driver_json, "after", &mut notes);
    let mut detail = v.detail;
    for note in notes {
        if !detail.is_empty() {
            detail.push('\n');
        }
        detail.push_str(&note);
    }
    InstallItemStatus {
        candidate_id: v.candidate_id,
        instance_id: v.instance_id,
        title: v.title,
        stage: v.stage,
        progress_known: v.progress_known,
        progress_percent: v.progress_percent,
        result_code: v.result_code,
        hresult: v.hresult,
        reboot_required: v.reboot_required,
        verified: v.verified,
        before_version,
        after_version,
        before_problem_code: v.before_problem_code,
        after_problem_code: v.after_problem_code,
        backup_path: v.backup_path,
        detail,
    }
}

#[cfg(windows)]
mod platform {
    pub fn boot_marker_ms() -> Result<i64, String> {
        use windows::Win32::System::SystemInformation::GetTickCount64;
        let uptime = unsafe { GetTickCount64() } as i64;
        Ok(chrono::Utc::now().timestamp_millis().saturating_sub(uptime))
    }
}
#[cfg(not(windows))]
mod platform {
    pub fn boot_marker_ms() -> Result<i64, String> {
        Err("Windows only".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aethercore_windows_pnp::DeviceStatus;
    use std::sync::Mutex;

    struct FakePlatform {
        mutation_count: Mutex<u32>,
        reboot: bool,
    }
    impl InstallPlatform for FakePlatform {
        fn boot_marker_ms(&self) -> std::result::Result<i64, String> {
            Ok(1000)
        }
        fn verify_devices(
            &self,
            ids: &[String],
        ) -> std::result::Result<Vec<DeviceVerification>, String> {
            Ok(ids
                .iter()
                .map(|id| DeviceVerification {
                    instance_id: id.clone(),
                    present: true,
                    class_name: "Net".into(),
                    hardware_ids: vec!["PCI\\VEN_FAKE&DEV_0001".into()],
                    compatible_ids: Vec::new(),
                    status: DeviceStatus::default(),
                    driver: Some(InstalledDriver {
                        provider: "Vendor".into(),
                        version: "2.0".into(),
                        inf_path: "oem2.inf".into(),
                        date: "".into(),
                    }),
                })
                .collect())
        }
        fn begin_restore(&self, _: &str) -> std::result::Result<RestorePointEvidence, String> {
            Ok(RestorePointEvidence {
                sequence_number: 7,
                description: "test".into(),
                verified_fresh: true,
            })
        }
        fn end_restore(&self, _: i64, _: &str) -> std::result::Result<(), String> {
            Ok(())
        }
        fn cancel_restore(&self, _: i64, _: &str) -> std::result::Result<(), String> {
            Ok(())
        }
        fn backup_driver(
            &self,
            inf: &str,
            dst: &Path,
        ) -> std::result::Result<BackupEvidence, String> {
            Ok(BackupEvidence {
                source_inf: inf.into(),
                backup_directory: dst.display().to_string(),
                manifest_path: "manifest".into(),
                file_count: 1,
                total_bytes: 1,
                not_applicable: false,
            })
        }
        fn execute_wua(
            &self,
            ids: &[UpdateIdentity],
            progress: &mut dyn FnMut(WuaProgress),
            before: &mut dyn FnMut() -> std::result::Result<(), String>,
        ) -> std::result::Result<WuaExecutionResult, String> {
            progress(WuaProgress {
                stage: ExecutionStage::Downloading,
                percent: 50,
                current_update_index: 0,
                current_update_percent: 50,
                bytes_downloaded: Some(5),
                bytes_total: Some(10),
            });
            before()?;
            *self.mutation_count.lock().unwrap() += 1;
            Ok(WuaExecutionResult {
                result_code: "orcSucceeded".into(),
                hresult: 0,
                reboot_required: self.reboot,
                updates: ids
                    .iter()
                    .cloned()
                    .map(|identity| aethercore_windows_update::WuaUpdateResult {
                        identity,
                        result_code: "orcSucceeded".into(),
                        hresult: 0,
                        reboot_required: self.reboot,
                    })
                    .collect(),
            })
        }
    }

    // Full coordinator integration tests use a service-minted Phase 2 snapshot in the Windows
    // integration suite. These unit tests lock the pure safety predicates that must never regress.
    #[test]
    fn boot_marker_requires_real_boot_change() {
        assert!(!boot_changed(1_000, 20_000));
        assert!(boot_changed(1_000, 40_001));
    }
    #[test]
    fn only_success_result_codes_verify() {
        assert!(result_code_success("orcSucceeded"));
        assert!(!result_code_success("orcSucceededWithErrors"));
        assert!(!result_code_success("orcFailed"));
    }
    #[test]
    fn fake_platform_never_mutates_before_barrier() {
        let p = FakePlatform {
            mutation_count: Mutex::new(0),
            reboot: false,
        };
        let ids = vec![UpdateIdentity {
            update_id: "u".into(),
            revision: 1,
        }];
        let mut progress = |_: WuaProgress| {};
        let mut barrier = || Ok(());
        let _ = p.execute_wua(&ids, &mut progress, &mut barrier).unwrap();
        assert_eq!(*p.mutation_count.lock().unwrap(), 1);
    }

    #[test]
    fn same_wua_offer_is_executed_once_for_multiple_devnodes() {
        let mk = |candidate: &str, instance: &str| DriverInstallAction {
            candidate_id: candidate.into(),
            update_id: "shared-update".into(),
            revision: 4,
            instance_id: instance.into(),
            display_name: instance.into(),
            matched_hardware_id: "PCI\\VEN_FAKE&DEV_0001".into(),
            match_quality: "Hardware ID".into(),
            current_driver: None,
            current_problem_code: 0,
            update_title: "Vendor - Net".into(),
            provider: "Vendor".into(),
            driver_class: "Net".into(),
            min_download_bytes: 1,
            max_download_bytes: 2,
            target_version: "2.0".into(),
            authority_type: "WindowsUpdate".into(),
            authority_provider_id: "microsoft.windows-update".into(),
            official_source: "Windows Update".into(),
            recommendation_state: "Recommended".into(),
            installation_mode: "WindowsManaged".into(),
            trust_state: "WindowsManaged".into(),
            provenance_digest: String::new(),
        };
        let identities = unique_identities(&[mk("c1", "PCI\\A"), mk("c2", "PCI\\B")]);
        assert_eq!(identities.len(), 1);
        assert_eq!(identities[0].update_id, "shared-update");
        assert_eq!(identities[0].revision, 4);
    }

    /// DBT-P46-B10: an unreadable stored driver record and a driver that simply
    /// has no recorded version both produced "". The version string is evidence
    /// the owner reads to check what the install actually changed, so "we could
    /// not tell" must not render as "there was none".
    #[test]
    fn unreadable_stored_driver_json_is_reported_not_read_as_no_version() {
        let readable = InstallItemRecord {
            before_driver_json: r#"{"version":"1.0"}"#.into(),
            after_driver_json: r#"{"version":"2.0"}"#.into(),
            detail: "installed".into(),
            ..InstallItemRecord::default()
        };
        let status = item_status(readable);
        assert_eq!(status.before_version, "1.0");
        assert_eq!(status.after_version, "2.0");
        assert_eq!(status.detail, "installed", "a clean read adds no note");

        let no_prior_driver = InstallItemRecord {
            before_driver_json: String::new(),
            after_driver_json: r#"{"version":"2.0"}"#.into(),
            ..InstallItemRecord::default()
        };
        let status = item_status(no_prior_driver);
        assert_eq!(status.before_version, "");
        assert_eq!(
            status.detail, "",
            "no driver recorded before the install is normal, not a fault"
        );

        let corrupted = InstallItemRecord {
            before_driver_json: "{not json".into(),
            after_driver_json: r#"{"version":"2.0"}"#.into(),
            detail: "installed".into(),
            ..InstallItemRecord::default()
        };
        let status = item_status(corrupted);
        assert_eq!(
            status.before_version, "",
            "there genuinely is no version to show"
        );
        assert!(
            status.detail.contains("unreadable") && status.detail.starts_with("installed"),
            "the reason must reach the owner alongside the existing detail: {}",
            status.detail
        );
    }

    #[test]
    fn preflight_driver_binding_comparison_uses_all_evidence() {
        let actual = Some(InstalledDriver {
            provider: "Vendor".into(),
            version: "2.0".into(),
            inf_path: "oem2.inf".into(),
            date: "2026-01-01".into(),
        });
        let exact = Some(DriverEvidence {
            provider: "vendor".into(),
            version: "2.0".into(),
            inf_path: "OEM2.INF".into(),
            date: "2026-01-01".into(),
        });
        let wrong_inf = Some(DriverEvidence {
            provider: "Vendor".into(),
            version: "2.0".into(),
            inf_path: "oem9.inf".into(),
            date: "2026-01-01".into(),
        });
        assert!(drivers_equivalent(&actual, &exact));
        assert!(!drivers_equivalent(&actual, &wrong_inf));
    }
}
