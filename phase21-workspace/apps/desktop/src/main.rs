#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

use aethercore_contracts::{
    PROTOCOL_VERSION,
    v1::{self, Request, RequestHeader, request, response},
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::{Emitter, command};
use uuid::Uuid;

#[cfg(windows)]
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UiSnapshot {
    connected: bool,
    service_version: String,
    health: String,
    server_time_unix_ms: i64,
    journal_event_count: u64,
    active_plan: Option<v1::PlanSnapshot>,
}

#[cfg(windows)]
static APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();
#[cfg(windows)]
static SESSION: OnceLock<Mutex<Option<Arc<aethercore_ipc::SessionClient>>>> = OnceLock::new();
#[cfg(windows)]
static CONNECT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
#[cfg(windows)]
static LAST_EVENT_SEQUENCE: AtomicU64 = AtomicU64::new(0);
#[cfg(windows)]
static SESSION_DESIRED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UiKernelEvent {
    sequence: u64,
    emitted_unix_ms: i64,
    kind: String,
    plan_id: String,
    payload: serde_json::Value,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UiSessionState {
    connected: bool,
    session_id: String,
    service_version: String,
    current_sequence: u64,
    replay_floor_sequence: u64,
    replay_complete: bool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UiStreamReset {
    reason: String,
    current_sequence: u64,
    replay_floor_sequence: u64,
    message_key: String,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SupportExportResult {
    path: String,
    sha256: String,
    verification_fingerprint_sha256: String,
}

#[cfg(windows)]
fn session_slot() -> &'static Mutex<Option<Arc<aethercore_ipc::SessionClient>>> {
    SESSION.get_or_init(|| Mutex::new(None))
}
#[cfg(windows)]
fn connect_lock() -> &'static Mutex<()> {
    CONNECT_LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(windows)]
fn normalize_event(event: v1::EventEnvelope) -> UiKernelEvent {
    use v1::event_envelope::Payload;
    let (kind, payload) = match event.payload {
        Some(Payload::ServiceSnapshot(v)) => (
            "serviceSnapshot",
            serde_json::to_value(v).unwrap_or_default(),
        ),
        Some(Payload::Plan(v)) => ("plan", serde_json::to_value(v).unwrap_or_default()),
        Some(Payload::DriverHubSnapshot(v)) => (
            "driverHubSnapshot",
            serde_json::to_value(v).unwrap_or_default(),
        ),
        Some(Payload::DriverInstallStatus(v)) => (
            "driverInstallStatus",
            serde_json::to_value(v).unwrap_or_default(),
        ),
        Some(Payload::RecoveryHistory(v)) => (
            "recoveryHistory",
            serde_json::to_value(v).unwrap_or_default(),
        ),
        Some(Payload::RepairAssessment(v)) => (
            "repairAssessment",
            serde_json::to_value(v).unwrap_or_default(),
        ),
        Some(Payload::SystemRepairStatus(v)) => (
            "systemRepairStatus",
            serde_json::to_value(v).unwrap_or_default(),
        ),
        Some(Payload::CleanupSnapshot(v)) => (
            "cleanupSnapshot",
            serde_json::to_value(v).unwrap_or_default(),
        ),
        Some(Payload::CleanupStatus(v)) => {
            ("cleanupStatus", serde_json::to_value(v).unwrap_or_default())
        }
        Some(Payload::StartupSnapshot(v)) => (
            "startupSnapshot",
            serde_json::to_value(v).unwrap_or_default(),
        ),
        Some(Payload::StartupStatus(v)) => {
            ("startupStatus", serde_json::to_value(v).unwrap_or_default())
        }
        Some(Payload::StartupHistory(v)) => (
            "startupHistory",
            serde_json::to_value(v).unwrap_or_default(),
        ),
        Some(Payload::DiagnosticsSnapshot(v)) => (
            "diagnosticsSnapshot",
            serde_json::to_value(v).unwrap_or_default(),
        ),
        Some(Payload::DiagnosticsHistory(v)) => (
            "diagnosticsHistory",
            serde_json::to_value(v).unwrap_or_default(),
        ),
        Some(Payload::ConsentIntent(v)) => {
            ("consentIntent", serde_json::to_value(v).unwrap_or_default())
        }
        Some(Payload::MutationLease(v)) => {
            ("mutationLease", serde_json::to_value(v).unwrap_or_default())
        }
        Some(Payload::ProgressTelemetry(v)) => (
            "progressTelemetry",
            serde_json::to_value(v).unwrap_or_default(),
        ),
        Some(Payload::Scheduler(v)) => ("scheduler", serde_json::to_value(v).unwrap_or_default()),
        Some(Payload::UpdateSnapshot(v)) => (
            "updateSnapshot",
            serde_json::to_value(v).unwrap_or_default(),
        ),
        Some(Payload::SupportBundle(v)) => {
            ("supportBundle", serde_json::to_value(v).unwrap_or_default())
        }
        Some(Payload::DeepScanSnapshot(v)) => (
            "deepScanSnapshot",
            serde_json::to_value(v).unwrap_or_default(),
        ),
        Some(Payload::PerformanceSnapshot(v)) => (
            "performanceSnapshot",
            serde_json::to_value(v).unwrap_or_default(),
        ),
        Some(Payload::BottleneckReport(v)) => (
            "bottleneckReport",
            serde_json::to_value(v).unwrap_or_default(),
        ),
        Some(Payload::OptimizationStatus(v)) => (
            "optimizationStatus",
            serde_json::to_value(v).unwrap_or_default(),
        ),
        Some(Payload::TimelinePage(v)) => {
            ("timelinePage", serde_json::to_value(v).unwrap_or_default())
        }
        Some(Payload::CareStatus(v)) => ("careStatus", serde_json::to_value(v).unwrap_or_default()),
        Some(Payload::Insights(v)) => ("insights", serde_json::to_value(v).unwrap_or_default()),
        Some(Payload::PlatformCapabilities(v)) => (
            "platformCapabilities",
            serde_json::to_value(v).unwrap_or_default(),
        ),
        Some(Payload::SecurityAudit(v)) => {
            ("securityAudit", serde_json::to_value(v).unwrap_or_default())
        }
        None => ("unknown", serde_json::Value::Null),
    };
    UiKernelEvent {
        sequence: event.sequence,
        emitted_unix_ms: event.emitted_unix_ms,
        kind: kind.into(),
        plan_id: event.plan_id,
        payload,
    }
}

#[cfg(windows)]
fn emit_session_state(connected: bool, client: Option<&aethercore_ipc::SessionClient>) {
    if let Some(app) = APP_HANDLE.get() {
        let state = UiSessionState {
            connected,
            session_id: client
                .map(|c| c.hello.session_id.clone())
                .unwrap_or_default(),
            service_version: client
                .map(|c| c.hello.service_version.clone())
                .unwrap_or_default(),
            current_sequence: client
                .map(|c| c.hello.current_sequence)
                .unwrap_or_else(|| LAST_EVENT_SEQUENCE.load(Ordering::Acquire)),
            replay_floor_sequence: client
                .map(|c| c.hello.replay_floor_sequence)
                .unwrap_or_default(),
            replay_complete: client.map(|c| c.hello.replay_complete).unwrap_or(false),
        };
        let _ = app.emit("aethercore://session-state", state);
    }
}

#[cfg(windows)]
fn connect_persistent_session() -> anyhow::Result<Arc<aethercore_ipc::SessionClient>> {
    let replay = LAST_EVENT_SEQUENCE.load(Ordering::Acquire);
    let on_event: Arc<dyn Fn(v1::EventEnvelope) + Send + Sync> = Arc::new(|event| {
        LAST_EVENT_SEQUENCE.fetch_max(event.sequence, Ordering::AcqRel);
        if let Some(app) = APP_HANDLE.get() {
            let _ = app.emit("aethercore://kernel-event", normalize_event(event));
        }
    });
    let on_stream_reset: Arc<dyn Fn(v1::StreamReset) + Send + Sync> = Arc::new(|reset| {
        // Advancing the local watermark before hydration causes any already-queued stale frames at
        // or below the reset sequence to be ignored by the renderer. Fresh events continue above
        // this watermark while typed hydration rebuilds the complete view.
        LAST_EVENT_SEQUENCE.store(reset.current_sequence, Ordering::Release);
        if let Some(app) = APP_HANDLE.get() {
            let reason = v1::StreamResetReason::try_from(reset.reason)
                .map(|value| value.as_str_name().to_owned())
                .unwrap_or_else(|_| "STREAM_RESET_REASON_UNSPECIFIED".into());
            let _ = app.emit(
                "aethercore://stream-reset",
                UiStreamReset {
                    reason,
                    current_sequence: reset.current_sequence,
                    replay_floor_sequence: reset.replay_floor_sequence,
                    message_key: reset.message_key,
                },
            );
        }
    });
    let on_disconnect: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {
        // A stale session may disconnect after a replacement has already connected. Never clear
        // the shared slot from the callback; only report offline when the currently published
        // session is absent/dead. active_session/request own replacement of the slot.
        let current_is_live = session_slot()
            .lock()
            .ok()
            .and_then(|slot| slot.as_ref().map(|c| c.is_alive()))
            .unwrap_or(false);
        if !current_is_live {
            emit_session_state(false, None);
        }
    });
    let client = aethercore_ipc::SessionClient::connect(
        "aethercore-desktop",
        env!("CARGO_PKG_VERSION"),
        replay,
        on_event,
        on_stream_reset,
        on_disconnect,
    )?;
    emit_session_state(true, Some(client.as_ref()));
    Ok(client)
}

#[cfg(windows)]
fn active_session() -> anyhow::Result<Arc<aethercore_ipc::SessionClient>> {
    if let Some(existing) = session_slot()
        .lock()
        .map_err(|_| anyhow::anyhow!("session lock poisoned"))?
        .as_ref()
        .filter(|c| c.is_alive())
        .cloned()
    {
        return Ok(existing);
    }
    let _connect_guard = connect_lock()
        .lock()
        .map_err(|_| anyhow::anyhow!("session connect lock poisoned"))?;
    // Recheck after serializing connection establishment: a background reconnect or another RPC
    // may have won the race while we waited for the connect lock.
    if let Some(existing) = session_slot()
        .lock()
        .map_err(|_| anyhow::anyhow!("session lock poisoned"))?
        .as_ref()
        .filter(|c| c.is_alive())
        .cloned()
    {
        return Ok(existing);
    }
    let client = connect_persistent_session()?;
    *session_slot()
        .lock()
        .map_err(|_| anyhow::anyhow!("session lock poisoned"))? = Some(client.clone());
    Ok(client)
}

#[cfg(windows)]
fn invalidate_session_if_current(failed: &Arc<aethercore_ipc::SessionClient>) {
    if let Ok(mut slot) = session_slot().lock() {
        if slot
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, failed))
        {
            *slot = None;
        }
    }
}

