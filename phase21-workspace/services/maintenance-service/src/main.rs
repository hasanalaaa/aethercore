use std::path::PathBuf;
#[cfg(windows)]
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[cfg(windows)]
use anyhow::Context;
use anyhow::Result;
#[cfg(windows)]
use tracing::warn;

mod assistant;
mod care;
#[cfg(windows)]
mod composition;
#[cfg(unix)]
mod composition;
#[cfg(windows)]
mod ctrlc_handler;
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

fn product_data_root() -> PathBuf {
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        if arg == "--data-dir"
            && let Some(value) = args.next()
            && !value.trim().is_empty()
        {
            return PathBuf::from(value);
        }
    }
    if let Ok(v) = std::env::var("AETHERCORE_DATA_DIR") {
        return PathBuf::from(v);
    }
    std::env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
        .join(aethercore_product_identity::PRODUCT_NAME)
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
