#![deny(unsafe_op_in_unsafe_fn)]

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
    thread,
};

use aethercore_collector_runtime::{CancellationToken, CommitFence};
use aethercore_operation_engine::{
    now_ms, OperationEngine, PlanState, PlanView, StartupChangeAction,
};
use aethercore_operation_kernel::{MutationLease, MutationWorkload, ProgressTelemetry, ProgressTelemetryStore, ReadBudgetLease, ReadWorkload};
use aethercore_persistence::{
    Database, MaintenanceExecutionRecord, MaintenanceItemRecord, RecoveryRecord,
    StartupChangeRecord,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

const DOMAIN: &str = "StartupManager";
const MAX_ACTIONS_PER_PLAN: usize = 128;

#[derive(Debug, Error)]
pub enum StartupError {
    #[error("startup management is only available on Windows")]
    UnsupportedPlatform,
    #[error("startup inventory is already running")]
    Busy,
    #[error("startup inventory was cancelled before publication")]
    Cancelled,
    #[error("startup inventory is not ready")]
    ScanNotReady,
    #[error("startup inventory is stale")]
    StaleScan,
    #[error("startup inventory belongs to a different Windows principal")]
    OwnershipMismatch,
    #[error("startup target is unknown: {0}")]
    UnknownItem(String),
    #[error("startup target is protected: {0}")]
    Protected(String),
    #[error("startup target is not safely manageable: {0}")]
    NotManageable(String),
    #[error("no Disable decisions were explicitly reviewed")]
    PassiveDefault,
    #[error("service changes require the dedicated second confirmation")]
    ServiceConfirmationRequired,
    #[error("too many startup changes in one plan")]
    TooManyActions,
    #[error("startup plan is not authorized")]
    AuthorizationRequired,
    #[error("startup execution is already running")]
    AlreadyRunning,
    #[error("another AetherCore maintenance mutation is active")]
    MutationBusy,
    #[error("startup target changed since review: {0}")]
    Drift(String),
    #[error("startup state serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("operation engine: {0}")]
    Engine(#[from] aethercore_operation_engine::EngineError),
    #[error("persistence: {0}")]
    Persistence(#[from] aethercore_persistence::PersistenceError),
    #[error("platform operation failed: {0}")]
    Platform(String),
}

pub type Result<T> = std::result::Result<T, StartupError>;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StartupScanState { Idle, Scanning, Ready, Failed }
impl StartupScanState { pub fn as_str(self) -> &'static str { match self { Self::Idle=>"Idle",Self::Scanning=>"Scanning",Self::Ready=>"Ready",Self::Failed=>"Failed" } } }

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RecommendationDecision { Unreviewed, KeepEnabled, Disable }
impl RecommendationDecision { pub fn as_str(self)->&'static str { match self { Self::Unreviewed=>"Unreviewed",Self::KeepEnabled=>"KeepEnabled",Self::Disable=>"Disable" } } }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag="type", rename_all="camelCase")]
pub enum NativeState {
    RegistryValue { hive:String, key:String, value_name:String, view:String, exists:bool, value_type:u32, data_hex:String },
    StartupFile { path:String, exists:bool, size_bytes:u64, modified_unix_ms:i64, sha256:String, backup_path:String, backup_exists:bool },
    ScheduledTask { task_path:String, enabled:bool, xml_sha256:String },
    Service { service_name:String, start_type:u32, delayed_auto:bool, service_type:u32, binary_path:String, launch_protected:u32 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct StartupItem {
    pub item_id:String,
    pub kind:String,
    pub scope:String,
    pub display_name:String,
    pub publisher:String,
    pub command:String,
    pub source:String,
    pub enabled:bool,
    pub manageable:bool,
    pub protected:bool,
    pub protection_reason:String,
    pub impact:String,
    pub confidence:String,
    pub evidence_detail:String,
    pub recommendation:String,
    pub service_change:bool,
    #[serde(skip)]
    pub original_state:Option<NativeState>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct StartupSummary {
    pub total:u32, pub registry:u32, pub startup_folders:u32, pub scheduled_tasks:u32,
    pub services:u32, pub protected:u32, pub manageable:u32, pub high_impact:u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct StartupSnapshot {
    pub scan_id:String, pub state:StartupScanState, pub inventory_epoch:u64,
    pub started_unix_ms:i64, pub completed_unix_ms:i64, pub error_message:String,
    pub summary:StartupSummary, pub items:Vec<StartupItem>, pub warnings:Vec<String>,
}
impl Default for StartupSnapshot { fn default()->Self { Self{scan_id:String::new(),state:StartupScanState::Idle,inventory_epoch:0,started_unix_ms:0,completed_unix_ms:0,error_message:String::new(),summary:Default::default(),items:vec![],warnings:vec![]} } }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct StartupDecision { pub item_id:String, pub decision:RecommendationDecision }

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct StartupExecutionStatus {
    pub plan_id:String, pub plan_state:String, pub stage:String, pub progress_known:bool,
    pub overall_percent:u32, pub current_item_id:String, pub detail:String,
    pub mutation_started:bool, pub recovery_required:bool, pub failure_message:String,
    pub started_unix_ms:i64, pub updated_unix_ms:i64, pub completed_unix_ms:i64,
    pub items:Vec<StartupExecutionItem>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct StartupExecutionItem { pub item_id:String,pub display_name:String,pub kind:String,pub stage:String,pub result_code:String,pub detail:String }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct StartupHistoryEntry {
    pub change_id:String, pub origin_change_id:String, pub plan_id:String, pub item_id:String,
    pub kind:String, pub display_name:String, pub direction:String, pub state:String,
    pub detail:String, pub created_unix_ms:i64, pub updated_unix_ms:i64,
    /// DBT-P46-B14: None is the common case — most changes are never restored.
    /// Previously flattened to 0, indistinguishable from a restore at epoch 0.
    pub restored_unix_ms:Option<i64>,
    pub restorable:bool,
}

pub trait StartupPlatform: Send + Sync + 'static {
    fn scan(&self) -> Result<(Vec<StartupItem>, Vec<String>)>;
    fn current_state(&self, action:&StartupChangeAction) -> Result<String>;
    fn apply(&self, action:&StartupChangeAction) -> Result<()>;
}

#[cfg(windows)] mod windows_impl;
#[cfg(windows)] use windows_impl::{WindowsStartupPlatform, acquire_mutation_guard};

#[cfg(not(windows))]
struct WindowsStartupPlatform;
#[cfg(not(windows))]
impl StartupPlatform for WindowsStartupPlatform {
    fn scan(&self)->Result<(Vec<StartupItem>,Vec<String>)>{Err(StartupError::UnsupportedPlatform)}
    fn current_state(&self,_:&StartupChangeAction)->Result<String>{Err(StartupError::UnsupportedPlatform)}
    fn apply(&self,_:&StartupChangeAction)->Result<()>{Err(StartupError::UnsupportedPlatform)}
}
#[cfg(not(windows))]
fn acquire_mutation_guard()->Result<()> { Ok(()) }

pub struct StartupManager {
    engine:Arc<OperationEngine>, db:Arc<Database>, platform:Arc<dyn StartupPlatform>,
    snapshot:Arc<RwLock<StartupSnapshot>>, snapshot_owner:Arc<RwLock<String>>, running:Arc<Mutex<Option<String>>>, data_root:PathBuf,
    telemetry:ProgressTelemetryStore,
}

impl StartupManager {
    pub fn new(engine:Arc<OperationEngine>, db:Arc<Database>, data_root:PathBuf)->Self {
        Self::with_platform_and_telemetry(engine,db,data_root,Arc::new(WindowsStartupPlatform),ProgressTelemetryStore::new())
    }
    pub fn with_platform(engine:Arc<OperationEngine>,db:Arc<Database>,data_root:PathBuf,platform:Arc<dyn StartupPlatform>)->Self {
        Self::with_platform_and_telemetry(engine,db,data_root,platform,ProgressTelemetryStore::new())
    }
    pub fn with_telemetry(engine:Arc<OperationEngine>,db:Arc<Database>,data_root:PathBuf,telemetry:ProgressTelemetryStore)->Self {
        Self::with_platform_and_telemetry(engine,db,data_root,Arc::new(WindowsStartupPlatform),telemetry)
    }
    pub fn with_platform_and_telemetry(engine:Arc<OperationEngine>,db:Arc<Database>,data_root:PathBuf,platform:Arc<dyn StartupPlatform>,telemetry:ProgressTelemetryStore)->Self {
        Self{engine,db,platform,snapshot:Arc::new(RwLock::new(StartupSnapshot::default())),snapshot_owner:Arc::new(RwLock::new(String::new())),running:Arc::new(Mutex::new(None)),data_root,telemetry}
    }

    pub fn passive_scan(&self,owner_principal_key:&str,token:CancellationToken)->Result<StartupSnapshot>{
        self.passive_scan_with_fence(owner_principal_key,token,CommitFence::new())
    }
    pub fn passive_scan_with_fence(&self,owner_principal_key:&str,token:CancellationToken,commit_fence:CommitFence)->Result<StartupSnapshot>{
        if owner_principal_key.trim().is_empty()||token.is_cancelled(){return Err(StartupError::Cancelled)}
        let started=now_ms();let scan_id=Uuid::new_v4().to_string();
        let epoch={let current=self.snapshot.read().map_err(|_|StartupError::Busy)?;if current.state==StartupScanState::Scanning{return Err(StartupError::Busy)}current.inventory_epoch.saturating_add(1).max(started.max(0) as u64)};
        let (items,warnings)=self.platform.scan()?;if token.is_cancelled(){return Err(StartupError::Cancelled)}
        let ready=StartupSnapshot{scan_id,state:StartupScanState::Ready,inventory_epoch:epoch,started_unix_ms:started,completed_unix_ms:now_ms(),error_message:String::new(),summary:summarize(&items),items,warnings};
        let committed=commit_fence.try_commit_checked(||{
            let Ok(mut owner)=self.snapshot_owner.write() else{return None};
            let Ok(mut current)=self.snapshot.write() else{return None};
            if current.state==StartupScanState::Scanning||token.is_cancelled(){return None}
            *owner=owner_principal_key.to_owned();*current=ready.clone();Some(())
        });
        if committed!=Some(()){return Err(StartupError::Cancelled)}
        Ok(ready)
    }

    pub fn start_scan_with_lease(&self,owner_principal_key:&str,lease:ReadBudgetLease)->Result<StartupSnapshot>{
        if !lease.matches(ReadWorkload::StartupDiscovery){return Err(StartupError::Platform("read budget lease identity mismatch".into()))}
        self.start_scan_inner(owner_principal_key,lease)
    }
    fn start_scan_inner(&self,owner_principal_key:&str,read_budget_lease:ReadBudgetLease)->Result<StartupSnapshot>{
        let scan_id=Uuid::new_v4().to_string(); let started=now_ms();
        let initial={
            let mut owner=self.snapshot_owner.write().map_err(|_|StartupError::Busy)?;
            let mut current=self.snapshot.write().map_err(|_|StartupError::Busy)?;
            if current.state==StartupScanState::Scanning{return Err(StartupError::Busy)}
            let epoch=current.inventory_epoch.saturating_add(1).max(started.max(0) as u64);
            let initial=StartupSnapshot{scan_id:scan_id.clone(),state:StartupScanState::Scanning,inventory_epoch:epoch,started_unix_ms:started,..Default::default()};
            *owner=owner_principal_key.to_owned();*current=initial.clone();initial
        };
        let epoch=initial.inventory_epoch;
        let state=self.snapshot.clone(); let platform=self.platform.clone();
        if let Err(error)=thread::Builder::new().name("aether-startup-discovery".into()).spawn(move||{
            let _read_budget_lease=read_budget_lease;
            let completed=now_ms();
            let next=match platform.scan(){
                Ok((items,warnings))=>StartupSnapshot{scan_id,state:StartupScanState::Ready,inventory_epoch:epoch,started_unix_ms:started,completed_unix_ms:completed,error_message:String::new(),summary:summarize(&items),items,warnings},
                Err(e)=>StartupSnapshot{scan_id,state:StartupScanState::Failed,inventory_epoch:epoch,started_unix_ms:started,completed_unix_ms:completed,error_message:e.to_string(),..Default::default()},
            };
            if let Ok(mut g)=state.write(){*g=next;}
        }){
            let detail=format!("startup discovery worker unavailable: {error}");
            if let Ok(mut current)=self.snapshot.write(){current.state=StartupScanState::Failed;current.completed_unix_ms=now_ms();current.error_message=detail.clone();}
            return Err(StartupError::Platform(detail));
        }
        Ok(initial)
    }
    fn snapshot(&self)->StartupSnapshot{self.snapshot.read().map(|v|v.clone()).unwrap_or_default()}
    pub fn snapshot_for_owner(&self,owner_principal_key:&str)->Result<StartupSnapshot>{let owner=self.snapshot_owner.read().map_err(|_|StartupError::Busy)?;if owner.as_str()!=owner_principal_key{return Err(StartupError::OwnershipMismatch)}Ok(self.snapshot())}

    pub fn create_plan(&self,owner_principal_key:&str,scan_id:&str,epoch:u64,decisions:&[StartupDecision],confirm_service_changes:bool)->Result<PlanView>{
        let snap=self.snapshot_for_owner(owner_principal_key)?;
        if snap.state!=StartupScanState::Ready{return Err(StartupError::ScanNotReady)}
        if snap.scan_id!=scan_id||snap.inventory_epoch!=epoch{return Err(StartupError::StaleScan)}
        let mut seen=HashSet::new(); let mut actions=Vec::new();
        for d in decisions {
            if !seen.insert(&d.item_id){return Err(StartupError::UnknownItem(format!("duplicate {}",d.item_id)))}
            if d.decision!=RecommendationDecision::Disable { continue; }
            let item=snap.items.iter().find(|i|i.item_id==d.item_id).ok_or_else(||StartupError::UnknownItem(d.item_id.clone()))?;
            if item.protected{return Err(StartupError::Protected(item.display_name.clone()))}
            if !item.manageable{return Err(StartupError::NotManageable(item.display_name.clone()))}
            if item.service_change&&!confirm_service_changes{return Err(StartupError::ServiceConfirmationRequired)}
            let original=item.original_state.clone().ok_or_else(||StartupError::NotManageable(item.display_name.clone()))?;
            let change_id=Uuid::new_v4().to_string();
            let applied=disabled_state(&original,&self.data_root,&change_id)?;
            actions.push(StartupChangeAction{change_id:change_id.clone(),item_id:item.item_id.clone(),direction:"Disable".into(),startup_kind:item.kind.clone(),display_name:item.display_name.clone(),source_locator:item.source.clone(),original_state_json:serde_json::to_string(&original)?,applied_state_json:serde_json::to_string(&applied)?,service_change:item.service_change});
            if actions.len()>MAX_ACTIONS_PER_PLAN{return Err(StartupError::TooManyActions)}
        }
        if actions.is_empty(){return Err(StartupError::PassiveDefault)}
        self.engine.create_startup_plan(owner_principal_key,epoch,scan_id,actions).map_err(Into::into)
    }

    pub fn create_restore_plan(&self,owner_principal_key:&str,change_id:&str,confirm_service_changes:bool)->Result<PlanView>{
        let source=self.db.get_startup_change_for_owner(change_id,owner_principal_key)?.ok_or_else(||StartupError::UnknownItem(change_id.into()))?;
        if !matches!(source.state.as_str(),"Applied"|"AppliedRecovered") { return Err(StartupError::Drift(format!("change {} is not restorable",change_id))); }
        if source.kind=="Service" && !confirm_service_changes { return Err(StartupError::ServiceConfirmationRequired); }
        let action=StartupChangeAction{change_id:Uuid::new_v4().to_string(),item_id:source.item_id.clone(),direction:"Restore".into(),startup_kind:source.kind.clone(),display_name:source.display_name.clone(),source_locator:String::new(),original_state_json:source.original_json.clone(),applied_state_json:source.applied_json.clone(),service_change:source.kind=="Service"};
        self.engine.create_startup_plan(owner_principal_key,0,&format!("restore:{change_id}"),vec![action]).map_err(Into::into)
    }

    pub fn start_with_lease(&self,owner_principal_key:&str,plan_id:&str,lease:MutationLease)->Result<StartupExecutionStatus>{
        if !lease.matches(MutationWorkload::Startup,plan_id,owner_principal_key){return Err(StartupError::Platform("mutation lease identity mismatch".into()))}
        self.start_inner(owner_principal_key,plan_id,lease)
    }

    fn start_inner(&self,owner_principal_key:&str,plan_id:&str,mutation_lease:MutationLease)->Result<StartupExecutionStatus>{
        let plan=self.engine.get_plan_for_owner(plan_id,owner_principal_key)?;
        if plan.state!=PlanState::AwaitingAuthorization{return Err(StartupError::AuthorizationRequired)}
        let mut running=self.running.lock().map_err(|_|StartupError::AlreadyRunning)?; if running.is_some(){return Err(StartupError::AlreadyRunning)} *running=Some(plan_id.into()); drop(running);
        if let Err(error)=self.engine.consume_authorization_and_begin(plan_id,owner_principal_key,"one-shot consent consumed; startup operation entered preflight"){if let Ok(mut running)=self.running.lock(){*running=None;}return Err(error.into())}
        let now=now_ms();
        if let Err(error)=self.db.upsert_maintenance_execution(&MaintenanceExecutionRecord{plan_id:plan_id.into(),domain:DOMAIN.into(),stage:"Queued".into(),detail:"Authorized startup plan queued".into(),started_unix_ms:now,updated_unix_ms:now,..Default::default()}){
            let _=self.engine.transition(plan_id,PlanState::Preflight,PlanState::Failed,"startup execution journal initialization failed after consent");
            if let Ok(mut running)=self.running.lock(){*running=None;}
            return Err(error.into());
        }
        let engine=self.engine.clone();let db=self.db.clone();let platform=self.platform.clone();let running=self.running.clone();let telemetry=self.telemetry.clone();let owner=owner_principal_key.to_owned();let id=plan_id.to_string();
        let spawn=thread::Builder::new().name("aether-startup-worker".into()).spawn(move||{
            let _mutation_lease=mutation_lease;
            let result=execute_plan_with_telemetry(&engine,&db,platform.as_ref(),&owner,&telemetry,&id);
            if let Err(e)=result{let _=fail_execution(&engine,&db,&id,&e.to_string());}
            telemetry.clear_for_owner(&owner,&id);
            if let Ok(mut g)=running.lock(){*g=None;}
        });
        if let Err(error)=spawn{
            let detail=format!("startup worker creation failed: {error}");
            let _=fail_execution(&self.engine,&self.db,plan_id,&detail);
            self.telemetry.clear_for_owner(owner_principal_key,plan_id);
            if let Ok(mut running)=self.running.lock(){*running=None;}
            return Err(StartupError::Platform(detail));
        }
        self.status(owner_principal_key,Some(plan_id))?.ok_or(StartupError::AlreadyRunning)
    }

    pub fn status(&self,owner_principal_key:&str,plan_id:Option<&str>)->Result<Option<StartupExecutionStatus>>{
        let rec=match plan_id{Some(id)=>{self.engine.get_plan_for_owner(id,owner_principal_key)?;self.db.get_maintenance_execution(id)?},None=>self.db.latest_maintenance_execution_for_owner(DOMAIN,owner_principal_key)?};
        let Some(rec)=rec else{return Ok(None)}; if rec.domain!=DOMAIN{return Ok(None)}
        let plan=self.engine.get_plan_for_owner(&rec.plan_id,owner_principal_key)?;
        let action_meta=self.engine.startup_actions(&rec.plan_id).unwrap_or_default().into_iter().map(|a|(a.item_id.clone(),(a.display_name,a.startup_kind))).collect::<HashMap<_,_>>();
        let items=self.db.maintenance_items(&rec.plan_id)?.into_iter().map(|i|{let (display_name,kind)=action_meta.get(&i.item_id).cloned().unwrap_or_else(||(i.kind.clone(),i.kind.clone()));StartupExecutionItem{item_id:i.item_id,display_name,kind,stage:i.stage,result_code:i.result_code,detail:i.detail}}).collect();
        let live=self.telemetry.get_for_owner(owner_principal_key,&rec.plan_id).filter(|value|value.emitted_unix_ms>=rec.updated_unix_ms);
        Ok(Some(StartupExecutionStatus{plan_id:rec.plan_id,plan_state:plan.state.as_str().into(),stage:live.as_ref().map(|v|v.stage.clone()).unwrap_or(rec.stage),progress_known:live.as_ref().map(|v|v.progress_known).unwrap_or(rec.progress_known),overall_percent:live.as_ref().map(|v|v.overall_percent).unwrap_or(rec.overall_percent),current_item_id:live.as_ref().map(|v|v.current_item_id.clone()).unwrap_or(rec.current_item_id),detail:live.as_ref().map(|v|v.detail.clone()).unwrap_or(rec.detail),mutation_started:rec.mutation_started,recovery_required:rec.recovery_required,failure_message:rec.failure_message,started_unix_ms:rec.started_unix_ms,updated_unix_ms:live.as_ref().map(|v|v.emitted_unix_ms).unwrap_or(rec.updated_unix_ms),completed_unix_ms:rec.completed_unix_ms.unwrap_or(0),items}))
    }

    pub fn history(&self,owner_principal_key:&str,limit:usize)->Result<Vec<StartupHistoryEntry>>{
        Ok(self.db.startup_changes_for_owner(owner_principal_key,limit)?.into_iter().map(|r|{let direction=r.direction.clone();let restorable=matches!(r.state.as_str(),"Applied"|"AppliedRecovered")&&direction=="Disable";StartupHistoryEntry{change_id:r.change_id,origin_change_id:r.origin_change_id,plan_id:r.plan_id,item_id:r.item_id,kind:r.kind,display_name:r.display_name,direction,state:r.state.clone(),detail:r.detail,created_unix_ms:r.created_unix_ms,updated_unix_ms:r.updated_unix_ms,restored_unix_ms:r.restored_unix_ms,restorable}}).collect())
    }

    pub fn recover_incomplete(&self)->Result<()> {
        // Never replay startup mutations after restart. Reconcile prepared records by observation only.
        for mut rec in self.db.startup_changes_in_states(&["Prepared"])? {
            let action=StartupChangeAction{change_id:rec.change_id.clone(),item_id:rec.item_id.clone(),direction:rec.direction.clone(),startup_kind:rec.kind.clone(),display_name:rec.display_name.clone(),source_locator:String::new(),original_state_json:rec.original_json.clone(),applied_state_json:rec.applied_json.clone(),service_change:rec.kind=="Service"};
            match self.platform.current_state(&action) {
                Ok(current) if current==expected_before(&action) => {rec.state="NoChange".into();rec.detail="Restart recovery observed original state; no mutation replayed.".into();}
                Ok(current) if current==target_after(&action) => {rec.state="AppliedRecovered".into();rec.detail="Restart recovery observed the intended changed state; no mutation replayed.".into();}
                Ok(_) => {rec.state="RecoveryRequired".into();rec.detail="Restart recovery found ambiguous startup state; manual review required.".into();}
                Err(e) => {rec.state="RecoveryRequired".into();rec.detail=format!("Restart recovery could not inspect target: {e}");}
            }
            rec.updated_unix_ms=now_ms(); self.db.upsert_startup_change(&rec)?;
        }
        for plan in self.engine.recoverable_plans()? {
            if self.engine.startup_actions(&plan.id).is_err(){continue}
            if let Some(mut execution)=self.db.get_maintenance_execution(&plan.id)? {
                if execution.domain!=DOMAIN{continue}
                execution.stage="RecoveryRequired".into();execution.recovery_required=true;execution.failure_message="Startup operation was interrupted. AetherCore will not replay startup mutations automatically.".into();execution.updated_unix_ms=now_ms();execution.completed_unix_ms=Some(execution.updated_unix_ms);self.db.upsert_maintenance_execution(&execution)?;
                let _=self.engine.transition(&plan.id,plan.state,PlanState::Failed,"startup mutation interrupted; no replay");
                self.db.add_recovery_record(&RecoveryRecord{plan_id:plan.id,severity:"Amber".into(),kind:"StartupRecovery".into(),summary:"Startup change interrupted".into(),detail:"AetherCore did not replay the mutation. Review Startup history before restoring or retrying.".into(),created_unix_ms:now_ms(),..Default::default()})?;
            }
        }
        Ok(())
    }
}

fn execute_plan(engine:&OperationEngine,db:&Database,platform:&dyn StartupPlatform,plan_id:&str)->Result<()> {
    let telemetry=ProgressTelemetryStore::new();
    execute_plan_with_telemetry(engine,db,platform,"",&telemetry,plan_id)
}

fn publish_progress(telemetry:&ProgressTelemetryStore,owner_principal_key:&str,plan_id:&str,stage:&str,pct:u32,current_item_id:&str,detail:&str){
    telemetry.publish(ProgressTelemetry{owner_principal_key:owner_principal_key.into(),plan_id:plan_id.into(),stage:stage.into(),progress_known:true,overall_percent:pct,current_item_id:current_item_id.into(),detail:detail.into(),..Default::default()});
}

fn execute_plan_with_telemetry(engine:&OperationEngine,db:&Database,platform:&dyn StartupPlatform,owner_principal_key:&str,telemetry:&ProgressTelemetryStore,plan_id:&str)->Result<()> {
    let actions=engine.startup_actions(plan_id)?;
    let _guard=acquire_mutation_guard()?;
    publish_progress(telemetry,owner_principal_key,plan_id,"Preflight",0,"","Validating exact current startup state");
    update_exec(db,plan_id,"Preflight",0,"Validating exact current startup state",false,false,"")?;
    for action in &actions {
        let current=platform.current_state(action)?; let expected=expected_before(action);
        if current!=expected{return Err(StartupError::Drift(action.display_name.clone()))}
    }
    // Durable rollback evidence is committed before the plan reaches Protected.
    for action in &actions {
        let now=now_ms();
        let origin=if action.direction=="Restore" { origin_from_restore_scan(engine,plan_id).unwrap_or_else(||action.change_id.clone()) } else { action.change_id.clone() };
        db.upsert_startup_change(&StartupChangeRecord{change_id:action.change_id.clone(),origin_change_id:origin,plan_id:plan_id.into(),item_id:action.item_id.clone(),kind:action.startup_kind.clone(),display_name:action.display_name.clone(),direction:action.direction.clone(),original_json:action.original_state_json.clone(),applied_json:action.applied_state_json.clone(),state:"Prepared".into(),detail:"Exact original and target states durably recorded before mutation.".into(),created_unix_ms:now,updated_unix_ms:now,restored_unix_ms:None})?;
    }
    engine.transition(plan_id,PlanState::Preflight,PlanState::Protected,"startup rollback evidence persisted")?;
    engine.transition(plan_id,PlanState::Protected,PlanState::Executing,"startup mutation barrier")?;
    publish_progress(telemetry,owner_principal_key,plan_id,"Executing",0,"","Applying reviewed startup changes");
    update_exec(db,plan_id,"Executing",0,"Applying reviewed startup changes",true,false,"")?;
    for (index,action) in actions.iter().enumerate() {
        let pct=((index*100)/actions.len().max(1)) as u32;
        publish_progress(telemetry,owner_principal_key,plan_id,"Executing",pct,&action.item_id,&format!("{}: {}",action.direction,action.display_name));
        db.upsert_maintenance_item(&MaintenanceItemRecord{plan_id:plan_id.into(),item_id:action.item_id.clone(),kind:action.startup_kind.clone(),stage:"Executing".into(),result_code:String::new(),bytes_affected:0,detail:format!("{} {}",action.direction,action.startup_kind),updated_unix_ms:now_ms()})?;
        update_exec(db,plan_id,"Executing",pct,&format!("{}: {}",action.direction,action.display_name),true,false,"")?;
        if let Some(mut execution)=db.get_maintenance_execution(plan_id)?{execution.current_item_id=action.item_id.clone();execution.updated_unix_ms=now_ms();db.upsert_maintenance_execution(&execution)?;}
        platform.apply(action)?;
        let after=platform.current_state(action)?; let target=target_after(action);
        if after!=target{return Err(StartupError::Drift(format!("{} failed post-change verification",action.display_name)))}
        let mut rec=db.get_startup_change(&action.change_id)?.ok_or_else(||StartupError::UnknownItem(action.change_id.clone()))?;
        rec.state=if action.direction=="Restore"{"Restored".into()}else{"Applied".into()};rec.detail="Native post-change verification matched the immutable target state.".into();rec.updated_unix_ms=now_ms();if action.direction=="Restore"{rec.restored_unix_ms=Some(rec.updated_unix_ms);}db.upsert_startup_change(&rec)?;
        if action.direction=="Restore" && rec.origin_change_id!=rec.change_id {
            if let Some(mut source)=db.get_startup_change(&rec.origin_change_id)? {
                source.state="Restored".into();source.detail=format!("Restored by startup change {}.",rec.change_id);source.updated_unix_ms=rec.updated_unix_ms;source.restored_unix_ms=Some(rec.updated_unix_ms);db.upsert_startup_change(&source)?;
            }
        }
        db.upsert_maintenance_item(&MaintenanceItemRecord{plan_id:plan_id.into(),item_id:action.item_id.clone(),kind:action.startup_kind.clone(),stage:"Completed".into(),result_code:"Verified".into(),bytes_affected:0,detail:"Change verified and reversible evidence retained.".into(),updated_unix_ms:now_ms()})?;
    }
    engine.transition(plan_id,PlanState::Executing,PlanState::Verifying,"startup post-change verification")?;
    publish_progress(telemetry,owner_principal_key,plan_id,"Verifying",95,"","Verifying exact post-change startup state");
    for action in &actions { if platform.current_state(action)?!=target_after(action){return Err(StartupError::Drift(action.display_name.clone()))} }
    engine.transition(plan_id,PlanState::Verifying,PlanState::Completed,"startup changes verified")?;
    publish_progress(telemetry,owner_principal_key,plan_id,"Completed",100,"","All reviewed startup changes verified");
    let now=now_ms();update_exec(db,plan_id,"Completed",100,"All reviewed startup changes verified",true,false,"")?;if let Some(mut r)=db.get_maintenance_execution(plan_id)?{r.current_item_id.clear();r.completed_unix_ms=Some(now);r.updated_unix_ms=now;db.upsert_maintenance_execution(&r)?;} Ok(())
}

fn fail_execution(engine:&OperationEngine,db:&Database,plan_id:&str,message:&str)->Result<()> {
    let current=engine.get_plan(plan_id)?;let mutation=db.get_maintenance_execution(plan_id)?.map(|r|r.mutation_started).unwrap_or(false);
    if !current.state.is_terminal(){let _=engine.transition(plan_id,current.state,PlanState::Failed,"startup execution failed");}
    update_exec(db,plan_id,"Failed",0,"Startup change did not complete cleanly",mutation,mutation,message)?;
    if mutation {db.add_recovery_record(&RecoveryRecord{plan_id:plan_id.into(),severity:"Amber".into(),kind:"StartupRecovery".into(),summary:"Startup change needs review".into(),detail:message.into(),created_unix_ms:now_ms(),..Default::default()})?;}
    Ok(())
}

fn update_exec(db:&Database,plan_id:&str,stage:&str,pct:u32,detail:&str,mutation:bool,recovery:bool,failure:&str)->Result<()> {
    let old=db.get_maintenance_execution(plan_id)?.unwrap_or_default();let now=now_ms();
    db.upsert_maintenance_execution(&MaintenanceExecutionRecord{plan_id:plan_id.into(),domain:DOMAIN.into(),stage:stage.into(),progress_known:true,overall_percent:pct,current_item_id:old.current_item_id,detail:detail.into(),mutation_started:mutation||old.mutation_started,recovery_required:recovery||old.recovery_required,failure_message:failure.into(),started_unix_ms:if old.started_unix_ms==0{now}else{old.started_unix_ms},updated_unix_ms:now,completed_unix_ms:old.completed_unix_ms,..old})?;Ok(())
}

fn disabled_state(original:&NativeState,data_root:&Path,change_id:&str)->Result<NativeState>{
    Ok(match original {
        NativeState::RegistryValue{hive,key,value_name,view,value_type,data_hex,..}=>NativeState::RegistryValue{hive:hive.clone(),key:key.clone(),value_name:value_name.clone(),view:view.clone(),exists:false,value_type:*value_type,data_hex:data_hex.clone()},
        NativeState::StartupFile{path,size_bytes,modified_unix_ms,sha256,..}=>NativeState::StartupFile{path:path.clone(),exists:false,size_bytes:*size_bytes,modified_unix_ms:*modified_unix_ms,sha256:sha256.clone(),backup_path:data_root.join("startup-backups").join(change_id).join(Path::new(path).file_name().unwrap_or_default()).to_string_lossy().into_owned(),backup_exists:true},
        NativeState::ScheduledTask{task_path,xml_sha256,..}=>NativeState::ScheduledTask{task_path:task_path.clone(),enabled:false,xml_sha256:xml_sha256.clone()},
        NativeState::Service{service_name,service_type,binary_path,launch_protected,..}=>NativeState::Service{service_name:service_name.clone(),start_type:3,delayed_auto:false,service_type:*service_type,binary_path:binary_path.clone(),launch_protected:*launch_protected},
    })
}
fn expected_before(a:&StartupChangeAction)->String{if a.direction=="Restore"{a.applied_state_json.clone()}else{a.original_state_json.clone()}}
fn target_after(a:&StartupChangeAction)->String{if a.direction=="Restore"{a.original_state_json.clone()}else{a.applied_state_json.clone()}}
fn origin_from_restore_scan(engine:&OperationEngine,plan_id:&str)->Option<String>{engine.get_plan(plan_id).ok()?.scan_id.strip_prefix("restore:").map(str::to_owned)}
fn summarize(items:&[StartupItem])->StartupSummary{let mut s=StartupSummary::default();for i in items{s.total+=1;match i.kind.as_str(){"RegistryRun"|"RegistryRunOnce"=>s.registry+=1,"StartupFolder"=>s.startup_folders+=1,"ScheduledTask"=>s.scheduled_tasks+=1,"Service"=>s.services+=1,_=>{}}if i.protected{s.protected+=1}if i.manageable{s.manageable+=1}if i.impact=="High"{s.high_impact+=1}}s}

#[cfg(test)]
mod tests {
    use super::*;
    use aethercore_persistence::Database;
    use std::sync::atomic::{AtomicUsize,Ordering};

    const OWNER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn startup_file_state(modified_unix_ms: Option<i64>) -> NativeState {
        NativeState::StartupFile {
            path: r"C:\Users\x\Start Menu\Programs\Startup\thing.lnk".into(),
            exists: true,
            size_bytes: 1024,
            modified_unix_ms,
            sha256: "a".repeat(64),
            backup_path: String::new(),
            backup_exists: false,
        }
    }

    /// DBT-P46-B12. `modified_unix_ms` is part of `original_state_json`, and
    /// `execute_plan_with_telemetry` authorises a mutation only when the
    /// freshly queried state string EQUALS the stored one. A failed
    /// `.modified()` read used to write 0 there, with two consequences: it
    /// stamps 1970-01-01 into durable rollback evidence, and when the read
    /// fails at both scan and preflight the two unknowns compare EQUAL —
    /// hiding exactly the drift that comparison exists to catch.
    #[test]
    fn an_unread_startup_file_mtime_is_not_the_epoch() {
        let unknown = startup_file_state(None);
        let epoch = startup_file_state(Some(0));
        assert_ne!(
            unknown, epoch,
            "an unread mtime must not equal a file genuinely stamped at the epoch"
        );

        let unknown_json = serde_json::to_string(&unknown).expect("serialize");
        let epoch_json = serde_json::to_string(&epoch).expect("serialize");
        assert_ne!(
            unknown_json, epoch_json,
            "the drift check compares these strings, so they must differ"
        );
        assert!(
            unknown_json.contains("\"modifiedUnixMs\":null"),
            "unknown must be null, not 0: {unknown_json}"
        );
        assert!(
            epoch_json.contains("\"modifiedUnixMs\":0"),
            "a real epoch timestamp must stay 0: {epoch_json}"
        );

        // A known mtime keeps its exact prior representation, so state written
        // before this change still compares equal to state written after it.
        assert!(
            serde_json::to_string(&startup_file_state(Some(1_700_000_000_000)))
                .expect("serialize")
                .contains("\"modifiedUnixMs\":1700000000000")
        );
    }
    fn approve(engine:&OperationEngine, plan:&PlanView){let intent=engine.begin_consent_intent(&plan.id,OWNER).unwrap();engine.approve_consent_intent(&intent.intent_id,OWNER,4242).unwrap();}

    struct MockPlatform{items:Vec<StartupItem>,states:Mutex<HashMap<String,String>>,mutations:AtomicUsize}
    impl StartupPlatform for MockPlatform{
        fn scan(&self)->Result<(Vec<StartupItem>,Vec<String>)>{Ok((self.items.clone(),vec![]))}
        fn current_state(&self,a:&StartupChangeAction)->Result<String>{Ok(self.states.lock().unwrap().get(&a.item_id).cloned().unwrap_or_else(||expected_before(a)))}
        fn apply(&self,a:&StartupChangeAction)->Result<()>{self.mutations.fetch_add(1,Ordering::SeqCst);self.states.lock().unwrap().insert(a.item_id.clone(),target_after(a));Ok(())}
    }
    fn item(id:&str,protected:bool,service:bool)->StartupItem{StartupItem{item_id:id.into(),kind:if service{"Service".into()}else{"RegistryRun".into()},scope:"Machine".into(),display_name:id.into(),publisher:"Vendor".into(),command:"agent.exe".into(),source:"source".into(),enabled:true,manageable:!protected,protected,protection_reason:if protected{"protected".into()}else{String::new()},impact:"Unknown".into(),confidence:"InsufficientEvidence".into(),evidence_detail:"No directly correlated boot-duration evidence.".into(),recommendation:"Review".into(),service_change:service,original_state:Some(if service{NativeState::Service{service_name:id.into(),start_type:2,delayed_auto:true,service_type:16,binary_path:r"C:\Vendor\agent.exe".into(),launch_protected:0}}else{NativeState::RegistryValue{hive:"HKLM".into(),key:"Run".into(),value_name:id.into(),view:"64".into(),exists:true,value_type:1,data_hex:"41000000".into()}})}}
    fn manager(items:Vec<StartupItem>)->(StartupManager,Arc<MockPlatform>,PathBuf){let path=std::env::temp_dir().join(format!("aether-startup-{}.db",Uuid::new_v4()));let db=Arc::new(Database::open(&path).unwrap());let engine=Arc::new(OperationEngine::new(db.clone()));let platform=Arc::new(MockPlatform{items,states:Mutex::new(HashMap::new()),mutations:AtomicUsize::new(0)});(StartupManager::with_platform(engine,db,std::env::temp_dir(),platform.clone()),platform,path)}
    fn ready(m:&StartupManager){let now=now_ms();*m.snapshot_owner.write().unwrap()=OWNER.into();*m.snapshot.write().unwrap()=StartupSnapshot{scan_id:"s".into(),state:StartupScanState::Ready,inventory_epoch:1,started_unix_ms:now,completed_unix_ms:now,summary:summarize(&m.platform.scan().unwrap().0),items:m.platform.scan().unwrap().0,warnings:vec![],error_message:String::new()};}

    #[test] fn passive_default_means_zero_mutations(){let(m,p,path)=manager(vec![item("a",false,false)]);ready(&m);assert!(matches!(m.create_plan(OWNER,"s",1,&[StartupDecision{item_id:"a".into(),decision:RecommendationDecision::Unreviewed}],false),Err(StartupError::PassiveDefault)));assert_eq!(p.mutations.load(Ordering::SeqCst),0);drop(m);let _=std::fs::remove_file(path);}
    #[test] fn keep_enabled_is_not_an_action(){let(m,_,path)=manager(vec![item("a",false,false),item("b",false,false)]);ready(&m);let plan=m.create_plan(OWNER,"s",1,&[StartupDecision{item_id:"a".into(),decision:RecommendationDecision::KeepEnabled},StartupDecision{item_id:"b".into(),decision:RecommendationDecision::Disable}],false).unwrap();assert_eq!(plan.action_count,1);drop(m);let _=std::fs::remove_file(path);}
    #[test] fn protected_target_cannot_be_selected(){let(m,_,path)=manager(vec![item("security",true,false)]);ready(&m);assert!(matches!(m.create_plan(OWNER,"s",1,&[StartupDecision{item_id:"security".into(),decision:RecommendationDecision::Disable}],false),Err(StartupError::Protected(_))));drop(m);let _=std::fs::remove_file(path);}
    #[test] fn service_requires_second_confirmation(){let(m,_,path)=manager(vec![item("svc",false,true)]);ready(&m);assert!(matches!(m.create_plan(OWNER,"s",1,&[StartupDecision{item_id:"svc".into(),decision:RecommendationDecision::Disable}],false),Err(StartupError::ServiceConfirmationRequired)));assert!(m.create_plan(OWNER,"s",1,&[StartupDecision{item_id:"svc".into(),decision:RecommendationDecision::Disable}],true).is_ok());drop(m);let _=std::fs::remove_file(path);}
    #[test] fn service_restore_requires_second_confirmation(){let(m,_,path)=manager(vec![item("svc",false,true)]);ready(&m);let plan=m.create_plan(OWNER,"s",1,&[StartupDecision{item_id:"svc".into(),decision:RecommendationDecision::Disable}],true).unwrap();let action=m.engine.startup_actions(&plan.id).unwrap().into_iter().next().unwrap();let now=now_ms();m.db.upsert_startup_change(&StartupChangeRecord{change_id:action.change_id.clone(),origin_change_id:action.change_id.clone(),plan_id:plan.id.clone(),item_id:action.item_id.clone(),kind:"Service".into(),display_name:"svc".into(),direction:"Disable".into(),original_json:action.original_state_json.clone(),applied_json:action.applied_state_json.clone(),state:"Applied".into(),detail:String::new(),created_unix_ms:now,updated_unix_ms:now,restored_unix_ms:None}).unwrap();assert!(matches!(m.create_restore_plan(OWNER,&action.change_id,false),Err(StartupError::ServiceConfirmationRequired)));assert!(m.create_restore_plan(OWNER,&action.change_id,true).is_ok());drop(m);let _=std::fs::remove_file(path);}
    #[test]
    fn restart_recovery_observes_original_state_without_replay() {
        let (m,p,path)=manager(vec![item("agent",false,false)]);
        ready(&m);
        let plan=m.create_plan(OWNER,"s",1,&[StartupDecision{item_id:"agent".into(),decision:RecommendationDecision::Disable}],false).unwrap();
        let action=m.engine.startup_actions(&plan.id).unwrap().into_iter().next().unwrap();
        let now=now_ms();
        m.db.upsert_startup_change(&StartupChangeRecord{change_id:action.change_id.clone(),origin_change_id:action.change_id.clone(),plan_id:plan.id.clone(),item_id:action.item_id.clone(),kind:action.startup_kind.clone(),display_name:action.display_name.clone(),direction:"Disable".into(),original_json:action.original_state_json.clone(),applied_json:action.applied_state_json.clone(),state:"Prepared".into(),detail:String::new(),created_unix_ms:now,updated_unix_ms:now,restored_unix_ms:None}).unwrap();
        m.recover_incomplete().unwrap();
        let rec=m.db.get_startup_change(&action.change_id).unwrap().unwrap();
        assert_eq!(rec.state,"NoChange");
        assert_eq!(p.mutations.load(Ordering::SeqCst),0);
        drop(m);
        let _=std::fs::remove_file(path);
    }

    #[test]
    fn restart_recovery_observes_applied_state_without_replay() {
        let (m,p,path)=manager(vec![item("agent",false,false)]);
        ready(&m);
        let plan=m.create_plan(OWNER,"s",1,&[StartupDecision{item_id:"agent".into(),decision:RecommendationDecision::Disable}],false).unwrap();
        let action=m.engine.startup_actions(&plan.id).unwrap().into_iter().next().unwrap();
        p.states.lock().unwrap().insert(action.item_id.clone(),action.applied_state_json.clone());
        let now=now_ms();
        m.db.upsert_startup_change(&StartupChangeRecord{change_id:action.change_id.clone(),origin_change_id:action.change_id.clone(),plan_id:plan.id.clone(),item_id:action.item_id.clone(),kind:action.startup_kind.clone(),display_name:action.display_name.clone(),direction:"Disable".into(),original_json:action.original_state_json.clone(),applied_json:action.applied_state_json.clone(),state:"Prepared".into(),detail:String::new(),created_unix_ms:now,updated_unix_ms:now,restored_unix_ms:None}).unwrap();
        m.recover_incomplete().unwrap();
        let rec=m.db.get_startup_change(&action.change_id).unwrap().unwrap();
        assert_eq!(rec.state,"AppliedRecovered");
        assert_eq!(p.mutations.load(Ordering::SeqCst),0);
        drop(m);
        let _=std::fs::remove_file(path);
    }

    #[test]
    fn snapshot_is_principal_bound() {
        let (m,_,path)=manager(vec![item("a",false,false)]);
        ready(&m);
        assert!(m.snapshot_for_owner(OWNER).is_ok());
        assert!(matches!(m.snapshot_for_owner("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),Err(StartupError::OwnershipMismatch)));
        drop(m); let _=std::fs::remove_file(path);
    }

    #[test]
    fn service_disable_target_is_manual_without_stop_semantics() {
        let original=NativeState::Service{service_name:"vendor".into(),start_type:2,delayed_auto:true,service_type:16,binary_path:r"C:\Vendor\agent.exe".into(),launch_protected:0};
        let target=disabled_state(&original,Path::new("."),"change").unwrap();
        match target {
            NativeState::Service{start_type,delayed_auto,..}=>{assert_eq!(start_type,3);assert!(!delayed_auto);}
            _=>panic!("expected service state"),
        }
    }

    #[test]
    fn disable_then_restore_preserves_auditable_change_chain() {
        let (m,p,path)=manager(vec![item("agent",false,false)]);
        ready(&m);
        let disable=m.create_plan(OWNER,"s",1,&[StartupDecision{item_id:"agent".into(),decision:RecommendationDecision::Disable}],false).unwrap();
        let disable_action=m.engine.startup_actions(&disable.id).unwrap().into_iter().next().unwrap();
        approve(&m.engine,&disable);
        m.engine.consume_authorization_and_begin(&disable.id,OWNER,"startup_test_authorized").unwrap();
        execute_plan(&m.engine,&m.db,p.as_ref(),&disable.id).unwrap();
        assert_eq!(m.db.get_startup_change(&disable_action.change_id).unwrap().unwrap().state,"Applied");

        let restore=m.create_restore_plan(OWNER,&disable_action.change_id,false).unwrap();
        let restore_action=m.engine.startup_actions(&restore.id).unwrap().into_iter().next().unwrap();
        assert_ne!(restore_action.change_id,disable_action.change_id);
        approve(&m.engine,&restore);
        m.engine.consume_authorization_and_begin(&restore.id,OWNER,"startup_restore_test_authorized").unwrap();
        execute_plan(&m.engine,&m.db,p.as_ref(),&restore.id).unwrap();

        let source=m.db.get_startup_change(&disable_action.change_id).unwrap().unwrap();
        let restore_record=m.db.get_startup_change(&restore_action.change_id).unwrap().unwrap();
        assert_eq!(source.state,"Restored");
        assert_eq!(restore_record.state,"Restored");
        assert_eq!(restore_record.origin_change_id,disable_action.change_id);
        assert!(source.restored_unix_ms.is_some());
        drop(m);
        let _=std::fs::remove_file(path);
    }
}
