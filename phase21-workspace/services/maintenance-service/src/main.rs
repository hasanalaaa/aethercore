use std::{path::PathBuf,sync::{Arc,atomic::{AtomicBool,Ordering}}};

use anyhow::{Context,Result};
use tracing::{error,warn};

#[cfg(windows)]
mod composition;
#[cfg(windows)]
mod errors;
#[cfg(windows)]
mod streaming;
#[cfg(windows)]
mod protocol;
#[cfg(windows)]
mod performance;
#[cfg(windows)]
mod router;
#[cfg(windows)]
mod server;
#[cfg(windows)]
mod scheduler;
#[cfg(windows)]
mod support;
mod timeline;
mod care;

const SERVICE_NAME:&str="AetherCoreMaintenance";

fn main()->Result<()>{
    #[cfg(windows)]{
        let console_mode=std::env::args().any(|a|a=="--console");
        if console_mode{aethercore_diagnostics::init_console()?;return run_console();}
        aethercore_diagnostics::init_json_file(&log_path())?;return windows_service_host::run();
    }
    #[cfg(not(windows))]
    anyhow::bail!("AetherCore maintenance service supports Windows only")
}

fn product_data_root()->PathBuf{
    let mut args=std::env::args();while let Some(arg)=args.next(){if arg=="--data-dir"{if let Some(value)=args.next(){if !value.trim().is_empty(){return PathBuf::from(value)}}}}
    if let Ok(v)=std::env::var("AETHERCORE_DATA_DIR"){return PathBuf::from(v)}
    std::env::var_os("ProgramData").map(PathBuf::from).unwrap_or_else(||PathBuf::from(r"C:\ProgramData")).join("AetherCore")
}
fn data_path()->PathBuf{product_data_root().join("state").join("aethercore.db")}
fn log_path()->PathBuf{product_data_root().join("logs").join("service.jsonl")}

#[cfg(windows)]
fn run_console()->Result<()>{let stop=Arc::new(AtomicBool::new(false));let s=stop.clone();ctrlc_handler::install(move||{s.store(true,Ordering::SeqCst);wake_pipe();})?;run_server(stop)}

#[cfg(windows)]
fn run_server(stop:Arc<AtomicBool>)->Result<()>{
    if let Err(e)=aethercore_restore_point::initialize_process_com_security(){warn!(error=%e,"System Restore COM security unavailable; protected driver installation will be blocked");}
    let root=product_data_root();let context=composition::build(&data_path(),&root).context("compose operation kernel")?;
    let _idle_scheduler=match scheduler::start(&context){
        Ok(handle)=>Some(handle),
        Err(error)=>{warn!(error=%error,"autonomous idle scheduler unavailable; interactive maintenance remains available");None}
    };
    server::run(stop,context)
}

#[cfg(windows)]
fn wake_pipe(){if let Ok(pipe_name)=aethercore_ipc::configured_pipe_name(){let _=std::fs::OpenOptions::new().read(true).write(true).open(pipe_name);}}

#[cfg(windows)]
mod ctrlc_handler {
    use std::sync::OnceLock;

    use anyhow::Result;
    use windows::Win32::{Foundation::BOOL, System::Console::SetConsoleCtrlHandler};

    static HANDLER: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

    unsafe extern "system" fn callback(_: u32) -> BOOL {
        if let Some(f) = HANDLER.get() {
            f();
        }
        true.into()
    }

    pub fn install(f: impl Fn() + Send + Sync + 'static) -> Result<()> {
        let _ = HANDLER.set(Box::new(f));
        unsafe {
            SetConsoleCtrlHandler(Some(callback), true)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        }
        Ok(())
    }
}

#[cfg(windows)]
mod windows_service_host {
    use std::{ffi::OsString, time::Duration};

    use windows_service::{
        define_windows_service,
        service::{
            ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
            ServiceType,
        },
        service_control_handler::{self, ServiceControlHandlerResult},
        service_dispatcher,
    };

    use super::*;

    define_windows_service!(ffi_service_main, service_main);

    pub fn run() -> Result<()> {
        service_dispatcher::start(SERVICE_NAME, ffi_service_main)
            .context("start service dispatcher")
    }

    fn service_main(_: Vec<OsString>) {
        if let Err(e) = service_main_impl() {
            error!(error = %e, "service failed");
        }
    }

    fn service_main_impl() -> Result<()> {
        // Fail before SERVICE_RUNNING if SCM configuration has not produced the expected live token.
        // This turns service startup itself into the authoritative SID-policy activation check.
        aethercore_security::verify_maintenance_service_token(SERVICE_NAME)
            .context("maintenance service token policy mismatch")?;

        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_handler = stop.clone();
        let status_handle = service_control_handler::register(SERVICE_NAME, move |control| {
            match control {
                ServiceControl::Stop => {
                    stop_for_handler.store(true, Ordering::SeqCst);
                    wake_pipe();
                    ServiceControlHandlerResult::NoError
                }
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        })?;

        status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })?;

        let result = run_server(stop);
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
}

