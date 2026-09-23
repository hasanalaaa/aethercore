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

pub fn run() -> Result<()> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main).context("start service dispatcher")
}

fn service_main(_: Vec<OsString>) {
    if let Err(e) = service_main_impl() {
        error!(error = %e, "service failed");
    }
}

fn service_main_impl() -> Result<()> {
    // Registered before anything can fail, so that every exit reports SERVICE_STOPPED.
    // DBT-P73-002: the token check used to return before any status was set. SCM then held
    // the service at START_PENDING with its process alive in the dispatcher, so it could
    // be neither started nor stopped: MSI 1920, then 1921 after a 4-minute wait.
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_handler = stop.clone();
    let status_handle =
        service_control_handler::register(SERVICE_NAME, move |control| match control {
            ServiceControl::Stop => {
                stop_for_handler.store(true, Ordering::SeqCst);
                wake_pipe();
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        })?;

    let result = verify_then_run(status_handle, stop);
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(if result.is_ok() { 0 } else { 1 }),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;
    result
}

fn verify_then_run(status_handle: ServiceStatusHandle, stop: Arc<AtomicBool>) -> Result<()> {
    // Fail before SERVICE_RUNNING if SCM configuration has not produced the expected live token.
    // This turns service startup itself into the authoritative SID-policy activation check.
    aethercore_security::verify_maintenance_service_token(SERVICE_NAME)
        .context("maintenance service token policy mismatch")?;
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;
    run_server(stop)
}
