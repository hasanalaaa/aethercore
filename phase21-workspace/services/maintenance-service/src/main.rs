use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::{Context, Result};
use tracing::{error, warn};

mod care;
#[cfg(windows)]
mod composition;
#[cfg(unix)]
mod composition;
#[cfg(unix)]
mod errors;
mod intelligence;
#[cfg(unix)]
mod performance;
#[cfg(unix)]
mod protocol;
#[cfg(unix)]
mod router;
#[cfg(windows)]
mod scheduler;
#[cfg(windows)]
mod server;
#[cfg(unix)]
mod streaming;
#[cfg(unix)]
mod support;
mod timeline;
// Phase 27/31: unix composition — the same router served over a UDS transport.
// Phase 31 (W7): UNCONDITIONAL on unix (the manual feature flag is retired; the
// `unix-ipc` feature remains only as an accepted force-on alias for exotic hosts).
#[cfg(unix)]
mod unix_composition;

const SERVICE_NAME: &str = "AetherCoreMaintenance";

fn main() -> Result<()> {
    #[cfg(windows)]
    {
        let console_mode = std::env::args().any(|a| a == "--console");
        if console_mode {
            aethercore_diagnostics::init_console()?;
            return run_console();
        }
        aethercore_diagnostics::init_json_file(&log_path())?;
        return windows_service_host::run();
    }
    // Phase 26 (unix): --foreground runs the identical service loop attached to the
    // console; --daemon = foreground + structured file logs + PID file. launchd/systemd
    // installation is explicitly deferred to Phase 28.
    #[cfg(unix)]
    {
        let args: Vec<String> = std::env::args().collect();
        let daemon = args.iter().any(|a| a == "--daemon");
        let foreground = args.iter().any(|a| a == "--foreground");
        if !(daemon || foreground) {
            anyhow::bail!(
                "usage: aethercore-maintenance-service [--foreground|--daemon] [--data-dir <dir>]"
            )
        }
        return unix_service::run_unix_service(daemon, &data_path(), &log_path());
    }
    #[allow(unreachable_code)]
    {
        anyhow::bail!("AetherCore maintenance service supports Windows and unix")
    }
}

/// Phase 26 — unix service scaffolding (no launchd/systemd installation in this phase).
#[cfg(unix)]
mod unix_service {
    use anyhow::Context;

    /// Phase 27 (T1): with the `unix-ipc` feature the --foreground loop binds the real
    /// UDS transport and serves the SAME router logic Windows serves over named pipes
    /// (one wire contract, byte-identical framing). Without the feature the honest
    /// Phase 26 scaffold prints its matrix and parks.
    #[cfg(unix)]
    pub fn run_unix_service(
        daemon: bool,
        data_path: &std::path::Path,
        log_path: &std::path::Path,
    ) -> anyhow::Result<()> {
        use std::{
            io::Write as _,
            sync::{
                Arc,
                atomic::{AtomicBool, Ordering},
            },
        };
        println!(
            "aethercore-maintenance-service: starting in {} mode",
            if daemon { "daemon" } else { "foreground" }
        );
        let mut pid_path: Option<std::path::PathBuf> = None;
        if daemon {
            if let Some(parent) = log_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            eprintln!("structured logs → {}", log_path.display());
            let path = data_path.with_extension("pid");
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(&path, std::process::id().to_string())
                .with_context(|| format!("write pid file {}", path.display()))?;
            println!("pid file written: {}", path.display());
            pid_path = Some(path);
            let _ = std::io::stdout().flush();
        }
        for (name, availability) in aethercore_platform_capabilities::matrix_for_current_platform()
        {
            let state = match &availability {
                aethercore_platform_capabilities::Availability::Native => "native".to_string(),
                aethercore_platform_capabilities::Availability::Degraded { note_key } => {
                    format!("degraded ({note_key})")
                }
                aethercore_platform_capabilities::Availability::NotAvailable { reason_key } => {
                    format!("not-available ({reason_key})")
                }
            };
            println!("capability {name}: {state}");
        }
        // Phase 28 (QD-026-003): daemon mode writes size-capped rotating JSONL logs;
        // foreground stays attached to the console.
        if daemon {
            let _ = aethercore_diagnostics::init_json_file_rotated(log_path);
        } else {
            let _ = aethercore_diagnostics::init_console();
        }
        let stop = Arc::new(AtomicBool::new(false));
        {
            let stop_for_signal = stop.clone();
            // SIGTERM/SIGINT flip the same flag the Windows SCM Stop control flips.
            signal_hook::flag::register(signal_hook::consts::SIGINT, stop_for_signal.clone())
                .context("register SIGINT handler")?;
            signal_hook::flag::register(signal_hook::consts::SIGTERM, stop_for_signal)
                .context("register SIGTERM handler")?;
        }
        let root = super::product_data_root();
        let context =
            super::composition::build(data_path, &root).context("compose operation kernel")?;
        // Integration-test rendezvous override + machine-readable socket announcement.
        let data_dir_override = std::env::args()
            .position(|a| a == "--unix-ipc-data-dir")
            .and_then(|i| std::env::args().nth(i + 1))
            .map(std::path::PathBuf::from);
        if let Some(dir) = &data_dir_override {
            let _ = dir;
        }
        crate::unix_composition::set_socket_announcer(Box::new(|path| {
            println!("unix-ipc-socket: {path}");
            use std::io::Write as _;
            let _ = std::io::stdout().flush();
        }));
        // Phase 28 (QD-026-003): run the governed accept loop to a graceful stop
        // (SIGTERM/SIGINT → drain → socket cleanup), then remove the PID file so the
        // CLI's detection cross-check converges to Offline after a clean stop.
        let result =
            crate::unix_composition::run_unix_server(stop, context, data_dir_override.as_deref());
        if let Some(path) = pid_path {
            if std::fs::remove_file(&path).is_ok() {
                println!("pid file removed: {}", path.display());
                let _ = std::io::stdout().flush();
            }
        }
        result
    }

