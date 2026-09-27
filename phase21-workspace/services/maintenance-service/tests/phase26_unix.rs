//! Phase 27 — the unix composition now binds the REAL v7 socket when built with
//! `unix-ipc`; this test drives it as a long-lived child in both modes.

// The only test in this file is #[cfg(unix)]; on Windows the target still
// compiles, so ungated imports here read as unused there.
#[cfg(unix)]
use std::process::{Command, Stdio};
#[cfg(unix)]
use std::time::Duration;

#[test]
#[cfg(unix)]
fn foreground_run_prints_honest_capability_matrix() {
    let bin = std::path::Path::new(env!("CARGO_BIN_EXE_aethercore-maintenance-service"));
    // Short per-test dir: SUN_LEN-safe on macOS (sun_path is 104 bytes and the default
    // $TMPDIR alone eats ~70 of them); no clash with a running service.
    let ipc_root = std::path::PathBuf::from(format!("/tmp/axt-p26-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ipc_root);
    std::fs::create_dir_all(&ipc_root).expect("create ipc root");
    let mut child = Command::new(bin)
        .arg("--foreground")
        .env("AETHERCORE_IPC_ROOT", &ipc_root)
        // DBT-P60-003: without a data directory the service used to write its database
        // under a relative `C:\ProgramData` in the working directory; it now refuses.
        .env("AETHERCORE_DATA_DIR", ipc_root.join("data"))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn service");
    let mut stdout = std::io::BufReader::new(child.stdout.take().expect("piped stdout"));
    use std::io::BufRead as _;
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    let mut collected = String::new();
    // Phase 31 (W7): the transport is unconditional on unix now, so this branch is
    // ALWAYS taken on macOS/Linux hosts.
    let real_transport = cfg!(unix);
    if real_transport {
        // Real transport: wait for the machine-readable socket announcement.
        loop {
            assert!(
                std::time::Instant::now() < deadline,
                "service never announced its socket: {collected}"
            );
            let mut line = String::new();
            let bytes = stdout.read_line(&mut line).expect("read service line");
            assert!(bytes > 0, "service exited before announcing: {collected}");
            collected.push_str(&line);
            if line.starts_with("unix-ipc-socket: ") {
                break;
            }
        }
    } else {
        // Scaffold mode: the service prints its matrix then exits 0 on its own.
        loop {
            let mut line = String::new();
            let bytes = stdout.read_line(&mut line).expect("read service line");
            if bytes == 0 {
                break;
            }
            collected.push_str(&line);
        }
    }
    // Honest matrix spot checks — both directions of honesty:
    assert!(
        collected.contains("capability telemetryCpu: native"),
        "{collected}"
    );
    assert!(
        collected.contains("capability driverServicing: not-available"),
        "NotAvailable must be printed, never simulated: {collected}"
    );
    assert!(collected.contains("capability careOrchestration: native"));
    let _ = child.kill();
    let status = child.wait().expect("wait for service");
    assert!(
        status.code().is_none() || status.code() == Some(0),
        "foreground run exits cleanly: {status:?}"
    );
}