fn request(payload: request::Payload) -> anyhow::Result<v1::Response> {
    let req = Request {
        header: Some(RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: Uuid::new_v4().to_string(),
        }),
        payload: Some(payload),
    };
    #[cfg(windows)]
    let resp = {
        let mut last_error = None;
        let mut response = None;
        for _ in 0..2 {
            let client = match active_session() {
                Ok(client) => client,
                Err(error) => {
                    last_error = Some(error);
                    continue;
                }
            };
            match client
                .request(req.clone(), Duration::from_secs(15))
                .map_err(anyhow::Error::from)
            {
                Ok(v) => {
                    response = Some(v);
                    break;
                }
                Err(e) => {
                    last_error = Some(e);
                    // Request-level deadline/backpressure errors do not invalidate a healthy
                    // persistent transport. Reconnect only after the IPC layer has observed an
                    // actual reader/writer disconnect; this prevents timeout-driven session churn.
                    if !client.is_alive() {
                        invalidate_session_if_current(&client);
                    }
                }
            }
        }
        response.ok_or_else(|| {
            last_error.unwrap_or_else(|| anyhow::anyhow!("service session unavailable"))
        })?
    };
    #[cfg(not(windows))]
    let resp = aethercore_ipc::connect(&req)?;
    if let Some(error) = resp.error.as_ref() {
        if error.code != v1::ErrorCode::Unspecified as i32 {
            anyhow::bail!(error.message_key.clone());
        }
    }
    if resp.status_code != 0 {
        anyhow::bail!(resp.error_message.clone());
    }
    Ok(resp)
}

#[command]
async fn start_ipc_session() -> Result<UiSessionState, String> {
    #[cfg(windows)]
    {
        SESSION_DESIRED.store(true, Ordering::Release);
        let client = tauri::async_runtime::spawn_blocking(|| {
            let client = active_session()?;
            // Listener registration happens in the renderer before this command. Re-publish the
            // complete principal-scoped state image through the same ordered event stream so a
            // renderer reload can hydrate even when the underlying pipe session stayed alive.
            let hydrate = Request {
                header: Some(RequestHeader {
                    protocol_version: PROTOCOL_VERSION,
                    request_id: Uuid::new_v4().to_string(),
                }),
                payload: Some(request::Payload::HydrateSession(
                    v1::HydrateSessionRequest {},
                )),
            };
            let response = client.request(hydrate, Duration::from_secs(15))?;
            if response.status_code != 0 {
                let detail = response
                    .error
                    .as_ref()
                    .map(|value| value.message_key.clone())
                    .unwrap_or_else(|| response.error_message.clone());
                anyhow::bail!(detail);
            }
            Ok::<_, anyhow::Error>(client)
        })
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
        return Ok(UiSessionState {
            connected: true,
            session_id: client.hello.session_id.clone(),
            service_version: client.hello.service_version.clone(),
            current_sequence: LAST_EVENT_SEQUENCE.load(Ordering::Acquire),
            replay_floor_sequence: client.hello.replay_floor_sequence,
            replay_complete: client.hello.replay_complete,
        });
    }
    #[cfg(not(windows))]
    Err("Windows only".into())
}

