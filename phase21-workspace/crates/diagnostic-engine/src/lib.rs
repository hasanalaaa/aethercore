#![forbid(unsafe_code)]

use std::{panic::{catch_unwind, AssertUnwindSafe}, sync::{Arc, Mutex}, thread, time::Duration};

use aethercore_collector_runtime::{
    run_isolated_gated, run_isolated_gated_with_token, CancellationToken, CollectorControl,
    CollectorFault, CollectorFaultRecord, CommitFence, FaultKind, IsolationGate,
};
use aethercore_crash_diagnostics::{CrashDiagnosticsSnapshot, CrashError, CrashRecord, EventEvidence};
use aethercore_hardware_telemetry::{HardwareTelemetrySnapshot, MemoryTelemetry, StorageDeviceTelemetry, TelemetryError};
use aethercore_operation_kernel::{ReadBudgetLease, ReadWorkload};
use aethercore_persistence::{Database, DiagnosticSnapshotRecord};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum DiagnosticError {
    #[error("diagnostic scan already running")]
    Busy,
    #[error("diagnostic state belongs to another Windows principal")]
    OwnershipMismatch,
    #[error("diagnostic persistence failed: {0}")]
    Persistence(String),
    #[error("diagnostic scan was cancelled before publication")]
    Cancelled,
    #[error("diagnostic provider failed: {0}")]
    Provider(String),
    #[error("diagnostic engine internal failure: {0}")]
    Internal(String),
}
pub type Result<T> = std::result::Result<T, DiagnosticError>;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScanState { Idle, Collecting, Ready, Partial, Failed }
impl ScanState { pub fn as_str(self)->&'static str{match self{Self::Idle=>"Idle",Self::Collecting=>"Collecting",Self::Ready=>"Ready",Self::Partial=>"Partial",Self::Failed=>"Failed"}} fn running(self)->bool{self==Self::Collecting} }

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase")]
pub struct DiagnosticCard {
    pub card_id:String,
    pub domain:String,
    pub severity:String,
    pub confidence:String,
    pub title:String,
    pub summary:String,
    pub evidence:Vec<String>,
    pub actions:Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase")]
pub struct ProviderFaultRecord {
    pub provider:String,
    pub kind:String,
    pub operation:String,
    pub detail:String,
}

impl From<&CollectorFault> for ProviderFaultRecord {
    fn from(fault: &CollectorFault) -> Self {
        Self {
            provider: fault.provider.into(),
            kind: format!("{:?}", fault.kind),
            operation: fault.operation.into(),
            detail: fault.detail.clone(),
        }
    }
}

impl From<&CollectorFaultRecord> for ProviderFaultRecord {
    fn from(fault: &CollectorFaultRecord) -> Self {
        Self {
            provider: fault.provider.clone(),
            kind: format!("{:?}", fault.kind),
            operation: fault.operation.clone(),
            detail: fault.detail.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase")]
pub struct DiagnosticsSnapshot {
    pub scan_id:String,
    pub state:ScanState,
    pub started_unix_ms:i64,
    pub completed_unix_ms:i64,
    #[serde(default)]
    pub event_window_days:u32,
    pub storage:Vec<StorageDeviceTelemetry>,
    pub memory:Option<MemoryTelemetry>,
    pub events:Vec<EventEvidence>,
    pub crashes:Vec<CrashRecord>,
    pub cards:Vec<DiagnosticCard>,
    #[serde(default)]
    pub provider_faults:Vec<ProviderFaultRecord>,
    pub warnings:Vec<String>,
}
impl Default for DiagnosticsSnapshot{fn default()->Self{Self{scan_id:String::new(),state:ScanState::Idle,started_unix_ms:0,completed_unix_ms:0,event_window_days:0,storage:vec![],memory:None,events:vec![],crashes:vec![],cards:vec![],provider_faults:vec![],warnings:vec![]}}}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase")]
// DBT-P46-B4: card_count is Option because a stored snapshot's JSON can fail to
// parse (corrupted, or written by an incompatible schema version). None means
// "could not be determined"; Some(0) means a scan that really did produce zero
// cards. The wire carries the same distinction as has_card_count/card_count.
pub struct DiagnosticHistoryEntry { pub scan_id:String,pub state:String,pub collected_unix_ms:i64,pub warning_count:u32,pub card_count:Option<u32> }

pub trait Backend:Send+Sync+'static{
    fn hardware(&self, control: CollectorControl)->std::result::Result<HardwareTelemetrySnapshot,CollectorFault>;
    fn crashes(&self, control: CollectorControl)->std::result::Result<CrashDiagnosticsSnapshot,CollectorFault>;
}
#[derive(Default)] pub struct WindowsBackend;
impl Backend for WindowsBackend{
    fn hardware(&self, control: CollectorControl)->std::result::Result<HardwareTelemetrySnapshot,CollectorFault>{
        aethercore_hardware_telemetry::collect_with_cancellation(control.cancellation()).map_err(|error| {
            let kind = match &error {
                TelemetryError::Timeout(_) => FaultKind::Timeout,
                TelemetryError::Cancelled(_) => FaultKind::Cancelled,
                TelemetryError::Unavailable(_) => FaultKind::Unavailable,
                TelemetryError::PermissionDenied(_) => FaultKind::PermissionDenied,
                TelemetryError::MalformedResponse(_) => FaultKind::MalformedResponse,
                TelemetryError::Windows(_) => FaultKind::ProviderFailure,
            };
            CollectorFault::new("hardware-telemetry", "collect", kind, error.to_string())
        })
    }
    fn crashes(&self, control: CollectorControl)->std::result::Result<CrashDiagnosticsSnapshot,CollectorFault>{
        aethercore_crash_diagnostics::collect_with_cancellation(control.cancellation()).map_err(|error| {
            let kind = match &error {
                CrashError::Timeout(_) => FaultKind::Timeout,
                CrashError::Cancelled(_) => FaultKind::Cancelled,
                CrashError::Unavailable(_) => FaultKind::Unavailable,
                CrashError::PermissionDenied(_) => FaultKind::PermissionDenied,
                CrashError::MalformedResponse(_) => FaultKind::MalformedResponse,
                CrashError::Io(_) => FaultKind::Io,
                CrashError::Windows(_) => FaultKind::ProviderFailure,
            };
            CollectorFault::new("crash-diagnostics", "collect", kind, error.to_string())
        })
    }
}

#[derive(Clone)] pub struct DiagnosticEngine{inner:Arc<Inner>}
struct Inner{backend:Arc<dyn Backend>,db:Arc<Database>,snapshot:Mutex<DiagnosticsSnapshot>,owner_principal_key:Mutex<String>,hardware_gate:IsolationGate,crash_gate:IsolationGate}

impl DiagnosticEngine{
    pub fn new(db:Arc<Database>)->Self{Self::with_backend(db,Arc::new(WindowsBackend))}
    pub fn with_backend(db:Arc<Database>,backend:Arc<dyn Backend>)->Self{Self{inner:Arc::new(Inner{backend,db,snapshot:Mutex::new(DiagnosticsSnapshot::default()),owner_principal_key:Mutex::new(String::new()),hardware_gate:IsolationGate::default(),crash_gate:IsolationGate::default()})}}
    pub fn snapshot(&self)->DiagnosticsSnapshot{self.inner.snapshot.lock().unwrap_or_else(|p|p.into_inner()).clone()}
    pub fn start_scan_with_lease(&self,owner_principal_key:&str,lease:ReadBudgetLease)->Result<DiagnosticsSnapshot>{
        if !lease.matches(ReadWorkload::Diagnostics){return Err(DiagnosticError::Internal("read budget lease identity mismatch".into()))}
        self.start_scan_inner(owner_principal_key,lease)
    }
    fn start_scan_inner(&self,owner_principal_key:&str,read_budget_lease:ReadBudgetLease)->Result<DiagnosticsSnapshot>{
        {
            // Keep owner + snapshot publication atomic to readers by taking locks in the same
            // owner-then-snapshot order used by snapshot_for_owner.
            let mut owner=self.inner.owner_principal_key.lock().unwrap_or_else(|p|p.into_inner());
            let mut s=self.inner.snapshot.lock().unwrap_or_else(|p|p.into_inner());
            if s.state.running(){return Err(DiagnosticError::Busy)}
            *owner=owner_principal_key.to_owned();
            *s=DiagnosticsSnapshot{scan_id:Uuid::new_v4().to_string(),state:ScanState::Collecting,started_unix_ms:Utc::now().timestamp_millis(),..Default::default()};
        }
        let owner=owner_principal_key.to_owned();
        let inner=self.inner.clone();
        let worker_inner=inner.clone();
        if let Err(error)=thread::Builder::new().name("aether-diagnostic-scan".into()).spawn(move||{
            let _read_budget_lease=read_budget_lease;
            let recovery_inner=worker_inner.clone();
            if catch_unwind(AssertUnwindSafe(|| run(worker_inner,owner))).is_err() {
                mark_scan_runtime_failure(
                    &recovery_inner,
                    "scan.run",
                    "diagnostic scan worker panicked outside a contained provider boundary",
                );
            }
        }) {
            mark_scan_runtime_failure(&self.inner,"scan.spawn",error.to_string());
            return Err(DiagnosticError::Internal(error.to_string()));
        }
        Ok(self.snapshot_for_owner(owner_principal_key)?)
    }
    pub fn passive_hardware_refresh(&self,owner_principal_key:&str,token:CancellationToken,commit_fence:CommitFence)->Result<DiagnosticsSnapshot>{
        if owner_principal_key.trim().is_empty()||token.is_cancelled(){return Err(DiagnosticError::Cancelled)}
        {let snapshot=self.inner.snapshot.lock().unwrap_or_else(|p|p.into_inner());if snapshot.state.running(){return Err(DiagnosticError::Busy)}}
        let backend=self.inner.backend.clone();let gate=self.inner.hardware_gate.clone();let child=token.child();
        let hardware=run_isolated_gated_with_token(&gate,"diagnostic-engine","passive-hardware",Duration::from_secs(10),child,move|control|backend.hardware(control))
            .map_err(|fault|if matches!(fault.kind,FaultKind::Cancelled){DiagnosticError::Cancelled}else{DiagnosticError::Provider(fault.to_string())})?;
        if token.is_cancelled(){return Err(DiagnosticError::Cancelled)}
        let now=Utc::now().timestamp_millis();
        let mut next={
            let owner=self.inner.owner_principal_key.lock().unwrap_or_else(|p|p.into_inner());
            let current=self.inner.snapshot.lock().unwrap_or_else(|p|p.into_inner());
            if owner.as_str()==owner_principal_key&&!current.state.running(){current.clone()}else{DiagnosticsSnapshot::default()}
        };
        next.scan_id=Uuid::new_v4().to_string();next.started_unix_ms=now;next.completed_unix_ms=now;
        next.storage=hardware.storage.clone();next.memory=hardware.memory.clone();
        next.provider_faults.retain(|fault|!is_hardware_fault_provider(&fault.provider));
        next.provider_faults.extend(hardware.provider_faults.iter().map(ProviderFaultRecord::from));
        next.warnings=hardware.warnings.clone();
        let hardware_available=!next.storage.is_empty()||next.memory.is_some();
        let event_available=!next.events.is_empty()||!next.crashes.is_empty()||next.event_window_days>0;
        next.cards=build_cards_with_availability(&next.storage,next.memory.as_ref(),&next.events,&next.crashes,event_available,next.event_window_days);
        next.state=passive_state(&next,hardware_available,event_available);
        let committed=commit_fence.try_commit_checked(||{
            if token.is_cancelled(){return None}
            let mut owner=self.inner.owner_principal_key.lock().unwrap_or_else(|p|p.into_inner());
            let mut current=self.inner.snapshot.lock().unwrap_or_else(|p|p.into_inner());
            if current.state.running(){return None}
            *owner=owner_principal_key.to_owned();*current=next.clone();Some(())
        });
        if committed!=Some(()){return Err(DiagnosticError::Cancelled)}
        Ok(next)
    }

    pub fn passive_event_log_refresh(&self,owner_principal_key:&str,token:CancellationToken,commit_fence:CommitFence)->Result<DiagnosticsSnapshot>{
        if owner_principal_key.trim().is_empty()||token.is_cancelled(){return Err(DiagnosticError::Cancelled)}
        {let snapshot=self.inner.snapshot.lock().unwrap_or_else(|p|p.into_inner());if snapshot.state.running(){return Err(DiagnosticError::Busy)}}
        let backend=self.inner.backend.clone();let gate=self.inner.crash_gate.clone();let child=token.child();
        let crash=run_isolated_gated_with_token(&gate,"diagnostic-engine","passive-eventlog",Duration::from_secs(10),child,move|control|backend.crashes(control))
            .map_err(|fault|if matches!(fault.kind,FaultKind::Cancelled){DiagnosticError::Cancelled}else{DiagnosticError::Provider(fault.to_string())})?;
        if token.is_cancelled(){return Err(DiagnosticError::Cancelled)}
        let now=Utc::now().timestamp_millis();
        let mut next={
            let owner=self.inner.owner_principal_key.lock().unwrap_or_else(|p|p.into_inner());
            let current=self.inner.snapshot.lock().unwrap_or_else(|p|p.into_inner());
            if owner.as_str()==owner_principal_key&&!current.state.running(){current.clone()}else{DiagnosticsSnapshot::default()}
        };
        next.scan_id=Uuid::new_v4().to_string();next.started_unix_ms=now;next.completed_unix_ms=now;
        next.event_window_days=if crash.event_window_days==0{aethercore_crash_diagnostics::DEFAULT_EVENT_WINDOW_DAYS}else{crash.event_window_days};
        next.events=crash.events.clone();next.crashes=crash.crashes.clone();
        next.provider_faults.retain(|fault|is_hardware_fault_provider(&fault.provider));
        next.provider_faults.extend(crash.provider_faults.iter().map(ProviderFaultRecord::from));
        // Hardware warnings are intentionally not carried as unstructured prose across refreshes;
        // provider faults and typed evidence remain preserved, while the newly refreshed provider
        // owns the current warning list.
        next.warnings=crash.warnings.clone();
        let hardware_available=!next.storage.is_empty()||next.memory.is_some();
        next.cards=build_cards_with_availability(&next.storage,next.memory.as_ref(),&next.events,&next.crashes,true,next.event_window_days);
        next.state=passive_state(&next,hardware_available,true);
        let committed=commit_fence.try_commit_checked(||{
            if token.is_cancelled(){return None}
            let mut owner=self.inner.owner_principal_key.lock().unwrap_or_else(|p|p.into_inner());
            let mut current=self.inner.snapshot.lock().unwrap_or_else(|p|p.into_inner());
            if current.state.running(){return None}
            *owner=owner_principal_key.to_owned();*current=next.clone();Some(())
        });
        if committed!=Some(()){return Err(DiagnosticError::Cancelled)}
        Ok(next)
    }

    pub fn snapshot_for_owner(&self,owner_principal_key:&str)->Result<DiagnosticsSnapshot>{let current=self.inner.owner_principal_key.lock().unwrap_or_else(|p|p.into_inner());if current.as_str()!=owner_principal_key{return Err(DiagnosticError::OwnershipMismatch)}let snapshot=self.inner.snapshot.lock().unwrap_or_else(|p|p.into_inner()).clone();drop(current);Ok(snapshot)}
    pub fn history(&self,owner_principal_key:&str,limit:usize)->Result<Vec<DiagnosticHistoryEntry>>{self.inner.db.diagnostic_snapshots_for_owner(owner_principal_key,limit).map_err(|e|DiagnosticError::Persistence(e.to_string())).map(|rows|rows.into_iter().map(|r|{let card_count=serde_json::from_str::<DiagnosticsSnapshot>(&r.snapshot_json).ok().map(|s|s.cards.len() as u32);DiagnosticHistoryEntry{scan_id:r.snapshot_id,state:r.state,collected_unix_ms:r.collected_unix_ms,warning_count:r.warning_count,card_count}}).collect())}
}

fn is_hardware_fault_provider(provider:&str)->bool{
    provider.contains("hardware")||provider.contains("storage")||provider.contains("wmi")||provider.contains("nvme")||provider.contains("smart")
}

fn passive_state(snapshot:&DiagnosticsSnapshot,hardware_available:bool,event_available:bool)->ScanState{
    if !hardware_available&&!event_available{
        return if snapshot.provider_faults.is_empty(){ScanState::Partial}else{ScanState::Failed};
    }
    if !hardware_available||!event_available{return ScanState::Partial;}
    if snapshot.provider_faults.is_empty()&&snapshot.warnings.is_empty(){ScanState::Ready}else{ScanState::Partial}
}

fn mark_scan_runtime_failure(inner:&Arc<Inner>,operation:&'static str,detail:impl Into<String>){
    let mut snapshot=inner.snapshot.lock().unwrap_or_else(|p|p.into_inner());
    if !snapshot.state.running(){return;}
    snapshot.state=ScanState::Failed;
    snapshot.completed_unix_ms=Utc::now().timestamp_millis();
    let fault=CollectorFault::new("diagnostic-engine",operation,FaultKind::Internal,detail);
    snapshot.provider_faults.push(ProviderFaultRecord::from(&fault));
    snapshot.warnings.push("Diagnostic scan stopped because an internal worker boundary failed.".into());
}

fn join_provider<T>(
    worker:std::io::Result<thread::JoinHandle<std::result::Result<T,CollectorFault>>>,
    operation:&'static str,
)->std::result::Result<T,CollectorFault>{
    match worker{
        Ok(handle)=>handle.join().unwrap_or_else(|_|Err(CollectorFault::new(
            "diagnostic-engine",operation,FaultKind::Internal,"provider supervisor thread terminated unexpectedly",
        ))),
        Err(error)=>Err(CollectorFault::new(
            "diagnostic-engine",operation,FaultKind::Internal,format!("failed to spawn provider supervisor thread: {error}"),
        )),
    }
}

fn run(inner:Arc<Inner>,owner_principal_key:String){
    const PROVIDER_WATCHDOG: Duration = Duration::from_secs(10);
    let scan_id={inner.snapshot.lock().unwrap_or_else(|p|p.into_inner()).scan_id.clone()};

    let hardware_backend = inner.backend.clone();
    let crash_backend = inner.backend.clone();
    let hardware_gate = inner.hardware_gate.clone();
    let crash_gate = inner.crash_gate.clone();
    // Fan out independent providers. Each branch owns a persistent isolation gate: if a platform
    // provider outlives its watchdog, later scans fail that branch fast until the original worker
    // actually exits, rather than accumulating detached/hung provider threads.
    let hardware_worker = thread::Builder::new().name("aether-diagnostic-hardware".into()).spawn(move || {
        run_isolated_gated(&hardware_gate, "diagnostic-engine", "hardware-provider", PROVIDER_WATCHDOG, move |control| hardware_backend.hardware(control))
    });
    let crash_worker = thread::Builder::new().name("aether-diagnostic-crash".into()).spawn(move || {
        run_isolated_gated(&crash_gate, "diagnostic-engine", "crash-provider", PROVIDER_WATCHDOG, move |control| crash_backend.crashes(control))
    });

    let hardware_result = join_provider(hardware_worker,"hardware-provider");
    let crash_result = join_provider(crash_worker,"crash-provider");

    let mut warnings=Vec::new();
    let mut provider_faults=Vec::new();
    let hardware=match hardware_result{
        Ok(v)=>Some(v),
        Err(fault)=>{warnings.push(format!("Hardware telemetry: {fault}"));provider_faults.push(ProviderFaultRecord::from(&fault));None}
    };
    let crash=match crash_result{
        Ok(v)=>Some(v),
        Err(fault)=>{warnings.push(format!("Crash diagnostics: {fault}"));provider_faults.push(ProviderFaultRecord::from(&fault));None}
    };
    if let Some(h)=&hardware{
        warnings.extend(h.warnings.clone());
        provider_faults.extend(h.provider_faults.iter().map(ProviderFaultRecord::from));
    }
    if let Some(c)=&crash{
        warnings.extend(c.warnings.clone());
        provider_faults.extend(c.provider_faults.iter().map(ProviderFaultRecord::from));
    }
    let storage=hardware.as_ref().map(|h|h.storage.clone()).unwrap_or_default();
    let memory=hardware.as_ref().and_then(|h|h.memory.clone());
    let event_window_days=crash.as_ref().map(|c| if c.event_window_days == 0 { aethercore_crash_diagnostics::DEFAULT_EVENT_WINDOW_DAYS } else { c.event_window_days }).unwrap_or(aethercore_crash_diagnostics::DEFAULT_EVENT_WINDOW_DAYS);
    let events=crash.as_ref().map(|c|c.events.clone()).unwrap_or_default();
    let crashes=crash.as_ref().map(|c|c.crashes.clone()).unwrap_or_default();
    let cards=build_cards_with_availability(&storage,memory.as_ref(),&events,&crashes,crash.is_some(),event_window_days);
    let state=match (hardware.is_some(),crash.is_some()){(true,true)=>if warnings.is_empty(){ScanState::Ready}else{ScanState::Partial},(false,false)=>ScanState::Failed,_=>ScanState::Partial};
    let completed=Utc::now().timestamp_millis();
    let mut snapshot=DiagnosticsSnapshot{scan_id:scan_id.clone(),state,started_unix_ms:inner.snapshot.lock().unwrap_or_else(|p|p.into_inner()).started_unix_ms,completed_unix_ms:completed,event_window_days,storage,memory,events,crashes,cards,provider_faults,warnings};
    let persistence_result = serde_json::to_string(&snapshot)
        .map_err(|error| (FaultKind::Internal, format!("diagnostic snapshot serialization failed: {error}")))
        .and_then(|json| inner.db.save_diagnostic_snapshot(&DiagnosticSnapshotRecord{
            snapshot_id:scan_id,
            owner_principal_key,
            state:state.as_str().into(),
            collected_unix_ms:completed,
            warning_count:snapshot.warnings.len() as u32,
            snapshot_json:json,
        }).map_err(|error| (FaultKind::Io, format!("diagnostic snapshot persistence failed: {error}"))));
    if let Err((kind,detail))=persistence_result {
        // A scan that could not reach the durable ledger is never presented as fully Ready.
        // The raw storage/serialization detail is preserved only as technical provider evidence.
        if snapshot.state==ScanState::Ready { snapshot.state=ScanState::Partial; }
        let fault=CollectorFault::new("diagnostic-journal","snapshot.persist",kind,detail);
        snapshot.provider_faults.push(ProviderFaultRecord::from(&fault));
        snapshot.warnings.push("Diagnostic history persistence was unavailable; this scan remains visible in memory but was not added to durable history.".into());
    }
    *inner.snapshot.lock().unwrap_or_else(|p|p.into_inner())=snapshot;
}

pub fn build_cards(
    storage: &[StorageDeviceTelemetry],
    memory: &MemoryTelemetry,
    events: &[EventEvidence],
    crashes: &[CrashRecord],
) -> Vec<DiagnosticCard> {
    build_cards_with_availability(storage, Some(memory), events, crashes, true, aethercore_crash_diagnostics::DEFAULT_EVENT_WINDOW_DAYS)
}

fn build_cards_with_availability(
    storage: &[StorageDeviceTelemetry],
    memory: Option<&MemoryTelemetry>,
    events: &[EventEvidence],
    crashes: &[CrashRecord],
    event_evidence_available: bool,
    event_window_days: u32,
) -> Vec<DiagnosticCard> {
    let mut cards = Vec::new();

    for d in storage {
        if matches!(d.severity.as_str(), "ActionRequired" | "Attention") {
            cards.push(DiagnosticCard {
                card_id: format!("storage:{}", d.device_id),
                domain: "Storage".into(),
                severity: d.severity.clone(),
                confidence: "ReportedMetricEvidence".into(),
                title: format!(
                    "{} — storage reliability",
                    if d.friendly_name.is_empty() { "Physical disk" } else { &d.friendly_name }
                ),
                summary: d.summary.clone(),
                evidence: d.reasons.clone(),
                actions: vec![
                    "Back up important files immediately if errors or critical warnings are present.".into(),
                    "Avoid unnecessary heavy write workloads until the device is checked.".into(),
                    "Run the drive manufacturer's diagnostic utility and review firmware/support guidance.".into(),
                    "Replace the drive if reliability errors persist or increase.".into(),
                ],
            });
        }
    }

    let mem_events = events
        .iter()
        .filter(|e| e.category == "MemoryHardwareEvidence")
        .collect::<Vec<_>>();
    if !event_evidence_available {
        cards.push(DiagnosticCard {
            card_id: "memory:whea-unavailable".into(),
            domain: "Memory".into(),
            severity: "Unknown".into(),
            confidence: "EvidenceUnavailable".into(),
            title: "Memory hardware-event evidence is unavailable".into(),
            summary: "The WHEA/Event Log collector did not complete, so AetherCore cannot state whether memory-related hardware errors were logged in this scan.".into(),
            evidence: Vec::new(),
            actions: vec![
                "Retry diagnostics after confirming Windows Event Log access.".into(),
                "Use Windows Memory Diagnostic if symptoms independently suggest memory instability.".into(),
            ],
        });
    } else if !mem_events.is_empty() {
        cards.push(DiagnosticCard {
            card_id: "memory:whea".into(),
            domain: "Memory".into(),
            severity: "Attention".into(),
            confidence: "LoggedHardwareEvidence".into(),
            title: "Memory-related hardware errors were logged".into(),
            summary: format!(
                "{} WHEA event(s) contained memory-related hardware-error evidence in the last {} days.",
                mem_events.len(), event_window_days
            ),
            evidence: mem_events
                .iter()
                .take(4)
                .map(|e| format!("WHEA event {}: {}", e.event_id, e.detail))
                .collect(),
            actions: vec![
                "Run Windows Memory Diagnostic at reboot for an offline test.".into(),
                "If diagnosing instability, remove overclock/XMP/EXPO variables before retesting.".into(),
                "If errors recur, test DIMMs individually and follow the system/vendor hardware service procedure.".into(),
            ],
        });
    } else {
        let mut evidence = Vec::new();
        if let Some(memory) = memory {
            if !memory.pressure_explanation.is_empty() {
                evidence.push(memory.pressure_explanation.clone());
            }
        }
        cards.push(DiagnosticCard {
            card_id: "memory:no-logged-errors".into(),
            domain: "Memory".into(),
            severity: "Info".into(),
            confidence: "LogWindowOnly".into(),
            title: "No logged memory hardware errors were found".into(),
            summary: format!("No memory-related WHEA evidence was found in the last {} days. This does not prove RAM is fault-free.", event_window_days),
            evidence,
            actions: vec![
                "Use Windows Memory Diagnostic if you are troubleshooting suspected memory instability.".into(),
            ],
        });
    }

    if let Some(memory) = memory {
        if memory.memory_load_percent >= 80 {
            cards.push(DiagnosticCard {
                card_id: "memory:pressure".into(),
                domain: "Memory".into(),
                severity: if memory.memory_load_percent >= 90 { "Attention" } else { "Info" }.into(),
                confidence: "CurrentOSMetric".into(),
                title: "Current memory pressure".into(),
                summary: memory.pressure_explanation.clone(),
                evidence: vec![format!(
                    "Available physical memory: {} of {} bytes",
                    memory.available_physical_bytes, memory.total_physical_bytes
                )],
                actions: vec![
                    "Close or inspect memory-heavy applications if performance is currently affected.".into(),
                ],
            });
        }
    }

    for e in events
        .iter()
        .filter(|e| {
            e.category == "HardwareError"
                || e.category == "ProcessorHardwareEvidence"
                || e.category == "PcieHardwareEvidence"
        })
        .take(8)
    {
        cards.push(DiagnosticCard {
            card_id: format!("event:{}:{}", e.provider, e.recorded_unix_ms),
            domain: "Hardware".into(),
            severity: e.severity.clone(),
            confidence: e.confidence.clone(),
            title: e.summary.clone(),
            summary: e.detail.clone(),
            evidence: vec![format!("{} event {}", e.provider, e.event_id)],
            actions: vec![
                "Review recent hardware/firmware/driver changes and vendor diagnostics before replacing components.".into(),
            ],
        });
    }

    if let Some(c) = crashes.first() {
        let nearby_whea = events
            .iter()
            .filter(|e| e.provider.eq_ignore_ascii_case("Microsoft-Windows-WHEA-Logger"))
            // DBT-P46-B6: a dump whose mtime could not be read has no time to
            // correlate against, so it correlates with nothing — rather than
            // being compared against a fabricated epoch-0 timestamp.
            .filter(|e| c.recorded_unix_ms.is_some_and(|crash_ms| (e.recorded_unix_ms - crash_ms).abs() <= 10 * 60 * 1000))
            .collect::<Vec<_>>();
        let mut evidence = vec![
            format!("Dump: {}", c.dump_file),
            if c.bugcheck_hex.is_empty() {
                "Bugcheck code not available from lightweight header parsing.".into()
            } else {
                format!("Bugcheck: {}", c.bugcheck_hex)
            },
        ];
        if !nearby_whea.is_empty() {
            evidence.push(format!(
                "{} WHEA event(s) were logged within ±10 minutes of the dump timestamp; this is correlation, not proof of causation.",
                nearby_whea.len()
            ));
        }
        cards.push(DiagnosticCard {
            card_id: "crash:recent".into(),
            domain: "Crash".into(),
            severity: "Attention".into(),
            confidence: c.confidence.clone(),
            title: "Recent Windows crash dump detected".into(),
            summary: c.summary.clone(),
            evidence,
            actions: vec![
                "Correlate the crash time with WHEA, driver and Windows Error Reporting events.".into(),
                "For module-level attribution, analyze the dump with matching Microsoft symbols; AetherCore does not infer a culprit from the filename alone.".into(),
            ],
        });
    } else if events.iter().any(|e| e.category == "BugcheckReport") {
        cards.push(DiagnosticCard {
            card_id: "crash:wer-report".into(),
            domain: "Crash".into(),
            severity: "Attention".into(),
            confidence: "LoggedCrashEvidence".into(),
            title: "Windows recorded a system crash".into(),
            summary: "Windows Error Reporting contains bugcheck/crash evidence, but no recent minidump metadata was available to the lightweight collector.".into(),
            evidence: vec!["Microsoft-Windows-WER-SystemErrorReporting event present.".into()],
            actions: vec![
                "Check crash-dump configuration and correlate the event time with WHEA and recent driver changes.".into(),
                "Do not assign a driver or hardware culprit without additional dump/event evidence.".into(),
            ],
        });
    } else if events.iter().any(|e| e.category == "UnexpectedShutdown") {
        cards.push(DiagnosticCard {
            card_id: "crash:unexpected-shutdown".into(),
            domain: "Crash".into(),
            severity: "Attention".into(),
            confidence: "EventHigh/CauseLow".into(),
            title: "Unexpected shutdown recorded".into(),
            summary: "Kernel-Power evidence confirms an unclean shutdown, but no recent dump metadata was found by the lightweight collector.".into(),
            evidence: vec!["Kernel-Power Event 41 present.".into()],
            actions: vec![
                "Check power delivery, thermal stability, WHEA events, and crash-dump configuration before drawing a root-cause conclusion.".into(),
            ],
        });
    }

    cards
}

#[cfg(test)]
mod tests{
    use super::*;use aethercore_hardware_telemetry::{HardwareTelemetrySnapshot,StorageDeviceTelemetry};use aethercore_crash_diagnostics::CrashDiagnosticsSnapshot;use aethercore_operation_kernel::ReadBudgetManager;
    const OWNER:&str="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    fn start_scan_leased(engine:&DiagnosticEngine,owner:&str)->Result<DiagnosticsSnapshot>{let budget=ReadBudgetManager::new(4);let lease=budget.try_acquire(ReadWorkload::Diagnostics).expect("read budget lease");engine.start_scan_with_lease(owner,lease)}
    struct Mock{h:HardwareTelemetrySnapshot,c:CrashDiagnosticsSnapshot}impl Backend for Mock{fn hardware(&self,_control:CollectorControl)->std::result::Result<HardwareTelemetrySnapshot,CollectorFault>{Ok(self.h.clone())}fn crashes(&self,_control:CollectorControl)->std::result::Result<CrashDiagnosticsSnapshot,CollectorFault>{Ok(self.c.clone())}}
    struct PartialMock; impl Backend for PartialMock { fn hardware(&self,_control:CollectorControl)->std::result::Result<HardwareTelemetrySnapshot,CollectorFault>{Err(CollectorFault::new("mock-hardware","collect",FaultKind::Unavailable,"storage provider unavailable"))} fn crashes(&self,_control:CollectorControl)->std::result::Result<CrashDiagnosticsSnapshot,CollectorFault>{Ok(CrashDiagnosticsSnapshot::default())} }
    struct CrashUnavailableMock; impl Backend for CrashUnavailableMock { fn hardware(&self,_control:CollectorControl)->std::result::Result<HardwareTelemetrySnapshot,CollectorFault>{Ok(HardwareTelemetrySnapshot::default())} fn crashes(&self,_control:CollectorControl)->std::result::Result<CrashDiagnosticsSnapshot,CollectorFault>{Err(CollectorFault::new("mock-crash","collect",FaultKind::Unavailable,"crash provider unavailable"))} }
    // DBT-P42-013: this handed back a PathBuf that 7 of the 8 callers removed by
    // hand at the end of the test — leaving the 8th, plus every panicking run,
    // plus the sqlite -wal/-shm sidecars, behind in %TEMP%. The whole directory
    // now dies with the test.
    fn db()->(Arc<Database>,tempfile::TempDir){let dir=tempfile::tempdir().expect("temp dir");let p=dir.path().join(format!("aethercore-diag-{}.db",Uuid::new_v4()));(Arc::new(Database::open(&p).unwrap()),dir)}
    #[test]fn no_whea_never_becomes_ram_healthy(){let cards=build_cards(&[],&MemoryTelemetry::default(),&[],&[]);let c=cards.iter().find(|c|c.card_id=="memory:no-logged-errors").unwrap();assert!(c.summary.contains("does not prove RAM is fault-free"));}
    #[test]fn critical_storage_card_uses_backup_first_guidance(){let mut d=StorageDeviceTelemetry{device_id:"0".into(),friendly_name:"Disk".into(),severity:"ActionRequired".into(),summary:"errors".into(),reasons:vec!["uncorrected".into()],..Default::default()};let cards=build_cards(&[d.clone()],&MemoryTelemetry::default(),&[],&[]);assert!(cards[0].actions[0].contains("Back up"));d.severity="Normal".into();assert!(!build_cards(&[d],&MemoryTelemetry::default(),&[],&[]).iter().any(|c|c.domain=="Storage"));}
    #[test]fn scan_persists_history(){let(db,_tmp)=db();let e=DiagnosticEngine::with_backend(db.clone(),Arc::new(Mock{h:HardwareTelemetrySnapshot::default(),c:CrashDiagnosticsSnapshot::default()}));start_scan_leased(&e,OWNER).unwrap();for _ in 0..100{if !e.snapshot().state.running(){break}std::thread::sleep(std::time::Duration::from_millis(10));}assert!(!e.history(OWNER,10).unwrap().is_empty());drop(e);drop(db);}    #[test]fn partial_collector_failure_is_honest_and_persisted(){let(db,_tmp)=db();let e=DiagnosticEngine::with_backend(db.clone(),Arc::new(PartialMock));start_scan_leased(&e,OWNER).unwrap();for _ in 0..100{if !e.snapshot().state.running(){break}std::thread::sleep(std::time::Duration::from_millis(10));}let s=e.snapshot();assert_eq!(s.state,ScanState::Partial);assert!(s.warnings.iter().any(|w|w.contains("Hardware telemetry")));assert!(!e.history(OWNER,10).unwrap().is_empty());drop(e);drop(db);}
    // DBT-P46-B5: when the crash provider fails entirely (crash=None), the live
    // scan path defaulted event_window_days to a bare 0 instead of
    // DEFAULT_EVENT_WINDOW_DAYS — the same constant this file already
    // substitutes two lines up when crash *ran* but reported a 0-day window.
    // "collector didn't run" and "collector ran and reported nothing" must
    // not silently share a value that reads as a real window in UI text
    // ("...in the last 0 days").
    #[test]fn crash_provider_failure_defaults_the_window_not_to_zero(){let(db,_tmp)=db();let e=DiagnosticEngine::with_backend(db.clone(),Arc::new(CrashUnavailableMock));start_scan_leased(&e,OWNER).unwrap();for _ in 0..100{if !e.snapshot().state.running(){break}std::thread::sleep(std::time::Duration::from_millis(10));}let s=e.snapshot();assert_eq!(s.state,ScanState::Partial);assert_eq!(s.event_window_days,aethercore_crash_diagnostics::DEFAULT_EVENT_WINDOW_DAYS,"a failed crash provider must not report a 0-day event window");drop(e);drop(db);}
    // DBT-P46-B4. Hasan's wire-contract decision (§46.15): has_card_count means
    // "the value was determined", NOT "the value is non-zero" — so the case that
    // matters most is a VALID snapshot that genuinely has zero cards vs. a
    // snapshot whose stored JSON cannot be parsed at all. Before the fix both
    // reported card_count: 0 with nothing to tell them apart.
    #[test]fn a_real_zero_card_scan_is_distinguishable_from_an_unparseable_one(){
        let(db,_tmp)=db();
        let e=DiagnosticEngine::with_backend(db.clone(),Arc::new(Mock{h:HardwareTelemetrySnapshot::default(),c:CrashDiagnosticsSnapshot::default()}));
        let empty_but_real=DiagnosticsSnapshot::default();
        db.save_diagnostic_snapshot(&aethercore_persistence::DiagnosticSnapshotRecord{snapshot_id:"real-zero".into(),owner_principal_key:OWNER.into(),state:"Ready".into(),collected_unix_ms:2,warning_count:0,snapshot_json:serde_json::to_string(&empty_but_real).unwrap()}).unwrap();
        db.save_diagnostic_snapshot(&aethercore_persistence::DiagnosticSnapshotRecord{snapshot_id:"unparseable".into(),owner_principal_key:OWNER.into(),state:"Ready".into(),collected_unix_ms:1,warning_count:0,snapshot_json:"{ this is not valid json".into()}).unwrap();
        let entries=e.history(OWNER,10).unwrap();
        let real=entries.iter().find(|x|x.scan_id=="real-zero").expect("the real-zero row");
        let broken=entries.iter().find(|x|x.scan_id=="unparseable").expect("the unparseable row");
        assert_eq!(real.card_count,Some(0),"a scan that genuinely produced zero cards must report a determined zero");
        assert_eq!(broken.card_count,None,"an unparseable stored snapshot must not report zero cards as if it had been measured");
        drop(e);drop(db);
    }
    #[test]fn diagnostic_snapshot_is_principal_bound(){let(db,_tmp)=db();let e=DiagnosticEngine::with_backend(db.clone(),Arc::new(Mock{h:HardwareTelemetrySnapshot::default(),c:CrashDiagnosticsSnapshot::default()}));start_scan_leased(&e,OWNER).unwrap();let other="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";assert!(matches!(e.snapshot_for_owner(other),Err(DiagnosticError::OwnershipMismatch)));drop(e);drop(db);}
    #[test]fn unavailable_event_source_never_becomes_no_logged_errors(){let cards=build_cards_with_availability(&[],Some(&MemoryTelemetry::default()),&[],&[],false,0);assert!(cards.iter().any(|c|c.card_id=="memory:whea-unavailable"));assert!(!cards.iter().any(|c|c.card_id=="memory:no-logged-errors"));}

    #[test]
    fn unavailable_memory_collector_never_becomes_zero_percent_pressure_evidence(){
        let cards=build_cards_with_availability(&[],None,&[],&[],true,aethercore_crash_diagnostics::DEFAULT_EVENT_WINDOW_DAYS);
        let no_errors=cards.iter().find(|c|c.card_id=="memory:no-logged-errors").expect("event-log conclusion should still exist");
        assert!(no_errors.evidence.is_empty(),"unavailable memory telemetry must not create synthetic pressure evidence");
        assert!(!cards.iter().any(|c|c.card_id=="memory:pressure"),"unavailable memory telemetry must not become a zero-percent pressure card");
    }

    struct PanicHardwareMock;
    impl Backend for PanicHardwareMock {
        fn hardware(&self,_control:CollectorControl)->std::result::Result<HardwareTelemetrySnapshot,CollectorFault>{panic!("fault injection") }
        fn crashes(&self,_control:CollectorControl)->std::result::Result<CrashDiagnosticsSnapshot,CollectorFault>{Ok(CrashDiagnosticsSnapshot::default())}
    }

    #[test]
    fn nested_provider_faults_are_preserved_in_the_diagnostic_snapshot(){
        let(db,_tmp)=db();
        let mut hardware=HardwareTelemetrySnapshot::default();
        hardware.provider_faults.push(CollectorFaultRecord::new(
            "hardware-telemetry", "nvme-health-ioctl", FaultKind::MalformedResponse, "fault injection",
        ));
        let mut crashes=CrashDiagnosticsSnapshot::default();
        crashes.provider_faults.push(CollectorFaultRecord::new(
            "crash-diagnostics", "eventlog.render", FaultKind::MalformedResponse, "fault injection",
        ));
        let e=DiagnosticEngine::with_backend(db.clone(),Arc::new(Mock{h:hardware,c:crashes}));
        start_scan_leased(&e,OWNER).unwrap();
        for _ in 0..100{if !e.snapshot().state.running(){break}std::thread::sleep(std::time::Duration::from_millis(10));}
        let s=e.snapshot();
        assert!(s.provider_faults.iter().any(|fault|fault.operation=="nvme-health-ioctl"));
        assert!(s.provider_faults.iter().any(|fault|fault.operation=="eventlog.render"));
        drop(e);drop(db);
    }


    #[test]
    fn provider_supervisor_panic_is_classified(){
        let worker=thread::Builder::new().name("fault-injection-provider-supervisor".into()).spawn(|| -> std::result::Result<(),CollectorFault>{
            panic!("fault injection");
        });
        let fault=join_provider(worker,"fault-injection-provider").unwrap_err();
        assert_eq!(fault.kind,FaultKind::Internal);
    }

    #[test]
    fn scan_runtime_failure_marks_collecting_snapshot_failed(){
        let(db,_tmp)=db();
        let e=DiagnosticEngine::with_backend(db.clone(),Arc::new(Mock{h:HardwareTelemetrySnapshot::default(),c:CrashDiagnosticsSnapshot::default()}));
        {
            let mut snapshot=e.inner.snapshot.lock().unwrap_or_else(|poison|poison.into_inner());
            snapshot.state=ScanState::Collecting;
        }
        mark_scan_runtime_failure(&e.inner,"scan.run","fault injection");
        let snapshot=e.snapshot();
        assert_eq!(snapshot.state,ScanState::Failed);
        assert!(snapshot.provider_faults.iter().any(|fault|fault.operation=="scan.run"&&fault.kind=="Internal"));
        drop(e);drop(db);
    }

    #[test]
    fn provider_panic_is_contained_and_persisted_as_typed_fault(){
        let(db,_tmp)=db();
        let e=DiagnosticEngine::with_backend(db.clone(),Arc::new(PanicHardwareMock));
        start_scan_leased(&e,OWNER).unwrap();
        for _ in 0..100{if !e.snapshot().state.running(){break}std::thread::sleep(std::time::Duration::from_millis(10));}
        let s=e.snapshot();
        assert_eq!(s.state,ScanState::Partial);
        assert!(s.provider_faults.iter().any(|fault|fault.kind=="Internal"));
        drop(e);drop(db);
    }

}