    /// Honest capability-matrix scaffold retained when unix IPC is not compiled in.
    #[cfg(not(any(windows, unix)))]
    pub fn run_unix_service(
        daemon: bool,
        data_path: &std::path::Path,
        log_path: &std::path::Path,
    ) -> anyhow::Result<()> {
        use std::io::Write as _;
        println!(
            "aethercore-maintenance-service: starting in {} mode",
            if daemon { "daemon" } else { "foreground" }
        );
        if daemon {
            if let Some(parent) = log_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            eprintln!("structured logs → {}", log_path.display());
            let pid_path = data_path.with_extension("pid");
            if let Some(parent) = pid_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(&pid_path, std::process::id().to_string())
                .with_context(|| format!("write pid file {}", pid_path.display()))?;
            println!("pid file written: {}", pid_path.display());
            let _ = std::io::stdout().flush();
        }
        // Honest capability matrix line (Phase 26 contract).
        for (name, availability) in aethercore_platform_capabilities::matrix_for_current_platform()
        {
            let state = match &availability {
                aethercore_platform_capabilities::Availability::Native => "native".to_string(),
                aethercore_platform_capabilities::Availability::Degraded { note_key } => {
                    format!("degraded ({note_key})")
                }
                aethercore_platform_capabilities::Availability::NotAvailable { reason_key } => {
                    format!("not-available ({reason_key})")
                }
            };
            println!("capability {name}: {state}");
        }
        println!("maintenance service scaffold ready (transport wiring lands in phase 27)");
        Ok(())
    }
}

fn product_data_root() -> PathBuf {
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        if arg == "--data-dir" {
            if let Some(value) = args.next() {
                if !value.trim().is_empty() {
                    return PathBuf::from(value);
                }
            }
        }
    }
    if let Ok(v) = std::env::var("AETHERCORE_DATA_DIR") {
        return PathBuf::from(v);
    }
    std::env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
        .join("AetherCore")
}
fn data_path() -> PathBuf {
    product_data_root().join("state").join("aethercore.db")
}
fn log_path() -> PathBuf {
    product_data_root().join("logs").join("service.jsonl")
}

#[cfg(windows)]
fn run_console() -> Result<()> {
    let stop = Arc::new(AtomicBool::new(false));
    let s = stop.clone();
    ctrlc_handler::install(move || {
        s.store(true, Ordering::SeqCst);
        wake_pipe();
    })?;
    run_server(stop)
}

#[cfg(windows)]
fn run_server(stop: Arc<AtomicBool>) -> Result<()> {
    if let Err(e) = aethercore_restore_point::initialize_process_com_security() {
        warn!(error=%e,"System Restore COM security unavailable; protected driver installation will be blocked");
    }
    let root = product_data_root();
    let context = composition::build(&data_path(), &root).context("compose operation kernel")?;
    let _idle_scheduler = match scheduler::start(&context) {
        Ok(handle) => Some(handle),
        Err(error) => {
            warn!(error=%error,"autonomous idle scheduler unavailable; interactive maintenance remains available");
            None
        }
    };
    server::run(stop, context)
}

#[cfg(windows)]
fn wake_pipe() {
    if let Ok(pipe_name) = aethercore_ipc::configured_pipe_name() {
        let _ = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(pipe_name);
    }
}

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
