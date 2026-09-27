use std::path::PathBuf;
#[cfg(windows)]
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[cfg(windows)]
use anyhow::Context;
use anyhow::Result;
// `error` has no consumer in this file: windows_service_host.rs inherits this
// crate-root scope through `use super::*` and calls `error!` there. Gating the
// import on #[cfg(windows)] would work, but this form keeps the import visible
// to a macOS reader as live code rather than a platform special case.
#[cfg_attr(not(windows), allow(unused_imports))]
use tracing::{error, warn};

mod assistant;
mod care;
#[cfg(windows)]
mod composition;
#[cfg(unix)]
mod composition;
#[cfg(windows)]
mod ctrlc_handler;
mod data_root;
#[cfg(any(unix, windows))]
mod errors;
mod intelligence;
// P36 (Hermes): router/protocol/streaming/performance are platform-neutral wire
// mapping and routing modules. They were module-gated to unix in P31 when only the
// unix composition consumed them; the Windows composition (composition.rs,
// scheduler.rs, server.rs) references the same crate-root modules, so they must
// compile on Windows too. Items that are genuinely unix-only inside them keep
// their own #[cfg(unix)] gates.
#[cfg(any(unix, windows))]
mod performance;
#[cfg(any(unix, windows))]
mod protocol;
#[cfg(any(unix, windows))]
mod router;
#[cfg(windows)]
mod scheduler;
#[cfg(windows)]
mod server;
#[cfg(any(unix, windows))]
mod streaming;
#[cfg(any(unix, windows))]
mod support;
mod timeline;
// Phase 27/31: unix composition — the same router served over a UDS transport.
// Phase 31 (W7): UNCONDITIONAL on unix (the manual feature flag is retired; the
// `unix-ipc` feature remains only as an accepted force-on alias for exotic hosts).
#[cfg(unix)]
mod unix_composition;
/// Phase 26 — unix service scaffolding (no launchd/systemd installation in this phase).
#[cfg(unix)]
mod unix_service;
#[cfg(windows)]
mod windows_service_host;

// DBT-P46-D1: one decider, shared with the IPC peer check and the installer
// hardener, which had each declared this independently.
// The only consumer is windows_service_host.rs, which reaches it through
// `use super::*` — so a macOS build sees an import with no local use.
#[cfg_attr(not(windows), allow(unused_imports))]
use aethercore_product_identity::SERVICE_NAME;

fn main() -> Result<()> {
    #[cfg(windows)]
    {
        let console_mode = std::env::args().any(|a| a == "--console");
        if console_mode {
            aethercore_diagnostics::init_console()?;
            return run_console();
        }
        // P75: the log is opened inside the service, after the SCM handler is registered.
        // Opened here, a failure (an unwritable logs directory) exited before the
        // dispatcher connected: SCM reported 1053 and nothing recorded why. The service
        // now reports SERVICE_STOPPED with an exit code instead. The writer rotates while
        // running (the old one rotated only at startup, so a long-lived service grew one
        // file without bound), as the unix daemon's already does.
        let log = log_path()
            .and_then(|path| aethercore_diagnostics::init_json_file_rotated(&path))
            .map_err(|error| error.to_string());
        return windows_service_host::run(log);
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
        return unix_service::run_unix_service(daemon, &data_path()?, &log_path()?);
    }
    #[allow(unreachable_code)]
    {
        anyhow::bail!("AetherCore maintenance service supports Windows and unix")
    }
}

fn product_data_root() -> Result<PathBuf> {
    data_root::resolve(
        std::env::args(),
        std::env::var("AETHERCORE_DATA_DIR").ok(),
        std::env::var_os("ProgramData"),
    )
}

fn data_path() -> Result<PathBuf> {
    Ok(product_data_root()?.join("state").join("aethercore.db"))
}
fn log_path() -> Result<PathBuf> {
    Ok(product_data_root()?.join("logs").join("service.jsonl"))
}

#[cfg(windows)]
fn run_console() -> Result<()> {
    let stop = Arc::new(AtomicBool::new(false));
    let s = stop.clone();
    ctrlc_handler::install(move || {
        s.store(true, Ordering::SeqCst);
        wake_pipe();
    })?;
    run_server(stop, || Ok(()))
}

/// `ready` runs once the operation kernel is composed, just before the pipe server starts:
/// the SCM host reports SERVICE_RUNNING there, not before a slow or failing composition.
#[cfg(windows)]
fn run_server(stop: Arc<AtomicBool>, ready: impl FnOnce() -> Result<()>) -> Result<()> {
    if let Err(e) = aethercore_restore_point::initialize_process_com_security() {
        warn!(error=%e,"System Restore COM security unavailable; protected driver installation will be blocked");
    }
    let root = product_data_root()?;
    let context = composition::build(&data_path()?, &root).context("compose operation kernel")?;
    let _idle_scheduler = match scheduler::start(&context) {
        Ok(handle) => Some(handle),
        Err(error) => {
            warn!(error=%error,"autonomous idle scheduler unavailable; interactive maintenance remains available");
            None
        }
    };
    ready()?;
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
