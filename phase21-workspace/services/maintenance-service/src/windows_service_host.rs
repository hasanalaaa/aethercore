//! The SCM host: dispatcher, control handler, and the two status transitions.

use std::{ffi::OsString, time::Duration};

use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult, ServiceStatusHandle},
    service_dispatcher,
};

use super::*;

define_windows_service!(ffi_service_main, service_main);

/// The outcome of opening the diagnostics log, decided in `main` before the dispatcher.
static LOG_INIT: std::sync::OnceLock<std::result::Result<(), String>> = std::sync::OnceLock::new();

pub fn run(log: std::result::Result<(), String>) -> Result<()> {
    let _ = LOG_INIT.set(log);
    service_dispatcher::start(SERVICE_NAME, ffi_service_main).context("start service dispatcher")
}

fn service_main(_: Vec<OsString>) {
    if let Err(e) = service_main_impl() {
        error!(error = %e, "service failed");
    }
}

fn status(
    state: ServiceState,
    accept: ServiceControlAccept,
    exit: u32,
    wait: Duration,
) -> ServiceStatus {
    ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: state,
        controls_accepted: accept,
        exit_code: ServiceExitCode::Win32(exit),
        checkpoint: u32::from(!wait.is_zero()),
        wait_hint: wait,
        process_id: None,
    }
}

fn service_main_impl() -> Result<()> {
    // Registered before anything can fail, so that every exit reports SERVICE_STOPPED.
    // DBT-P73-002: the token check used to return before any status was set. SCM then held
    // the service at START_PENDING with its process alive in the dispatcher, so it could
    // be neither started nor stopped: MSI 1920, then 1921 after a 4-minute wait.
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_handler = stop.clone();
    let handle_for_handler = Arc::new(std::sync::OnceLock::<ServiceStatusHandle>::new());
    let handle_slot = handle_for_handler.clone();
    let registered =
        service_control_handler::register(SERVICE_NAME, move |control| match control {
            ServiceControl::Stop => {
                stop_for_handler.store(true, Ordering::SeqCst);
                // P75: say that the stop is under way. Without STOP_PENDING the SCM kept reading
                // RUNNING until the process had already torn down.
                if let Some(handle) = handle_for_handler.get() {
                    let _ = handle.set_service_status(status(
                        ServiceState::StopPending,
                        ServiceControlAccept::empty(),
                        0,
                        Duration::from_secs(30),
                    ));
                }
                wake_pipe();
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        });
    let status_handle = match registered {
        Ok(handle) => handle,
        Err(e) => {
            // P75: without a handle no status can ever be reported, and the dispatcher waits
            // for a SERVICE_STOPPED that cannot come: the service sat at START_PENDING with
            // its process alive. Ending the process is the one signal left to the SCM.
            error!(error = %e, "service control handler registration failed");
            std::process::exit(1);
        }
    };
    let _ = handle_slot.set(status_handle);

    let result = verify_then_run(status_handle, stop);
    status_handle.set_service_status(status(
        ServiceState::Stopped,
        ServiceControlAccept::empty(),
        if result.is_ok() { 0 } else { 1 },
        Duration::default(),
    ))?;
    result
}

fn verify_then_run(status_handle: ServiceStatusHandle, stop: Arc<AtomicBool>) -> Result<()> {
    if let Some(Err(e)) = LOG_INIT.get() {
        anyhow::bail!("diagnostics log unavailable: {e}");
    }
    // Fail before SERVICE_RUNNING if SCM configuration has not produced the expected live token.
    // This turns service startup itself into the authoritative SID-policy activation check.
    aethercore_security::verify_maintenance_service_token(SERVICE_NAME)
        .context("maintenance service token policy mismatch")?;
    // P75: RUNNING was reported here, before the database, the model and the scheduler were
    // composed; a client could connect to a service with no pipe yet, and a composition
    // failure followed a RUNNING. Composition now runs under START_PENDING.
    status_handle.set_service_status(status(
        ServiceState::StartPending,
        ServiceControlAccept::empty(),
        0,
        Duration::from_secs(120),
    ))?;
    run_server(stop, || {
        status_handle
            .set_service_status(status(
                ServiceState::Running,
                ServiceControlAccept::STOP,
                0,
                Duration::default(),
            ))
            .context("report SERVICE_RUNNING")
    })
}
