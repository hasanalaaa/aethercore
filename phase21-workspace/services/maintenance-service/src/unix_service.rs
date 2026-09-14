//! The unix service loop: `--foreground` and `--daemon`.

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
    for (name, availability) in
        aethercore_platform_capabilities::matrix_for_current_platform_observed(
            crate::performance::observe_telemetry(),
        )
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
    for (name, availability) in
        aethercore_platform_capabilities::matrix_for_current_platform_observed(
            crate::performance::observe_telemetry(),
        )
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