#[command]
async fn get_snapshot() -> Result<UiSnapshot, String> {
    tauri::async_runtime::spawn_blocking(|| {
        match request(request::Payload::GetSnapshot(v1::GetSnapshotRequest {})) {
            Ok(resp) => match resp.payload {
                Some(response::Payload::Snapshot(s)) => {
                    let s = s.snapshot.ok_or("missing snapshot")?;
                    Ok(UiSnapshot {
                        connected: true,
                        service_version: s.service_version,
                        health: s.health,
                        server_time_unix_ms: s.server_time_unix_ms,
                        journal_event_count: s.journal_event_count,
                        active_plan: s.active_plan,
                    })
                }
                _ => Err("unexpected service response".into()),
            },
            Err(_) => Ok(UiSnapshot {
                connected: false,
                service_version: "—".into(),
                health: "Service offline".into(),
                server_time_unix_ms: 0,
                journal_event_count: 0,
                active_plan: None,
            }),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn get_driver_hub_snapshot() -> Result<v1::DriverHubSnapshot, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let resp = request(request::Payload::GetDriverHubSnapshot(
            v1::GetDriverHubSnapshotRequest {},
        ))
        .map_err(|e| e.to_string())?;
        extract_driver_hub(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn start_driver_scan() -> Result<v1::DriverHubSnapshot, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let resp = request(request::Payload::StartDriverScan(
            v1::StartDriverScanRequest {},
        ))
        .map_err(|e| e.to_string())?;
        extract_driver_hub(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn set_driver_candidate_policy(
    scan_id: String,
    inventory_epoch: u64,
    candidate_id: String,
    policy: String,
) -> Result<v1::DriverHubSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::SetDriverCandidatePolicy(
            v1::SetDriverCandidatePolicyRequest {
                scan_id,
                inventory_epoch,
                candidate_id,
                policy,
            },
        ))
        .map_err(|e| e.to_string())?;
        extract_driver_hub(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn open_gpu_vendor_support(vendor: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || open_gpu_vendor_support_blocking(&vendor))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

fn extract_driver_hub(resp: v1::Response) -> Result<v1::DriverHubSnapshot, String> {
    match resp.payload {
        Some(response::Payload::DriverHubSnapshot(payload)) => {
            payload.snapshot.ok_or("missing driver hub snapshot".into())
        }
        _ => Err("unexpected driver hub response".into()),
    }
}

#[command]
async fn approve_plan_with_uac(plan_id: String, locale: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || authorize_blocking(&plan_id, &locale))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

fn extract_plan(resp: v1::Response) -> Result<v1::PlanSnapshot, String> {
    match resp.payload {
        Some(response::Payload::Plan(p)) => p.plan.ok_or("missing plan".into()),
        _ => Err("unexpected service response".into()),
    }
}

#[command]
async fn create_driver_install_plan(
    scan_id: String,
    inventory_epoch: u64,
    candidate_ids: Vec<String>,
) -> Result<v1::PlanSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::CreateDriverInstallPlan(
            v1::CreateDriverInstallPlanRequest {
                scan_id,
                inventory_epoch,
                candidate_ids,
            },
        ))
        .map_err(|e| e.to_string())?;
        extract_plan(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn start_driver_install(plan_id: String) -> Result<v1::DriverInstallStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::StartDriverInstall(
            v1::StartDriverInstallRequest { plan_id },
        ))
        .map_err(|e| e.to_string())?;
        extract_install_status(resp)?.ok_or("driver installation status missing".into())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn get_driver_install_status(
    plan_id: String,
) -> Result<Option<v1::DriverInstallStatus>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::GetDriverInstallStatus(
            v1::GetDriverInstallStatusRequest { plan_id },
        ))
        .map_err(|e| e.to_string())?;
        extract_install_status(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn get_recovery_history(limit: u32) -> Result<Vec<v1::RecoveryEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::GetRecoveryHistory(
            v1::GetRecoveryHistoryRequest { limit },
        ))
        .map_err(|e| e.to_string())?;
        match resp.payload {
            Some(response::Payload::RecoveryHistory(payload)) => Ok(payload.entries),
            _ => Err("unexpected recovery history response".into()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

fn extract_install_status(resp: v1::Response) -> Result<Option<v1::DriverInstallStatus>, String> {
    match resp.payload {
        Some(response::Payload::DriverInstallStatus(payload)) => Ok(payload.status),
        _ => Err("unexpected driver install status response".into()),
    }
}

#[command]
async fn start_repair_assessment() -> Result<v1::RepairAssessmentSnapshot, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let resp = request(request::Payload::StartRepairAssessment(
            v1::StartRepairAssessmentRequest {},
        ))
        .map_err(|e| e.to_string())?;
        extract_repair_assessment(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn get_repair_assessment() -> Result<v1::RepairAssessmentSnapshot, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let resp = request(request::Payload::GetRepairAssessment(
            v1::GetRepairAssessmentRequest {},
        ))
        .map_err(|e| e.to_string())?;
        extract_repair_assessment(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn create_system_repair_plan(
    assessment_id: String,
    run_disk_scan: bool,
) -> Result<v1::PlanSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::CreateSystemRepairPlan(
            v1::CreateSystemRepairPlanRequest {
                assessment_id,
                run_disk_scan,
            },
        ))
        .map_err(|e| e.to_string())?;
        extract_plan(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn start_system_repair(plan_id: String) -> Result<v1::SystemRepairStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::StartSystemRepair(
            v1::StartSystemRepairRequest { plan_id },
        ))
        .map_err(|e| e.to_string())?;
        extract_system_repair_status(resp)?.ok_or("system repair status missing".into())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn get_system_repair_status(
    plan_id: String,
) -> Result<Option<v1::SystemRepairStatus>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::GetSystemRepairStatus(
            v1::GetSystemRepairStatusRequest { plan_id },
        ))
        .map_err(|e| e.to_string())?;
        extract_system_repair_status(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn start_cleanup_scan() -> Result<v1::CleanupSnapshot, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let resp = request(request::Payload::StartCleanupScan(
            v1::StartCleanupScanRequest {},
        ))
        .map_err(|e| e.to_string())?;
        extract_cleanup_snapshot(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn get_cleanup_snapshot() -> Result<v1::CleanupSnapshot, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let resp = request(request::Payload::GetCleanupSnapshot(
            v1::GetCleanupSnapshotRequest {},
        ))
        .map_err(|e| e.to_string())?;
        extract_cleanup_snapshot(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn create_cleanup_plan(
    scan_id: String,
    inventory_epoch: u64,
    candidate_ids: Vec<String>,
) -> Result<v1::PlanSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::CreateCleanupPlan(
            v1::CreateCleanupPlanRequest {
                scan_id,
                inventory_epoch,
                candidate_ids,
            },
        ))
        .map_err(|e| e.to_string())?;
        extract_plan(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn start_cleanup(plan_id: String) -> Result<v1::CleanupStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::StartCleanup(v1::StartCleanupRequest {
            plan_id,
        }))
        .map_err(|e| e.to_string())?;
        extract_cleanup_status(resp)?.ok_or("cleanup status missing".into())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn get_cleanup_status(plan_id: String) -> Result<Option<v1::CleanupStatus>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::GetCleanupStatus(
            v1::GetCleanupStatusRequest { plan_id },
        ))
        .map_err(|e| e.to_string())?;
        extract_cleanup_status(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn extract_repair_assessment(resp: v1::Response) -> Result<v1::RepairAssessmentSnapshot, String> {
    match resp.payload {
        Some(response::Payload::RepairAssessment(payload)) => {
            payload.assessment.ok_or("missing repair assessment".into())
        }
        _ => Err("unexpected repair assessment response".into()),
    }
}

fn extract_system_repair_status(
    resp: v1::Response,
) -> Result<Option<v1::SystemRepairStatus>, String> {
    match resp.payload {
        Some(response::Payload::SystemRepairStatus(payload)) => Ok(payload.status),
        _ => Err("unexpected system repair status response".into()),
    }
}

fn extract_cleanup_snapshot(resp: v1::Response) -> Result<v1::CleanupSnapshot, String> {
    match resp.payload {
        Some(response::Payload::CleanupSnapshot(payload)) => {
            payload.snapshot.ok_or("missing cleanup snapshot".into())
        }
        _ => Err("unexpected cleanup snapshot response".into()),
    }
}

fn extract_cleanup_status(resp: v1::Response) -> Result<Option<v1::CleanupStatus>, String> {
    match resp.payload {
        Some(response::Payload::CleanupStatus(payload)) => Ok(payload.status),
        _ => Err("unexpected cleanup status response".into()),
    }
}

#[command]
async fn start_startup_scan() -> Result<v1::StartupSnapshot, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let resp = request(request::Payload::StartStartupScan(
            v1::StartStartupScanRequest {},
        ))
        .map_err(|e| e.to_string())?;
        extract_startup_snapshot(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn get_startup_snapshot() -> Result<v1::StartupSnapshot, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let resp = request(request::Payload::GetStartupSnapshot(
            v1::GetStartupSnapshotRequest {},
        ))
        .map_err(|e| e.to_string())?;
        extract_startup_snapshot(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn create_startup_plan(
    scan_id: String,
    inventory_epoch: u64,
    decisions: Vec<v1::StartupDecisionInput>,
    confirm_service_changes: bool,
) -> Result<v1::PlanSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::CreateStartupPlan(
            v1::CreateStartupPlanRequest {
                scan_id,
                inventory_epoch,
                decisions,
                confirm_service_changes,
            },
        ))
        .map_err(|e| e.to_string())?;
        extract_plan(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn create_startup_restore_plan(
    change_id: String,
    confirm_service_changes: bool,
) -> Result<v1::PlanSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::CreateStartupRestorePlan(
            v1::CreateStartupRestorePlanRequest {
                change_id,
                confirm_service_changes,
            },
        ))
        .map_err(|e| e.to_string())?;
        extract_plan(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn start_startup_changes(plan_id: String) -> Result<v1::StartupStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::StartStartupChanges(
            v1::StartStartupChangesRequest { plan_id },
        ))
        .map_err(|e| e.to_string())?;
        extract_startup_status(resp)?.ok_or("startup status missing".into())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn get_startup_status(plan_id: String) -> Result<Option<v1::StartupStatus>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::GetStartupStatus(
            v1::GetStartupStatusRequest { plan_id },
        ))
        .map_err(|e| e.to_string())?;
        extract_startup_status(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn get_startup_history(limit: u32) -> Result<Vec<v1::StartupHistoryEntryInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::GetStartupHistory(
            v1::GetStartupHistoryRequest { limit },
        ))
        .map_err(|e| e.to_string())?;
        match resp.payload {
            Some(response::Payload::StartupHistory(p)) => Ok(p.entries),
            _ => Err("unexpected startup history response".into()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn start_diagnostics_scan() -> Result<v1::DiagnosticsSnapshot, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let resp = request(request::Payload::StartDiagnosticsScan(
            v1::StartDiagnosticsScanRequest {},
        ))
        .map_err(|e| e.to_string())?;
        extract_diagnostics_snapshot(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn get_diagnostics_snapshot() -> Result<v1::DiagnosticsSnapshot, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let resp = request(request::Payload::GetDiagnosticsSnapshot(
            v1::GetDiagnosticsSnapshotRequest {},
        ))
        .map_err(|e| e.to_string())?;
        extract_diagnostics_snapshot(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn get_diagnostics_history(
    limit: u32,
) -> Result<Vec<v1::DiagnosticHistoryEntryInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::GetDiagnosticsHistory(
            v1::GetDiagnosticsHistoryRequest { limit },
        ))
        .map_err(|e| e.to_string())?;
        match resp.payload {
            Some(response::Payload::DiagnosticsHistory(p)) => Ok(p.entries),
            _ => Err("unexpected diagnostics history response".into()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

fn extract_diagnostics_snapshot(resp: v1::Response) -> Result<v1::DiagnosticsSnapshot, String> {
    match resp.payload {
        Some(response::Payload::DiagnosticsSnapshot(p)) => {
            p.snapshot.ok_or("missing diagnostics snapshot".into())
        }
        _ => Err("unexpected diagnostics response".into()),
    }
}

#[command]
async fn start_deep_scan() -> Result<v1::DeepScanSnapshot, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let resp = request(request::Payload::StartDeepScan(v1::StartDeepScanRequest {}))
            .map_err(|e| e.to_string())?;
        extract_deep_scan_snapshot(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn cancel_deep_scan(scan_id: String) -> Result<v1::DeepScanSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::CancelDeepScan(
            v1::CancelDeepScanRequest { scan_id },
        ))
        .map_err(|e| e.to_string())?;
        extract_deep_scan_snapshot(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn get_deep_scan_snapshot() -> Result<v1::DeepScanSnapshot, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let resp = request(request::Payload::GetDeepScanSnapshot(
            v1::GetDeepScanSnapshotRequest {},
        ))
        .map_err(|e| e.to_string())?;
        extract_deep_scan_snapshot(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn get_deep_scan_history(limit: u32) -> Result<Vec<v1::DeepScanHistoryEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::GetDeepScanHistory(
            v1::GetDeepScanHistoryRequest { limit },
        ))
        .map_err(|e| e.to_string())?;
        match resp.payload {
            Some(response::Payload::DeepScanHistory(v)) => Ok(v.entries),
            _ => Err("unexpected deep scan history response".into()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn seal_remediation_plan(
    scan_id: String,
    selected_action_ids: Vec<String>,
) -> Result<v1::PcRemediationPlan, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::SealRemediationPlan(
            v1::SealRemediationPlanRequest {
                scan_id,
                selected_action_ids,
            },
        ))
        .map_err(|e| e.to_string())?;
        match resp.payload {
            Some(response::Payload::PcRemediationPlan(v)) => {
                v.plan.ok_or("missing remediation plan".into())
            }
            _ => Err("unexpected remediation plan response".into()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

fn extract_deep_scan_snapshot(resp: v1::Response) -> Result<v1::DeepScanSnapshot, String> {
    match resp.payload {
        Some(response::Payload::DeepScanSnapshot(v)) => {
            v.snapshot.ok_or("missing deep scan snapshot".into())
        }
        _ => Err("unexpected deep scan response".into()),
    }
}

#[command]
async fn check_for_updates(channel: i32) -> Result<v1::UpdateSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || check_for_updates_blocking(channel))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[command]
async fn get_update_snapshot() -> Result<v1::UpdateSnapshot, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let response = request(request::Payload::GetUpdateSnapshot(
            v1::GetUpdateSnapshotRequest {},
        ))
        .map_err(|e| e.to_string())?;
        extract_update_snapshot(response)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn stage_update(release_id: String) -> Result<v1::UpdateSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || stage_update_blocking(&release_id))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

fn check_for_updates_blocking(channel: i32) -> anyhow::Result<v1::UpdateSnapshot> {
    use aethercore_update_download::HttpsTransport;
    let response = request(request::Payload::GetUpdateCheckDescriptor(
        v1::GetUpdateCheckDescriptorRequest { channel },
    ))?;
    let descriptor = match response.payload {
        Some(response::Payload::UpdateCheckDescriptor(v)) => v,
        _ => anyhow::bail!("update.error.unavailable"),
    };
    if descriptor.channel != channel
        || descriptor.max_manifest_bytes == 0
        || descriptor.max_signature_bytes == 0
    {
        anyhow::bail!("update.error.trust");
    }
    let transport = HttpsTransport::new().map_err(update_transport_error)?;
    let manifest = transport
        .get_bytes(
            &descriptor.manifest_url,
            descriptor.max_manifest_bytes as usize,
        )
        .map_err(update_transport_error)?;
    let signature = transport
        .get_bytes(
            &descriptor.signature_url,
            descriptor.max_signature_bytes as usize,
        )
        .map_err(update_transport_error)?;
    let response = request(request::Payload::SubmitUpdateManifest(
        v1::SubmitUpdateManifestRequest {
            channel,
            manifest_bytes: manifest,
            signature_bytes: signature,
        },
    ))?;
    extract_update_snapshot(response).map_err(anyhow::Error::msg)
}

fn stage_update_blocking(release_id: &str) -> anyhow::Result<v1::UpdateSnapshot> {
    use aethercore_update_download::HttpsTransport;
    use aethercore_update_engine::MAX_STAGE_CHUNK_BYTES;
    use std::io::Read;
    const MAX_UPDATE_PACKAGE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
    let response = request(request::Payload::BeginUpdateStageUpload(
        v1::BeginUpdateStageUploadRequest {
            release_id: release_id.into(),
        },
    ))?;
    let descriptor = match response.payload {
        Some(response::Payload::UpdateStageUploadDescriptor(v)) => v,
        _ => anyhow::bail!("update.error.unavailable"),
    };
    if descriptor.upload_id.is_empty()
        || descriptor.package_url.is_empty()
        || descriptor.expected_size == 0
        || descriptor.expected_size > MAX_UPDATE_PACKAGE_BYTES
        || descriptor.expected_sha256.len() != 64
        || !descriptor
            .expected_sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
        || descriptor.max_chunk_bytes == 0
        || descriptor.max_chunk_bytes as usize > MAX_STAGE_CHUNK_BYTES as usize
    {
        let _ = cancel_update_upload(&descriptor.upload_id);
        anyhow::bail!("update.error.integrity");
    }
    let result = (|| -> anyhow::Result<v1::UpdateSnapshot> {
        let cache_root = user_update_cache_dir()?;
        std::fs::create_dir_all(&cache_root)?;
        let local_path = cache_root.join(format!("{}.download", descriptor.upload_id));
        let mut cleanup = LocalTempFile::new(local_path.clone());
        let transport = HttpsTransport::new().map_err(update_transport_error)?;
        let actual_hash = transport
            .download_to_file(
                &descriptor.package_url,
                &local_path,
                MAX_UPDATE_PACKAGE_BYTES,
                descriptor.expected_size,
                &|| false,
                &mut |_done, _total| {},
            )
            .map_err(update_transport_error)?;
        if !actual_hash.eq_ignore_ascii_case(&descriptor.expected_sha256) {
            anyhow::bail!("update.error.integrity")
        }
        let mut file = std::fs::File::open(&local_path)?;
        let chunk_size = (descriptor.max_chunk_bytes as usize).min(MAX_STAGE_CHUNK_BYTES as usize);
        let mut buffer = vec![0u8; chunk_size];
        let mut offset = 0u64;
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            let response = request(request::Payload::WriteUpdateStageChunk(
                v1::WriteUpdateStageChunkRequest {
                    upload_id: descriptor.upload_id.clone(),
                    offset,
                    data: buffer[..read].to_vec(),
                },
            ))?;
            let _ = extract_update_snapshot(response).map_err(anyhow::Error::msg)?;
            offset = offset
                .checked_add(read as u64)
                .ok_or_else(|| anyhow::anyhow!("update.error.integrity"))?;
        }
        if offset != descriptor.expected_size {
            anyhow::bail!("update.error.integrity")
        }
        drop(file);
        let response = request(request::Payload::FinalizeUpdateStageUpload(
            v1::FinalizeUpdateStageUploadRequest {
                upload_id: descriptor.upload_id.clone(),
            },
        ))?;
        let snapshot = extract_update_snapshot(response).map_err(anyhow::Error::msg)?;
        cleanup.remove_now();
        Ok(snapshot)
    })();
    if result.is_err() {
        let _ = cancel_update_upload(&descriptor.upload_id);
    }
    result
}

fn cancel_update_upload(upload_id: &str) -> anyhow::Result<()> {
    if upload_id.is_empty() {
        return Ok(());
    }
    let _ = request(request::Payload::CancelUpdateStageUpload(
        v1::CancelUpdateStageUploadRequest {
            upload_id: upload_id.into(),
        },
    ))?;
    Ok(())
}

fn cancel_update_install_intent(intent_id: &str) -> anyhow::Result<()> {
    if intent_id.is_empty() {
        return Ok(());
    }
    let _ = request(request::Payload::CancelUpdateInstallIntent(
        v1::CancelUpdateInstallIntentRequest {
            intent_id: intent_id.into(),
        },
    ))?;
    Ok(())
}

fn user_update_cache_dir() -> anyhow::Result<std::path::PathBuf> {
    let root = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("update.error.internal"))?;
    Ok(root.join("AetherCore").join("UpdateCache"))
}

struct LocalTempFile {
    path: std::path::PathBuf,
    removed: bool,
}
impl LocalTempFile {
    fn new(path: std::path::PathBuf) -> Self {
        Self {
            path,
            removed: false,
        }
    }
    fn remove_now(&mut self) {
        match std::fs::remove_file(&self.path) {
            Ok(()) => self.removed = true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => self.removed = true,
            Err(_) => {}
        }
    }
}
impl Drop for LocalTempFile {
    fn drop(&mut self) {
        if !self.removed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn update_transport_error(error: aethercore_update_download::DownloadError) -> anyhow::Error {
    use aethercore_update_download::DownloadError;
    let key = match error {
        DownloadError::Network(_) | DownloadError::HttpStatus(_) | DownloadError::Cancelled => {
            "update.error.network"
        }
        DownloadError::InvalidHttpsUrl
        | DownloadError::ResponseTooLarge
        | DownloadError::SizeMismatch => "update.error.integrity",
        DownloadError::Io(_) => "update.error.internal",
    };
    anyhow::anyhow!(key)
}

#[command]
async fn install_staged_update(release_id: String, locale: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || install_update_blocking(&release_id, &locale))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}
#[command]
async fn create_support_bundle_preview(
    include_hardware: bool,
    include_crash_metadata: bool,
    include_operation_history: bool,
    include_scheduler_activity: bool,
) -> Result<v1::SupportBundlePreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let response = request(request::Payload::CreateSupportBundlePreview(
            v1::CreateSupportBundlePreviewRequest {
                include_hardware,
                include_crash_metadata,
                include_operation_history,
                include_scheduler_activity,
            },
        ))
        .map_err(|e| e.to_string())?;
        match response.payload {
            Some(response::Payload::SupportBundlePreview(v)) => {
                v.preview.ok_or("missing support preview".into())
            }
            _ => Err("unexpected support preview response".into()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
#[command]
async fn export_support_bundle(preview_id: String) -> Result<SupportExportResult, String> {
    tauri::async_runtime::spawn_blocking(move || export_support_bundle_blocking(&preview_id))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}
fn extract_update_snapshot(response: v1::Response) -> Result<v1::UpdateSnapshot, String> {
    match response.payload {
        Some(response::Payload::UpdateSnapshot(v)) => {
            v.snapshot.ok_or("missing update snapshot".into())
        }
        _ => Err("unexpected update response".into()),
    }
}

#[cfg(windows)]
fn install_update_blocking(release_id: &str, locale: &str) -> anyhow::Result<()> {
    use aethercore_windows_foundation::OwnedHandle;
    use windows::{
        Win32::{
            System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject},
            UI::{
                Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW},
                WindowsAndMessaging::SW_SHOWNORMAL,
            },
        },
        core::PCWSTR,
    };
    let response = request(request::Payload::BeginUpdateInstallIntent(
        v1::BeginUpdateInstallIntentRequest {
            release_id: release_id.into(),
        },
    ))?;
    let intent_id = match response.payload {
        Some(response::Payload::UpdateInstallIntent(v)) => v.intent_id,
        _ => anyhow::bail!("update.error.stateConflict"),
    };
    let broker = locate_update_broker()?;
    let locale = if locale.eq_ignore_ascii_case("ar") {
        "ar"
    } else {
        "en"
    };
    let params = format!("--intent-id {intent_id} --locale {locale}");
    let verb = wide("runas");
    let file = wide_os(broker.as_os_str());
    let parameters = wide(&params);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    let broker_result = (|| -> anyhow::Result<()> {
        unsafe {
            ShellExecuteExW(&mut info)?;
            if info.hProcess.is_invalid() {
                anyhow::bail!("update.error.brokerRejected")
            }
            let process = OwnedHandle::new(info.hProcess);
            WaitForSingleObject(process.get(), INFINITE);
            let mut exit = 1u32;
            GetExitCodeProcess(process.get(), &mut exit)?;
            if exit == 2 {
                anyhow::bail!("update.error.cancelled")
            }
            if exit != 0 {
                anyhow::bail!("update.error.installFailed")
            }
        }
        Ok(())
    })();
    if broker_result.is_err() {
        let _ = cancel_update_install_intent(&intent_id);
    }
    broker_result
}
#[cfg(not(windows))]
fn install_update_blocking(_: &str, _: &str) -> anyhow::Result<()> {
    anyhow::bail!("Windows only")
}
#[cfg(windows)]
fn locate_update_broker() -> anyhow::Result<std::path::PathBuf> {
    let desktop = std::env::current_exe()?;
    let sibling = desktop
        .parent()
        .ok_or_else(|| anyhow::anyhow!("desktop executable has no parent"))?
        .join("aethercore-update-broker.exe");
    if std::env::var_os("AETHERCORE_DATA_DIR").is_some() && sibling.is_file() {
        return Ok(sibling);
    }
    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        let installed = std::path::PathBuf::from(program_files)
            .join("AetherCore")
            .join("aethercore-update-broker.exe");
        if installed.is_file() {
            return Ok(installed);
        }
    }
    if sibling.is_file() {
        return Ok(sibling);
    }
    anyhow::bail!("update.error.brokerPath")
}

fn export_support_bundle_blocking(preview_id: &str) -> anyhow::Result<SupportExportResult> {
    use sha2::{Digest, Sha256};
    use std::io::Write;
    let response = request(request::Payload::PrepareSupportBundle(
        v1::PrepareSupportBundleRequest {
            preview_id: preview_id.into(),
        },
    ))?;
    let ready = match response.payload {
        Some(response::Payload::SupportBundleReady(v)) => v
            .bundle
            .ok_or_else(|| anyhow::anyhow!("support.error.unavailable"))?,
        _ => anyhow::bail!("support.error.unavailable"),
    };
    if !ready.file_name.starts_with("AetherCore-Support-")
        || !ready.file_name.ends_with(".aetherdiag")
        || ready.file_name.contains('/')
        || ready.file_name.contains('\\')
    {
        anyhow::bail!("support.error.invalid")
    }
    if ready.public_key_hex.len() != 64
        || !ready.public_key_hex.bytes().all(|b| b.is_ascii_hexdigit())
        || ready.public_key_fingerprint_sha256.len() != 64
        || !ready
            .public_key_fingerprint_sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
    {
        anyhow::bail!("support.error.integrity")
    }
    let public_key = decode_hex_32(&ready.public_key_hex)
        .ok_or_else(|| anyhow::anyhow!("support.error.integrity"))?;
    let computed_fingerprint = hex_lower(Sha256::digest(public_key).as_slice());
    if !computed_fingerprint.eq_ignore_ascii_case(&ready.public_key_fingerprint_sha256) {
        anyhow::bail!("support.error.integrity")
    }
    let root = std::env::var_os("USERPROFILE")
        .map(std::path::PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("support.error.destination"))?;
    let downloads = root.join("Downloads");
    std::fs::create_dir_all(&downloads)?;
    let path = non_overwriting_support_path(&downloads, &ready.file_name)?;
    let temporary = downloads.join(format!(".AetherCore-{}.aetherdiag-new", ready.bundle_id));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| -> anyhow::Result<SupportExportResult> {
        let mut hash = Sha256::new();
        let mut offset = 0u64;
        loop {
            let response = request(request::Payload::ReadSupportBundleChunk(
                v1::ReadSupportBundleChunkRequest {
                    bundle_id: ready.bundle_id.clone(),
                    offset,
                    max_bytes: 256 * 1024,
                },
            ))?;
            let chunk = match response.payload {
                Some(response::Payload::SupportBundleChunk(v)) => v,
                _ => anyhow::bail!("support.error.unavailable"),
            };
            if chunk.offset != offset || chunk.total_size != ready.size_bytes {
                anyhow::bail!("support.error.integrity")
            }
            file.write_all(&chunk.data)?;
            hash.update(&chunk.data);
            offset = offset
                .checked_add(chunk.data.len() as u64)
                .ok_or_else(|| anyhow::anyhow!("support.error.integrity"))?;
            if chunk.eof {
                break;
            }
        }
        file.sync_all()?;
        drop(file);
        let actual_sha256 = hex_lower(hash.finalize().as_slice());
        if offset != ready.size_bytes || !actual_sha256.eq_ignore_ascii_case(&ready.sha256) {
            anyhow::bail!("support.error.integrity")
        }
        let archive_bytes = std::fs::read(&temporary)?;
        aethercore_support_bundle::verify_archive(
            &archive_bytes,
            &ready.public_key_fingerprint_sha256,
        )
        .map_err(|_| anyhow::anyhow!("support.error.integrity"))?;
        std::fs::rename(&temporary, &path)?;
        let _ = request(request::Payload::MarkSupportBundleExported(
            v1::MarkSupportBundleExportedRequest {
                bundle_id: ready.bundle_id.clone(),
            },
        ));
        let _ = request(request::Payload::DiscardSupportBundle(
            v1::DiscardSupportBundleRequest {
                bundle_id: ready.bundle_id.clone(),
            },
        ));
        Ok(SupportExportResult {
            path: path.to_string_lossy().into_owned(),
            sha256: actual_sha256,
            verification_fingerprint_sha256: ready.public_key_fingerprint_sha256.clone(),
        })
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
        let _ = request(request::Payload::DiscardSupportBundle(
            v1::DiscardSupportBundleRequest {
                bundle_id: ready.bundle_id.clone(),
            },
        ));
    }
    result
}

fn non_overwriting_support_path(
    downloads: &std::path::Path,
    file_name: &str,
) -> anyhow::Result<std::path::PathBuf> {
    let direct = downloads.join(file_name);
    if !direct.exists() {
        return Ok(direct);
    }
    let stem = file_name
        .strip_suffix(".aetherdiag")
        .ok_or_else(|| anyhow::anyhow!("support.error.invalid"))?;
    for index in 1..=999u16 {
        let candidate = downloads.join(format!("{stem}-{index}.aetherdiag"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    anyhow::bail!("support.error.destination")
}

fn decode_hex_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    let bytes = value.as_bytes();
    for index in 0..32 {
        let hi = hex_nibble(bytes[index * 2])?;
        let lo = hex_nibble(bytes[index * 2 + 1])?;
        out[index] = (hi << 4) | lo;
    }
    Some(out)
}
fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

fn extract_startup_snapshot(resp: v1::Response) -> Result<v1::StartupSnapshot, String> {
    match resp.payload {
        Some(response::Payload::StartupSnapshot(p)) => {
            p.snapshot.ok_or("missing startup snapshot".into())
        }
        _ => Err("unexpected startup snapshot response".into()),
    }
}
fn extract_startup_status(resp: v1::Response) -> Result<Option<v1::StartupStatus>, String> {
    match resp.payload {
        Some(response::Payload::StartupStatus(p)) => Ok(p.status),
        _ => Err("unexpected startup status response".into()),
    }
}

#[cfg(windows)]
fn open_gpu_vendor_support_blocking(vendor: &str) -> anyhow::Result<()> {
    use aethercore_gpu_policy::{GpuVendor, installed_app_path, policy};
    use windows::{
        Win32::UI::{
            Shell::{SHELLEXECUTEINFOW, ShellExecuteExW},
            WindowsAndMessaging::SW_SHOWNORMAL,
        },
        core::PCWSTR,
    };

    // Never execute a path or URL supplied by Windows Update metadata. The UI passes only a
    // vendor enum-like string. Native code resolves that value through the shared fixed policy.
    let vendor =
        GpuVendor::parse(vendor).ok_or_else(|| anyhow::anyhow!("unsupported GPU vendor"))?;
    let definition = policy(vendor);
    let target = installed_app_path(vendor);
    let verb = wide("open");
    let file = match target.as_ref() {
        Some(path) => wide_os(path.as_os_str()),
        None => wide(definition.official_url),
    };
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut info)? };
    Ok(())
}

#[cfg(not(windows))]
fn open_gpu_vendor_support_blocking(_: &str) -> anyhow::Result<()> {
    anyhow::bail!("Windows only")
}

#[cfg(windows)]
fn authorize_blocking(plan_id: &str, locale: &str) -> anyhow::Result<()> {
    use aethercore_windows_foundation::OwnedHandle;
    use windows::{
        Win32::{
            System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject},
            UI::{
                Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW},
                WindowsAndMessaging::SW_SHOWNORMAL,
            },
        },
        core::PCWSTR,
    };

    let resp = request(request::Payload::BeginConsentIntent(
        v1::BeginConsentIntentRequest {
            plan_id: plan_id.into(),
        },
    ))?;
    let intent_id = match resp.payload {
        Some(response::Payload::ConsentIntent(intent)) => intent.intent_id,
        _ => anyhow::bail!("authorization.requiredOrInvalid"),
    };

    let broker = locate_broker()?;
    // The elevated boundary receives only a non-secret opaque intent identifier. The broker
    // retrieves the immutable plan summary back from the LocalSystem service after elevation.
    let locale = if locale.eq_ignore_ascii_case("ar") {
        "ar"
    } else {
        "en"
    };
    let params = format!("--intent-id {intent_id} --locale {locale}");
    let verb = wide("runas");
    let file = wide_os(broker.as_os_str());
    let parameters = wide(&params);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };

    unsafe {
        ShellExecuteExW(&mut info)?;
        if info.hProcess.is_invalid() {
            anyhow::bail!("authorization.brokerRejected");
        }

        let process = OwnedHandle::new(info.hProcess);
        WaitForSingleObject(process.get(), INFINITE);
        let mut exit_code = 1u32;
        GetExitCodeProcess(process.get(), &mut exit_code)?;

        if exit_code != 0 {
            anyhow::bail!("authorization.brokerRejected");
        }
    }
    Ok(())
}

#[cfg(windows)]
fn locate_broker() -> anyhow::Result<std::path::PathBuf> {
    let desktop = std::env::current_exe()?;
    let sibling = desktop
        .parent()
        .ok_or_else(|| anyhow::anyhow!("desktop executable has no parent"))?
        .join("aethercore-consent-broker.exe");

    // run-dev.ps1 sets AETHERCORE_DATA_DIR and hosts the service from target\debug,
    // where the sibling broker is the service's expected identity. Installed mode
    // prefers the ACL-protected Program Files copy so the service and UI agree.
    if std::env::var_os("AETHERCORE_DATA_DIR").is_some() && sibling.is_file() {
        return Ok(sibling);
    }

    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        let installed = std::path::PathBuf::from(program_files)
            .join("AetherCore")
            .join("aethercore-consent-broker.exe");
        if installed.is_file() {
            return Ok(installed);
        }
    }

    if sibling.is_file() {
        return Ok(sibling);
    }

    anyhow::bail!("authorization.brokerPathUnavailable")
}

#[cfg(not(windows))]
fn authorize_blocking(_: &str, _: &str) -> anyhow::Result<()> {
    anyhow::bail!("Windows only")
}

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(windows)]
fn wide_os(s: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    s.encode_wide().chain(std::iter::once(0)).collect()
}

// ---------------------------------------------------------------------------
// Phase 20 — performance intelligence commands
// ---------------------------------------------------------------------------

fn extract_perf_snapshot(resp: v1::Response) -> Result<v1::PerfSnapshot, String> {
    match resp.payload {
        Some(response::Payload::PerformanceSnapshot(p)) => {
            p.snapshot.ok_or("missing performance snapshot".into())
        }
        _ => Err("unexpected performance response".into()),
    }
}

#[command]
async fn start_perf_sampling(interval_ms: u32) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        request(request::Payload::StartPerfSampling(
            v1::StartPerfSamplingRequest { interval_ms },
        ))
        .map(|_| ())
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn stop_perf_sampling() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        request(request::Payload::StopPerfSampling(
            v1::StopPerfSamplingRequest {},
        ))
        .map(|_| ())
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn get_performance_snapshot() -> Result<v1::PerfSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::GetPerformanceSnapshot(
            v1::GetPerformanceSnapshotRequest {},
        ))
        .map_err(|e| e.to_string())?;
        extract_perf_snapshot(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn extract_bottleneck_report(resp: v1::Response) -> Result<v1::BottleneckReport, String> {
    match resp.payload {
        Some(response::Payload::BottleneckReport(r)) => {
            r.report.ok_or("missing bottleneck report".into())
        }
        _ => Err("unexpected bottleneck response".into()),
    }
}

#[command]
async fn get_bottleneck_report() -> Result<v1::BottleneckReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::GetBottleneckReport(
            v1::GetBottleneckReportRequest {},
        ))
        .map_err(|e| e.to_string())?;
        extract_bottleneck_report(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn extract_optimization_plan(resp: v1::Response) -> Result<v1::OptimizationPlanSnapshot, String> {
    match resp.payload {
        Some(response::Payload::OptimizationPlan(p)) => {
            p.plan.ok_or("missing optimization plan".into())
        }
        _ => Err("unexpected optimization plan response".into()),
    }
}

// ---------------------------------------------------------------------------
// Phase 21 — Timeline Intelligence commands
// ---------------------------------------------------------------------------

#[command]
async fn get_timeline_page(
    page_size: u32,
    before_sequence: u64,
) -> Result<v1::TimelineResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::GetTimelinePage(
            v1::GetTimelinePageRequest {
                page_size,
                before_sequence,
            },
        ))
        .map_err(|e| e.to_string())?;
        match resp.payload {
            Some(response::Payload::TimelinePage(p)) => Ok(p),
            _ => Err("unexpected timeline response".into()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn get_recurrence_patterns() -> Result<v1::RecurrencePatternsResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::GetRecurrencePatterns(
            v1::GetRecurrencePatternsRequest {},
        ))
        .map_err(|e| e.to_string())?;
        match resp.payload {
            Some(response::Payload::RecurrencePatterns(p)) => Ok(p),
            _ => Err("unexpected recurrence patterns response".into()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Phase 22 — One-Click Care commands
// ---------------------------------------------------------------------------

fn extract_care_status(resp: v1::Response) -> Result<v1::CareRunStatus, String> {
    match resp.payload {
        Some(response::Payload::CareStatus(p)) => p.status.ok_or("missing care status".into()),
        _ => Err("unexpected care response".into()),
    }
}

#[command]
async fn get_care_status() -> Result<v1::CareRunStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::GetCareStatus(v1::GetCareStatusRequest {}))
            .map_err(|e| e.to_string())?;
        extract_care_status(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn grant_care_session_consent() -> Result<v1::CareRunStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::GrantCareSessionConsent(
            v1::GrantCareSessionConsentRequest {},
        ))
        .map_err(|e| e.to_string())?;
        extract_care_status(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn start_care_run() -> Result<v1::CareRunStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::StartCareRun(v1::StartCareRunRequest {}))
            .map_err(|e| e.to_string())?;
        extract_care_status(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn cancel_care_run() -> Result<v1::CareRunStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::CancelCareRun(v1::CancelCareRunRequest {}))
            .map_err(|e| e.to_string())?;
        extract_care_status(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Phase 23 — Local Intelligence commands (advisory-only)
// ---------------------------------------------------------------------------

fn extract_insights(resp: v1::Response) -> Result<v1::InsightsResponse, String> {
    match resp.payload {
        Some(response::Payload::InsightsResponse(p)) => Ok(p),
        _ => Err("unexpected insights response".into()),
    }
}

#[command]
async fn list_insights() -> Result<v1::InsightsResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::ListInsights(v1::ListInsightsRequest {}))
            .map_err(|e| e.to_string())?;
        extract_insights(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn request_insight(question_key: String) -> Result<v1::InsightsResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::RequestInsight(
            v1::RequestInsightRequest { question_key },
        ))
        .map_err(|e| e.to_string())?;
        extract_insights(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn dismiss_insight(insight_id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        request(request::Payload::DismissInsight(
            v1::DismissInsightRequest { insight_id },
        ))
        .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Phase 27 — honest platform/engine surface for the Diagnostics & About sections.
// ---------------------------------------------------------------------------

/// Typed availability chip for the renderer capability matrix.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UiCapabilityAvailability {
    state: String,
    key: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UiPlatformCapabilityStatus {
    name: String,
    availability: UiCapabilityAvailability,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UiPlatformCapabilities {
    platform: String,
    capabilities: Vec<UiPlatformCapabilityStatus>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UiEngineSource {
    source: String,
    platform: String,
}

#[command]
async fn get_platform_capabilities() -> Result<UiPlatformCapabilities, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::GetPlatformCapabilities(
            v1::GetPlatformCapabilitiesRequest {},
        ))
        .map_err(|e| e.to_string())?;
        match resp.payload {
            Some(response::Payload::PlatformCapabilitiesResponse(matrix)) => {
                Ok(UiPlatformCapabilities {
                    platform: matrix.platform,
                    capabilities: matrix
                        .capabilities
                        .into_iter()
                        .map(|c| {
                            let (state, key) =
                                c.availability.map(|a| (a.state, a.key)).unwrap_or_default();
                            UiPlatformCapabilityStatus {
                                name: c.name,
                                availability: UiCapabilityAvailability { state, key },
                            }
                        })
                        .collect(),
                })
            }
            _ => Err("unexpected capabilities response".into()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn get_engine_source() -> Result<UiEngineSource, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::GetEngineSource(
            v1::GetEngineSourceRequest {},
        ))
        .map_err(|e| e.to_string())?;
        match resp.payload {
            Some(response::Payload::EngineSourceResponse(source)) => Ok(UiEngineSource {
                source: source.source,
                platform: source.platform,
            }),
            _ => Err("unexpected engine source response".into()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
async fn create_optimization_plan(
    selected_finding_ids: Vec<String>,
) -> Result<v1::OptimizationPlanSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resp = request(request::Payload::CreateOptimizationPlan(
            v1::CreateOptimizationPlanRequest {
                selected_finding_ids,
            },
        ))
        .map_err(|e| e.to_string())?;
        extract_optimization_plan(resp)
    })
    .await
    .map_err(|e| e.to_string())?
}

// =====================================================================
// Phase 34 — Fleet (management surface + read-only snapshot). No SSH from
// the UI process: remote operations stay in aetherctl. Local management
// actions (add/edit/remove/trust/untrust) mutate the SAME strict-domain
// inventory file the CLI uses; trust authorization runs the typed
// fingerprint/public-key binding before persisting any record.
// =====================================================================

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UiFleetHost {
    host_id: String,
    display_name: String,
    hostname: String,
    port: u16,
    username: String,
    enabled: bool,
    trusted: bool,
    /// Typed trust metadata for the UI's trust/key status column.
    fingerprint: Option<String>,
    key_type: Option<String>,
    tags: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UiFleetSnapshot {
    hosts: Vec<UiFleetHost>,
    schedules: Vec<UiFleetSchedule>,
    schedule_count: usize,
    ssh_available: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UiFleetSchedule {
    schedule_id: String,
    scope: Vec<String>,
    profile_id: String,
    enabled: bool,
    cadence: String,
    next_run_unix_ms: i64,
    last_result: Option<aethercore_fleet::FleetScheduleLastResult>,
}

fn ui_schedule(schedule: &aethercore_fleet::FleetSchedule) -> UiFleetSchedule {
    let cadence = match schedule.cadence {
        aethercore_fleet::FleetCadence::EveryHours(hours) => format!("every_hours:{hours}"),
        aethercore_fleet::FleetCadence::DailyAtUtcHour(hour) => format!("daily_at_utc_hour:{hour}"),
    };
    UiFleetSchedule {
        schedule_id: schedule.schedule_id.clone(),
        scope: schedule.scope.clone(),
        profile_id: schedule.profile_id.clone(),
        enabled: schedule.enabled,
        cadence,
        next_run_unix_ms: schedule.next_run_unix_ms,
        last_result: schedule.last_result.clone(),
    }
}

#[command]
fn fleet_snapshot() -> UiFleetSnapshot {
    let inventory = load_fleet_inventory();
    let hosts = inventory
        .hosts
        .iter()
        .map(|host| UiFleetHost {
            host_id: host.host_id.clone(),
            display_name: host.display_name.clone(),
            hostname: host.hostname.clone(),
            port: host.port,
            username: host.username.clone(),
            enabled: host.enabled,
            trusted: host.trust.is_some(),
            fingerprint: host.trust.as_ref().map(|t| t.host_key_sha256.clone()),
            key_type: host.trust.as_ref().map(|t| t.key_type.clone()),
            tags: host.tags.iter().cloned().collect(),
        })
        .collect();
    let schedules = load_fleet_schedules();
    UiFleetSnapshot {
        hosts,
        schedule_count: schedules.len(),
        schedules: schedules.iter().map(ui_schedule).collect(),
        ssh_available: aethercore_fleet::ssh_binary().is_some(),
    }
}

/// Typed UI add/remove/edit results (no secrets cross this boundary).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UiFleetActionResult {
    ok: bool,
    host_id: String,
    detail: String,
}

fn load_fleet_inventory() -> aethercore_fleet::FleetInventory {
    let path = dirs_fleet_state().join("fleet").join("inventory.json");
    if let Ok(raw) = std::fs::read(&path) {
        if let Ok(inventory) = aethercore_fleet::FleetInventory::from_bytes(&raw) {
            return inventory;
        }
    }
    aethercore_fleet::FleetInventory::new()
}

fn save_fleet_inventory(inventory: &aethercore_fleet::FleetInventory) -> Result<(), String> {
    let path = dirs_fleet_state().join("fleet").join("inventory.json");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = inventory.to_bytes().map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())
}

fn fleet_action_error(host_id: &str, error: impl std::fmt::Display) -> UiFleetActionResult {
    UiFleetActionResult {
        ok: false,
        host_id: host_id.to_string(),
        detail: error.to_string(),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct UiAddHost {
    host_id: String,
    display_name: String,
    hostname: String,
    port: u16,
    username: String,
    /// "agent" | "key" | "cert" — a PATH reference only, never contents.
    auth_kind: String,
    auth_path: Option<String>,
    tags: Vec<String>,
}

#[command]
fn fleet_add_host(input: UiAddHost) -> UiFleetActionResult {
    let auth = match (input.auth_kind.as_str(), input.auth_path.as_deref()) {
        ("agent", _) => aethercore_fleet::AuthReference::Agent,
        ("key", Some(path)) => aethercore_fleet::AuthReference::KeyFile {
            path: path.to_string(),
        },
        ("cert", Some(path)) => aethercore_fleet::AuthReference::Certificate {
            path: path.to_string(),
        },
        _ => {
            return fleet_action_error(
                &input.host_id,
                "auth kind requires key|cert with a path reference",
            );
        }
    };
    let host = aethercore_fleet::FleetHost::new(
        &input.host_id,
        &input.display_name,
        &input.hostname,
        input.port,
        &input.username,
        auth,
        input.tags.iter().cloned().collect(),
    );
    let host = match host {
        Ok(host) => host,
        Err(error) => return fleet_action_error(&input.host_id, error),
    };
    let mut inventory = load_fleet_inventory();
    if let Err(error) = inventory.add(host) {
        return fleet_action_error(&input.host_id, error);
    }
    match save_fleet_inventory(&inventory) {
        Ok(()) => UiFleetActionResult {
            ok: true,
            host_id: input.host_id,
            detail: "added".to_string(),
        },
        Err(error) => fleet_action_error(&input.host_id, error),
    }
}

/// Edit NON-SECRET metadata only: display name, enabled flag, tags. Hostname
/// changes are allowed (identity rotation is an explicit admin action);
/// username/port/auth require remove + re-add for audit clarity. No secret
/// field exists on this struct — nothing secret can be edited here.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct UiEditHost {
    host_id: String,
    display_name: Option<String>,
    enabled: Option<bool>,
    tags: Option<Vec<String>>,
}

#[command]
fn fleet_edit_host(input: UiEditHost) -> UiFleetActionResult {
    let mut inventory = load_fleet_inventory();
    {
        let Some(host) = inventory.get_mut(&input.host_id) else {
            return fleet_action_error(&input.host_id, "unknown host id");
        };
        if let Some(display_name) = &input.display_name {
            host.display_name = display_name.trim().to_string();
        }
        if let Some(enabled) = input.enabled {
            host.enabled = enabled;
        }
        if let Some(tags) = &input.tags {
            host.tags = tags.iter().cloned().collect();
        }
    }
    // Re-validate the whole record through the strict parser.
    let record = match inventory.get(&input.host_id) {
        Some(host) => match serde_json::to_vec(host) {
            Ok(bytes) => bytes,
            Err(error) => return fleet_action_error(&input.host_id, error),
        },
        None => return fleet_action_error(&input.host_id, "unknown host id"),
    };
    if let Err(error) = aethercore_fleet::FleetHost::from_bytes(&record) {
        return fleet_action_error(&input.host_id, error);
    }
    match save_fleet_inventory(&inventory) {
        Ok(()) => UiFleetActionResult {
            ok: true,
            host_id: input.host_id,
            detail: "edited".to_string(),
        },
        Err(error) => fleet_action_error(&input.host_id, error),
    }
}

#[command]
fn fleet_remove_host(host_id: String, confirm: bool) -> UiFleetActionResult {
    if !confirm {
        return fleet_action_error(&host_id, "explicit confirmation required");
    }
    let mut inventory = load_fleet_inventory();
    let removed = inventory.get(&host_id).cloned();
    if let Err(error) = inventory.remove(&host_id) {
        return fleet_action_error(&host_id, error);
    }
    // Revocation must also drop the trust-store record + known_hosts line.
    if let Some(host) = removed {
        if let Err(error) = fleet_trust_store().untrust(&host.hostname, host.port) {
            return fleet_action_error(&host_id, error);
        }
    }
    match save_fleet_inventory(&inventory) {
        Ok(()) => UiFleetActionResult {
            ok: true,
            host_id,
            detail: "removed".to_string(),
        },
        Err(error) => fleet_action_error(&host_id, error),
    }
}

fn fleet_trust_store() -> aethercore_fleet::TrustStore {
    let dir = dirs_fleet_state().join("fleet").join("trust");
    aethercore_fleet::TrustStore::open(&dir).expect("trust store dir")
}

/// Trust/untrust flow. `public_key_base64` + `authorized_fingerprint` are
/// BOTH required: the store recomputes the fingerprint from the key and
/// rejects any mismatch, so a fingerprint alone can never create trust.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct UiTrustInput {
    host_id: String,
    key_type: String,
    public_key_base64: String,
    authorized_fingerprint: String,
}

#[command]
fn fleet_trust_host(input: UiTrustInput) -> UiFleetActionResult {
    let mut inventory = load_fleet_inventory();
    let host = match inventory.get(&input.host_id) {
        Some(host) => host.clone(),
        None => return fleet_action_error(&input.host_id, "unknown host id"),
    };
    let record = aethercore_fleet::TrustedHostKey::authorize(
        &input.host_id,
        &host.hostname,
        host.port,
        &input.key_type,
        &input.public_key_base64,
        &input.authorized_fingerprint,
        now_unix_ms(),
    );
    let record = match record {
        Ok(record) => record,
        Err(error) => return fleet_action_error(&input.host_id, error),
    };
    // Bind the same fingerprint into the inventory pin (single source of
    // truth for the trust decision).
    if let Some(host_mut) = inventory.get_mut(&input.host_id) {
        if let Err(error) = host_mut.pin_trust(
            &record.host_key_sha256,
            &record.key_type,
            record.trusted_unix_ms,
        ) {
            return fleet_action_error(&input.host_id, error);
        }
    }
    let store = fleet_trust_store();
    if let Err(error) = store.trust(&record) {
        return fleet_action_error(&input.host_id, error);
    }
    match save_fleet_inventory(&inventory) {
        Ok(()) => UiFleetActionResult {
            ok: true,
            host_id: input.host_id,
            detail: "trusted (fingerprint+public key bound)".to_string(),
        },
        Err(error) => fleet_action_error(&input.host_id, error),
    }
}

#[command]
fn fleet_untrust_host(host_id: String) -> UiFleetActionResult {
    let mut inventory = load_fleet_inventory();
    if let Some(host) = inventory.get_mut(&host_id) {
        let (hostname, port) = (host.hostname.clone(), host.port);
        host.trust = None;
        let store = fleet_trust_store();
        if let Err(error) = store.untrust(&hostname, port) {
            return fleet_action_error(&host_id, error);
        }
    } else {
        return fleet_action_error(&host_id, "unknown host id");
    }
    match save_fleet_inventory(&inventory) {
        Ok(()) => UiFleetActionResult {
            ok: true,
            host_id,
            detail: "untrusted".to_string(),
        },
        Err(error) => fleet_action_error(&host_id, error),
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UiFleetRemoteResult {
    host_id: String,
    operation: String,
    outcome: String,
    detail: Option<String>,
    stdout: Option<String>,
    stderr: Option<String>,
}

fn ui_remote_result(
    host_id: &str,
    operation: &str,
    result: aethercore_fleet::RemoteResult,
) -> UiFleetRemoteResult {
    UiFleetRemoteResult {
        host_id: host_id.to_string(),
        operation: operation.to_string(),
        outcome: match result.outcome {
            aethercore_fleet::RemoteOutcomeKind::Success => "success",
            aethercore_fleet::RemoteOutcomeKind::Failed => "failed",
            aethercore_fleet::RemoteOutcomeKind::NotVerified => "not_verified",
            aethercore_fleet::RemoteOutcomeKind::NotAvailable => "not_available",
            aethercore_fleet::RemoteOutcomeKind::Timeout => "timeout",
            aethercore_fleet::RemoteOutcomeKind::Cancelled => "cancelled",
            aethercore_fleet::RemoteOutcomeKind::AuthFailure => "auth_failure",
            aethercore_fleet::RemoteOutcomeKind::HostKeyMismatch => "host_key_mismatch",
            aethercore_fleet::RemoteOutcomeKind::Incompatible => "incompatible",
        }
        .to_string(),
        detail: result.detail,
        stdout: result.output.as_ref().map(|output| output.stdout.clone()),
        stderr: result.output.map(|output| output.stderr),
    }
}

fn fleet_remote_operation(
    host_id: &str,
    operation_name: &str,
    operation: aethercore_fleet::RemoteOperation,
) -> UiFleetRemoteResult {
    let inventory = load_fleet_inventory();
    let Some(host) = inventory.get(host_id) else {
        return UiFleetRemoteResult {
            host_id: host_id.to_string(),
            operation: operation_name.to_string(),
            outcome: "failed".to_string(),
            detail: Some("unknown host id".to_string()),
            stdout: None,
            stderr: None,
        };
    };
    let store = match fleet_trust_store_checked() {
        Ok(store) => store,
        Err(detail) => {
            return UiFleetRemoteResult {
                host_id: host_id.to_string(),
                operation: operation_name.to_string(),
                outcome: "not_available".to_string(),
                detail: Some(detail),
                stdout: None,
                stderr: None,
            };
        }
    };
    let transport = aethercore_fleet::TrustedSshTransport::new(store, Duration::from_secs(45));
    ui_remote_result(
        host_id,
        operation_name,
        transport.execute_operation(host, operation),
    )
}

fn fleet_trust_store_checked() -> Result<aethercore_fleet::TrustStore, String> {
    aethercore_fleet::TrustStore::open(&dirs_fleet_state().join("fleet").join("trust"))
        .map_err(|error| format!("open fleet trust store: {error}"))
}

#[command]
fn fleet_probe(host_id: String) -> UiFleetRemoteResult {
    fleet_remote_operation(
        &host_id,
        "probe",
        aethercore_fleet::RemoteOperation::VersionProbe,
    )
}

#[command]
fn fleet_audit(host_id: String) -> UiFleetRemoteResult {
    fleet_remote_operation(
        &host_id,
        "audit",
        aethercore_fleet::RemoteOperation::SecurityAudit {
            targets_json: "[]".to_string(),
        },
    )
}

#[command]
fn fleet_compliance(host_id: String, profile_id: String) -> UiFleetRemoteResult {
    let profile: &'static str = match profile_id.as_str() {
        "cis-l1" => "cis-l1",
        "cis-l2" => "cis-l2",
        _ => {
            return UiFleetRemoteResult {
                host_id,
                operation: "compliance".to_string(),
                outcome: "incompatible".to_string(),
                detail: Some("unsupported compliance profile".to_string()),
                stdout: None,
                stderr: None,
            };
        }
    };
    fleet_remote_operation(
        &host_id,
        "compliance",
        aethercore_fleet::RemoteOperation::ComplianceCollect { profile },
    )
}

fn fleet_schedules_path() -> std::path::PathBuf {
    dirs_fleet_state().join("fleet").join("schedules.json")
}

fn load_fleet_schedules() -> Vec<aethercore_fleet::FleetSchedule> {
    let path = fleet_schedules_path();
    std::fs::read(&path)
        .ok()
        .and_then(|raw| serde_json::from_slice(&raw).ok())
        .unwrap_or_default()
}

fn save_fleet_schedules(schedules: &[aethercore_fleet::FleetSchedule]) -> Result<(), String> {
    let path = fleet_schedules_path();
    std::fs::create_dir_all(path.parent().ok_or("missing fleet state parent")?)
        .map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec_pretty(schedules).map_err(|error| error.to_string())?;
    std::fs::write(path, bytes).map_err(|error| error.to_string())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct UiScheduleInput {
    schedule_id: String,
    scope: Vec<String>,
    profile_id: String,
    every_hours: u32,
    enabled: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UiScheduleActionResult {
    ok: bool,
    schedule_id: String,
    detail: String,
}

#[command]
fn fleet_schedule_add(input: UiScheduleInput) -> UiScheduleActionResult {
    let schedule = match aethercore_fleet::FleetSchedule::new(
        &input.schedule_id,
        input.scope,
        &input.profile_id,
        aethercore_fleet::FleetCadence::EveryHours(input.every_hours),
        now_unix_ms(),
    ) {
        Ok(schedule) => schedule,
        Err(error) => {
            return UiScheduleActionResult {
                ok: false,
                schedule_id: input.schedule_id,
                detail: error.to_string(),
            };
        }
    };
    let mut schedules = load_fleet_schedules();
    if schedules
        .iter()
        .any(|existing| existing.schedule_id == schedule.schedule_id)
    {
        return UiScheduleActionResult {
            ok: false,
            schedule_id: schedule.schedule_id,
            detail: "duplicate schedule id".to_string(),
        };
    }
    let id = schedule.schedule_id.clone();
    schedules.push(schedule);
    schedules.sort_by(|a, b| a.schedule_id.cmp(&b.schedule_id));
    match save_fleet_schedules(&schedules) {
        Ok(()) => UiScheduleActionResult {
            ok: true,
            schedule_id: id,
            detail: "added".to_string(),
        },
        Err(error) => UiScheduleActionResult {
            ok: false,
            schedule_id: id,
            detail: error,
        },
    }
}

#[command]
fn fleet_schedule_update(input: UiScheduleInput) -> UiScheduleActionResult {
    let mut schedules = load_fleet_schedules();
    let Some(existing) = schedules
        .iter_mut()
        .find(|schedule| schedule.schedule_id == input.schedule_id)
    else {
        return UiScheduleActionResult {
            ok: false,
            schedule_id: input.schedule_id,
            detail: "unknown schedule id".to_string(),
        };
    };
    let replacement = match aethercore_fleet::FleetSchedule::new(
        &input.schedule_id,
        input.scope,
        &input.profile_id,
        aethercore_fleet::FleetCadence::EveryHours(input.every_hours),
        now_unix_ms(),
    ) {
        Ok(mut replacement) => {
            replacement.enabled = input.enabled;
            replacement.last_result = existing.last_result.clone();
            replacement
        }
        Err(error) => {
            return UiScheduleActionResult {
                ok: false,
                schedule_id: input.schedule_id,
                detail: error.to_string(),
            };
        }
    };
    *existing = replacement;
    let id = input.schedule_id;
    match save_fleet_schedules(&schedules) {
        Ok(()) => UiScheduleActionResult {
            ok: true,
            schedule_id: id,
            detail: "updated".to_string(),
        },
        Err(error) => UiScheduleActionResult {
            ok: false,
            schedule_id: id,
            detail: error,
        },
    }
}

#[command]
fn fleet_schedule_remove(schedule_id: String, confirm: bool) -> UiScheduleActionResult {
    if !confirm {
        return UiScheduleActionResult {
            ok: false,
            schedule_id,
            detail: "explicit confirmation required".to_string(),
        };
    }
    let mut schedules = load_fleet_schedules();
    let before = schedules.len();
    schedules.retain(|schedule| schedule.schedule_id != schedule_id);
    if schedules.len() == before {
        return UiScheduleActionResult {
            ok: false,
            schedule_id,
            detail: "unknown schedule id".to_string(),
        };
    }
    match save_fleet_schedules(&schedules) {
        Ok(()) => UiScheduleActionResult {
            ok: true,
            schedule_id,
            detail: "removed".to_string(),
        },
        Err(error) => UiScheduleActionResult {
            ok: false,
            schedule_id,
            detail: error,
        },
    }
}

struct DesktopScheduleStore {
    path: std::path::PathBuf,
    schedules: std::sync::Mutex<Vec<aethercore_fleet::FleetSchedule>>,
}

impl aethercore_fleet::SchedulerStore for DesktopScheduleStore {
    fn schedules(&self) -> Vec<aethercore_fleet::FleetSchedule> {
        self.schedules
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }
    fn save_schedule(&self, schedule: &aethercore_fleet::FleetSchedule) {
        if let Ok(mut guard) = self.schedules.lock() {
            if let Some(slot) = guard
                .iter_mut()
                .find(|item| item.schedule_id == schedule.schedule_id)
            {
                *slot = schedule.clone();
            }
            let _ = save_fleet_schedules(&guard);
        }
    }
    fn append_history(&self, record: &aethercore_fleet::ScheduleRunRecord) -> i64 {
        let path = self
            .path
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join("run_history.json");
        let mut history: Vec<serde_json::Value> = std::fs::read(&path)
            .ok()
            .and_then(|raw| serde_json::from_slice(&raw).ok())
            .unwrap_or_default();
        let seq = history.len() as i64 + 1;
        history.push(serde_json::json!({
            "runSeq": seq,
            "scheduleId": record.schedule_id,
            "triggerKind": record.trigger_kind,
            "startedUnixMs": record.started_unix_ms,
            "finishedUnixMs": record.finished_unix_ms,
            "hostsAttempted": record.hosts_attempted,
            "hostsOk": record.hosts_ok,
            "hostsFailed": record.hosts_failed,
            "outcomeSummary": record.outcome_summary,
        }));
        let _ = std::fs::write(
            path,
            serde_json::to_vec_pretty(&history).unwrap_or_default(),
        );
        seq
    }
}

#[command]
fn fleet_schedule_run_due() -> serde_json::Value {
    let schedules = load_fleet_schedules();
    if schedules.is_empty() {
        return serde_json::json!({"ran": 0, "runs": []});
    }
    let store = DesktopScheduleStore {
        path: fleet_schedules_path(),
        schedules: std::sync::Mutex::new(schedules),
    };
    let inventory = load_fleet_inventory();
    let trust_store = match fleet_trust_store_checked() {
        Ok(store) => store,
        Err(error) => return serde_json::json!({"ran": 0, "runs": [], "error": error}),
    };
    let transport = std::sync::Arc::new(aethercore_fleet::TrustedSshTransport::new(
        trust_store,
        Duration::from_secs(45),
    ));
    let summaries = aethercore_fleet::run_due_schedules(
        &store,
        &inventory,
        &aethercore_fleet::SystemClock,
        transport,
        &aethercore_fleet::OrchestratorConfig::default(),
        &aethercore_fleet::OverlapLock::default(),
        &aethercore_fleet::transport::CancelToken::default(),
    );
    serde_json::json!({
        "ran": summaries.len(),
        "runs": summaries.iter().map(|summary| serde_json::json!({
            "scheduleId": summary.schedule_id,
            "hostsAttempted": summary.hosts_attempted,
            "hostsOk": summary.hosts_ok,
            "hostsFailed": summary.hosts_failed,
            "outcomeSummary": summary.outcome_summary,
            "error": summary.error,
        })).collect::<Vec<_>>(),
    })
}

fn now_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn dirs_fleet_state() -> std::path::PathBuf {
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return std::path::PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("AetherCore");
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(state) = std::env::var_os("XDG_STATE_HOME") {
            return std::path::PathBuf::from(state).join("aethercore");
        }
        if let Some(home) = std::env::var_os("HOME") {
            return std::path::PathBuf::from(home)
                .join(".local")
                .join("state")
                .join("aethercore");
        }
    }
    #[cfg(windows)]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return std::path::PathBuf::from(appdata).join("aethercore");
        }
    }
    std::env::temp_dir().join("aethercore-fleet-state")
}

fn main() {
    let result = tauri::Builder::default()
        .setup(|app| {
            #[cfg(windows)]
            {
                let _ = APP_HANDLE.set(app.handle().clone());
                std::thread::Builder::new()
                    .name("aether-desktop-ipc-reconnect".into())
                    .spawn(|| {
                        loop {
                            let desired = SESSION_DESIRED.load(Ordering::Acquire);
                            let needs_connect = session_slot()
                                .lock()
                                .map(|slot| slot.as_ref().is_none_or(|c| !c.is_alive()))
                                .unwrap_or(true);
                            if desired && needs_connect {
                                let _ = active_session();
                            }
                            std::thread::sleep(Duration::from_secs(5));
                        }
                    })
                    .map_err(|error| -> Box<dyn std::error::Error> { Box::new(error) })?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_ipc_session,
            get_snapshot,
            get_driver_hub_snapshot,
            start_driver_scan,
            set_driver_candidate_policy,
            open_gpu_vendor_support,
            approve_plan_with_uac,
            create_driver_install_plan,
            start_driver_install,
            get_driver_install_status,
            get_recovery_history,
            start_repair_assessment,
            get_repair_assessment,
            create_system_repair_plan,
            start_system_repair,
            get_system_repair_status,
            start_cleanup_scan,
            get_cleanup_snapshot,
            create_cleanup_plan,
            start_cleanup,
            get_cleanup_status,
            start_startup_scan,
            get_startup_snapshot,
            create_startup_plan,
            create_startup_restore_plan,
            start_startup_changes,
            get_startup_status,
            get_startup_history,
            start_diagnostics_scan,
            get_diagnostics_snapshot,
            get_diagnostics_history,
            start_deep_scan,
            cancel_deep_scan,
            get_deep_scan_snapshot,
            get_deep_scan_history,
            seal_remediation_plan,
            check_for_updates,
            get_update_snapshot,
            stage_update,
            install_staged_update,
            create_support_bundle_preview,
            export_support_bundle,
            start_perf_sampling,
            stop_perf_sampling,
            get_performance_snapshot,
            get_bottleneck_report,
            create_optimization_plan,
            get_timeline_page,
            get_recurrence_patterns,
            get_care_status,
            grant_care_session_consent,
            start_care_run,
            cancel_care_run,
            list_insights,
            request_insight,
            dismiss_insight,
            get_platform_capabilities,
            get_engine_source,
            fleet_snapshot,
            fleet_add_host,
            fleet_edit_host,
            fleet_remove_host,
            fleet_trust_host,
            fleet_untrust_host,
            fleet_probe,
            fleet_audit,
            fleet_compliance,
            fleet_schedule_add,
            fleet_schedule_update,
            fleet_schedule_remove,
            fleet_schedule_run_due
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("AetherCore desktop runtime failed: {error}");
        std::process::exit(1);
    }
}
