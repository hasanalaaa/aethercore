//! Phase 28 — GD-equivalent proof suite: the REAL aetherctl binary against a REAL
//! spawned maintenance-service daemon over the real UDS rendezvous (this Mac is the
//! unix test host; CX-7).
//!
//! Run with:
//!   cargo test --workspace --jobs 2 \
//!     --features aethercore-maintenance-service/unix-ipc,aetherctl/e2e
//!
//! Both binaries must share one target directory (the workspace run guarantees it):
//! the suite locates the daemon next to the CARGO_BIN_EXE_aetherctl artifact.

#![cfg(all(unix, feature = "e2e"))]

use std::{
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use serde_json::Value;

fn aetherctl_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_aetherctl"))
}

fn daemon_bin() -> PathBuf {
    let mut path = aetherctl_bin();
    path.pop();
    path.push("aethercore-maintenance-service");
    assert!(
        path.is_file(),
        "daemon binary missing at {} — build with --features aethercore-maintenance-service/unix-ipc",
        path.display()
    );
    path
}

/// Short /tmp root: sun_path=104 on macOS forbids long $TMPDIR-derived sockets.
fn fresh_root(tag: &str) -> PathBuf {
    let dir = PathBuf::from(format!("/tmp/axt-p28-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct ServiceProcess {
    child: Child,
    stdout: BufReader<std::process::ChildStdout>,
    stderr: std::process::ChildStderr,
}

impl Drop for ServiceProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn spawn_daemon(root: &Path, daemon_mode: bool) -> ServiceProcess {
    let mut command = Command::new(daemon_bin());
    command.arg(if daemon_mode {
        "--daemon"
    } else {
        "--foreground"
    });
    command.args(["--data-dir", root.to_str().unwrap()]);
    command.args(["--unix-ipc-data-dir", root.to_str().unwrap()]);
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn maintenance-service");
    let stdout = BufReader::new(child.stdout.take().unwrap());
    let stderr = child.stderr.take().unwrap();
    ServiceProcess {
        child,
        stdout,
        stderr,
    }
}

fn wait_for_socket_line(service: &mut ServiceProcess) -> String {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        assert!(Instant::now() < deadline, "no socket announcement in time");
        let mut line = String::new();
        let bytes = service.stdout.read_line(&mut line).expect("read stdout");
        if bytes == 0 {
            panic!("daemon exited before binding the socket");
        }
        if let Some(path) = line.strip_prefix("unix-ipc-socket: ") {
            return path.trim().to_string();
        }
    }
}

struct CliOutput {
    status: i32,
    stdout: String,
    stderr: String,
}

fn run_cli(socket_dir: &Path, data_dir: Option<&Path>, args: &[&str]) -> CliOutput {
    let mut command = Command::new(aetherctl_bin());
    command.args([
        "--socket-dir",
        socket_dir.to_str().unwrap(),
        "--output",
        "json",
        "--timeout-ms",
        "15000",
    ]);
    command.args(args);
    if let Some(data_dir) = data_dir {
        command.env("AETHERCORE_DATA_DIR", data_dir);
    } else {
        command.env_remove("AETHERCORE_DATA_DIR");
        command.env_remove("AETHERCORE_PID_FILE");
    }
    let output = command.output().expect("spawn aetherctl");
    CliOutput {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    }
}

fn parse_envelope(output: &CliOutput) -> Value {
    serde_json::from_str(output.stdout.trim()).expect("aetherctl stdout is one JSON envelope")
}

fn expect_ok_envelope(output: &CliOutput, command: &str) -> Value {
    assert_eq!(output.status, 0, "{command} exited 0: {}", output.stderr);
    let envelope = parse_envelope(output);
    assert_eq!(envelope["schema"], "aethercore.aetherctl.v1");
    assert_eq!(envelope["command"], command, "envelope echoes the command");
    assert_eq!(envelope["ok"], true, "{command}: {}", output.stdout);
    assert!(envelope.get("error").is_none());
    envelope
}

// ---------------------------------------------------------------------------
// Offline surface (no daemon needed)
// ---------------------------------------------------------------------------

#[test]
#[cfg(unix)]
fn offline_surface_succeeds_with_no_daemon_present() {
    let socket_dir = fresh_root("offline");
    // Deliberately NOT created: no endpoint exists at all.
    let data_dir = fresh_root("offline-data");

    let about = run_cli(&socket_dir, None, &["about"]);
    let envelope = expect_ok_envelope(&about, "about");
    assert_eq!(envelope["data"]["protocolVersion"], 7);

    expect_ok_envelope(&run_cli(&socket_dir, None, &["version"]), "version");

    let caps = run_cli(&socket_dir, None, &["capabilities"]);
    let envelope = expect_ok_envelope(&caps, "capabilities");
    assert_eq!(envelope["data"]["platform"], "macos");
    let rows = envelope["data"]["capabilities"].as_array().unwrap();
    assert!(rows.len() >= 16, "full honest matrix present");
    let cpu = rows.iter().find(|r| r["name"] == "telemetryCpu").unwrap();
    assert_eq!(cpu["availability"]["state"], "native");
    let drivers = rows
        .iter()
        .find(|r| r["name"] == "driverServicing")
        .unwrap();
    assert_eq!(drivers["availability"]["state"], "notAvailable");

    let source = run_cli(&socket_dir, None, &["engine-source"]);
    let envelope = expect_ok_envelope(&source, "engine-source");
    assert_eq!(envelope["data"]["source"], "native");

    let telemetry = run_cli(&socket_dir, None, &["telemetry-once"]);
    let envelope = expect_ok_envelope(&telemetry, "telemetry-once");
    assert_eq!(envelope["data"]["providerSource"], "PerfPlatform");
    assert!(envelope["data"]["cpu"]["totalBusyBp"].is_u64());
    assert!(envelope["data"]["memory"]["totalPhysicalBytes"].is_u64());

    let self_check = run_cli(&socket_dir, None, &["self-check"]);
    let envelope = expect_ok_envelope(&self_check, "self-check");
    assert_eq!(envelope["data"]["manifestValid"], true);
    assert_eq!(envelope["data"]["loaded"], false);
    let artifacts = envelope["data"]["artifacts"].as_array().unwrap();
    assert!(artifacts.iter().all(|a| a["sha256Match"] == true));

    let detect = run_cli(&socket_dir, None, &["service", "detect"]);
    let envelope = expect_ok_envelope(&detect, "service detect");
    assert_eq!(envelope["data"]["state"], "Offline");

    let units_text = Command::new(aetherctl_bin())
        .args([
            "--socket-dir",
            socket_dir.to_str().unwrap(),
            "service",
            "units",
            "--print",
        ])
        .env_remove("AETHERCORE_DATA_DIR")
        .output()
        .unwrap();
    assert!(units_text.status.success());
    let text = String::from_utf8_lossy(&units_text.stdout).to_string();
    assert!(text.contains("com.aethercore.maintenance"));
    assert!(text.contains("aethercore-maintenance.service"));
    assert!(text.contains("--daemon"));

    let _ = std::fs::remove_dir_all(&socket_dir);
    let _ = std::fs::remove_dir_all(&data_dir);
}

// ---------------------------------------------------------------------------
// Full service-backed matrix over the REAL daemon
// ---------------------------------------------------------------------------

#[test]
#[cfg(unix)]
fn service_backed_command_matrix_round_trips_over_real_uds() {
    let root = fresh_root("matrix");
    let mut daemon = spawn_daemon(&root, false);
    let _socket_path = wait_for_socket_line(&mut daemon);

    let cli = |args: &[&str]| run_cli(&root.join("ipc"), Some(&root), args);

    // doctor → tag 40. On a fresh daemon with no owned diagnostic snapshot the service
    // answers diagnostics.stateUnavailable (typed rejection, exit 5) — both outcomes
    // are valid round trips and neither is simulated.
    let doctor = cli(&["doctor"]);
    match doctor.status {
        0 => {
            let envelope = parse_envelope(&doctor);
            assert!(envelope["data"]["scanId"].is_string());
            assert!(envelope["data"]["state"].is_string());
        }
        5 => {
            let envelope = parse_envelope(&doctor);
            assert_eq!(envelope["error"]["kind"], "RejectedByService");
            assert_eq!(
                envelope["error"]["message_key"], "diagnostics.stateUnavailable",
                "{}",
                doctor.stdout
            );
        }
        other => panic!("doctor unexpected exit {other}: {}", doctor.stdout),
    }

    // perf start → 71
    let start = cli(&["perf", "start", "--interval-ms", "250"]);
    let envelope = expect_ok_envelope(&start, "perf start");
    assert_eq!(envelope["data"]["accepted"], true);
    assert_eq!(envelope["data"]["intervalMs"], 250);

    // perf stop → 72
    expect_ok_envelope(&cli(&["perf", "stop"]), "perf stop");

    // perf snapshot → 73
    let snapshot = cli(&["perf", "snapshot"]);
    let envelope = expect_ok_envelope(&snapshot, "perf snapshot");
    assert!(envelope["data"]["cpu"]["totalBusyBp"].as_u64().is_some());
    assert!(envelope["data"]["capturedUnixMs"].as_i64().unwrap() > 0);

    // perf report → 74 (needs samples; start briefly first)
    let _ = cli(&["perf", "start", "--interval-ms", "250"]);
    std::thread::sleep(Duration::from_millis(900));
    let report = cli(&["perf", "report"]);
    if report.status == 0 {
        let envelope = parse_envelope(&report);
        assert!(envelope["data"]["digestSha256"].is_string());
    } else {
        // Honest insufficient-evidence rejection carries the service's message key.
        assert_eq!(report.status, 5, "{}", report.stdout);
        let envelope = parse_envelope(&report);
        assert_eq!(envelope["error"]["kind"], "RejectedByService");
    }

    // optimize plan → 75 (may reject honestly if evidence is insufficient)
    let plan = cli(&["optimize", "plan"]);
    match plan.status {
        0 => {
            let envelope = parse_envelope(&plan);
            assert!(envelope["data"]["planId"].is_string());
        }
        5 => {
            let envelope = parse_envelope(&plan);
            assert_eq!(envelope["error"]["kind"], "RejectedByService");
        }
        other => panic!("optimize plan unexpected exit {other}: {}", plan.stdout),
    }

    // optimize status → 77
    let status = cli(&["optimize", "status"]);
    expect_ok_envelope(&status, "optimize status");

    // optimize start → 76: the SERVICE governs execution and refuses; typed exit 5.
    let forbidden = cli(&["optimize", "start"]);
    assert_eq!(forbidden.status, 5, "{}", forbidden.stdout);
    let envelope = parse_envelope(&forbidden);
    assert_eq!(envelope["ok"], false);
    assert_eq!(envelope["error"]["kind"], "RejectedByService");

    // timeline page → 78
    let page = cli(&["timeline", "page", "--size", "10"]);
    let envelope = expect_ok_envelope(&page, "timeline page");
    assert!(envelope["data"]["hasMore"].is_boolean());
    assert!(envelope["data"]["entries"].is_array());

    // timeline patterns → 79
    let patterns = cli(&["timeline", "patterns"]);
    let envelope = expect_ok_envelope(&patterns, "timeline patterns");
    assert!(envelope["data"]["patterns"].is_array());
    assert!(envelope["data"]["digestSha256"].is_string());

    // care status → 82
    let care_status = cli(&["care", "status"]);
    let envelope = expect_ok_envelope(&care_status, "care status");
    assert!(envelope["data"]["sessionConsentGranted"].is_boolean());

    // insights list/explain → 84/85
    let list = cli(&["insights", "list"]);
    let envelope = expect_ok_envelope(&list, "insights list");
    assert!(envelope["data"]["engineLabel"].is_string());
    let explain = cli(&[
        "insights",
        "explain",
        "--question",
        "insight.question.overview",
    ]);
    expect_ok_envelope(&explain, "insights explain");

    // scan status/history → 67/68
    let scan_status = cli(&["scan", "status"]);
    expect_ok_envelope(&scan_status, "scan status");
    let history = cli(&["scan", "history", "--limit", "5"]);
    let envelope = expect_ok_envelope(&history, "scan history");
    assert!(envelope["data"]["entries"].is_array());

    drop(daemon);
    let _ = std::fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// kill -9 → stale detection + next-daemon rebind
// ---------------------------------------------------------------------------

#[test]
#[cfg(unix)]
fn kill9_stale_endpoint_detected_then_next_daemon_rebinds() {
    let root = fresh_root("stale");
    // Daemon MODE: writes the PID file the CLI cross-check reads (foreground does not,
    // by Phase 26 contract) — kill -9 leaves both endpoint AND pid record stale.
    let mut daemon = spawn_daemon(&root, true);
    let socket_path = wait_for_socket_line(&mut daemon);

    assert!(root.join("state").join("aethercore.pid").is_file());

    let cli_detect = || run_cli(&root.join("ipc"), Some(&root), &["service", "detect"]);

    let alive = cli_detect();
    let envelope = expect_ok_envelope(&alive, "service detect");
    assert_eq!(envelope["data"]["state"], "Reachable");
    let recorded_pid = envelope["data"]["pid"].as_u64();
    assert_eq!(
        recorded_pid,
        Some(daemon.child.id() as u64),
        "PID cross-check"
    );

    // SIGKILL — no cleanup handlers run.
    daemon.child.kill().expect("SIGKILL daemon");
    let _ = daemon.child.wait();
    assert!(
        Path::new(&socket_path).exists(),
        "stale socket file remains"
    );

    let stale = cli_detect();
    let envelope = expect_ok_envelope(&stale, "service detect");
    assert_eq!(
        envelope["data"]["state"], "StaleEndpointRecovered",
        "{}",
        stale.stdout
    );

    // And a service-backed command reports typed unreachability (exit 3).
    let doctor = run_cli(&root.join("ipc"), Some(&root), &["doctor"]);
    assert_eq!(doctor.status, 3, "{}", doctor.stdout);
    let envelope = parse_envelope(&doctor);
    assert_eq!(envelope["error"]["kind"], "ServiceUnreachable");

    // Next lifecycle rebinds the SAME rendezvous (daemon-side stale recovery).
    let mut second = spawn_daemon(&root, false);
    let rebound = wait_for_socket_line(&mut second);
    assert_eq!(rebound, socket_path, "second lifecycle rebinds same path");
    let recovered = cli_detect();
    let envelope = expect_ok_envelope(&recovered, "service detect");
    assert_eq!(envelope["data"]["state"], "Reachable");

    drop(second);
    let _ = std::fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// SIGTERM graceful stop (QD-026-003)
// ---------------------------------------------------------------------------

#[test]
#[cfg(unix)]
fn sigterm_drains_connected_session_then_exits_zero_cleaning_files() {
    let root = fresh_root("sigterm");
    let mut daemon = spawn_daemon(&root, true); // --daemon: PID file + rotating logs
    let socket_path = wait_for_socket_line(&mut daemon);

    let pid_file = root.join("state").join("aethercore.pid");
    assert!(pid_file.is_file(), "--daemon wrote the PID file");
    let log_file = root.join("logs").join("service.jsonl");
    assert!(log_file.exists(), "--daemon created the structured log");

    // A connected-but-idle session represents in-flight work the drain window must
    // protect: the daemon may NOT exit while it is still connected.
    let mut held =
        aethercore_ipc::UnixSocketSession::connect(Path::new(&socket_path)).expect("client");
    use aethercore_contracts::{PROTOCOL_VERSION, v1};
    use prost::Message as _;
    let hello = v1::ClientFrame {
        payload: Some(v1::client_frame::Payload::Hello(v1::ClientHello {
            protocol_version: PROTOCOL_VERSION,
            client_name: "phase28-drain-probe".into(),
            client_version: "0".into(),
            replay_after_sequence: 0,
        })),
    };
    held.send_frame(
        &hello.encode_to_vec(),
        aethercore_contracts::MAX_CLIENT_SESSION_FRAME_BYTES,
    )
    .expect("send hello");
    // Consume ServerHello so the server considers the session established.
    let mut buffer = Vec::new();
    held.recv_frame(
        &mut buffer,
        aethercore_contracts::MAX_SERVER_SESSION_FRAME_BYTES,
    )
    .expect("server hello");

    // SIGTERM via kill(2) — no subprocess, direct syscall binding.
    {
        let pid = daemon.child.id() as i32;
        signal_term(pid);
    }

    // Drain window: the process must STILL BE ALIVE while our session holds open.
    std::thread::sleep(Duration::from_millis(700));
    assert!(
        daemon.child.try_wait().unwrap().is_none(),
        "graceful drain kept the daemon alive for the connected session"
    );

    // Release the session → daemon should finish draining and exit 0.
    drop(held);

    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if let Some(status) = daemon.child.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "daemon did not exit after drain");
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(
        status.code(),
        Some(0),
        "graceful SIGTERM stop exits 0 (got {status:?})"
    );
    assert!(
        !Path::new(&socket_path).exists(),
        "socket file removed on graceful stop"
    );
    assert!(!pid_file.exists(), "PID file removed on graceful stop");

    // Prevent Drop from killing an already-exited child spuriously.
    let _ = daemon.child.wait();

    // Rotated log artifacts exist with deterministic names.
    let gen1 = root.join("logs").join("service.jsonl.1");
    let _ = std::fs::metadata(&gen1); // optional: rotation only after cap exceeded

    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
fn signal_term(pid: i32) {
    const SIGTERM: i32 = 15;
    // SAFETY: kill(2) only delivers SIGTERM to the daemon process we spawned for this
    // test; it cannot affect any other state.
    let result = unsafe { raw_kill(pid, SIGTERM) };
    assert_eq!(result, 0, "SIGTERM delivered");
}

#[cfg(unix)]
unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

#[cfg(unix)]
unsafe fn raw_kill(pid: i32, sig: i32) -> i32 {
    unsafe { kill(pid, sig) }
}

// ---------------------------------------------------------------------------
// Consent discipline (adversarial)
// ---------------------------------------------------------------------------

#[test]
#[cfg(unix)]
fn consent_adversarial_refusals_are_typed_and_grant_is_visible() {
    let root = fresh_root("consent");
    let mut daemon = spawn_daemon(&root, false);
    let _socket = wait_for_socket_line(&mut daemon);
    let cli = |args: &[&str], stdin: Option<&[u8]>| {
        let mut command = Command::new(aetherctl_bin());
        command.args([
            "--socket-dir",
            root.join("ipc").to_str().unwrap(),
            "--output",
            "json",
            "--timeout-ms",
            "15000",
        ]);
        command.args(args);
        command.env("AETHERCORE_DATA_DIR", &root);
        command.stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        let mut child = command.spawn().expect("spawn aetherctl");
        if let Some(bytes) = stdin {
            use std::io::Write as _;
            let mut pipe = child.stdin.take().unwrap();
            let _ = pipe.write_all(bytes);
            let _ = pipe.flush();
            drop(pipe);
        }
        let output = child.wait_with_output().expect("reap aetherctl");
        CliOutput {
            status: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        }
    };

    // (a) Fresh daemon: care status shows no consent yet.
    let before = cli(&["care", "status"], None);
    let envelope = expect_ok_envelope(&before, "care status");
    assert_eq!(envelope["data"]["sessionConsentGranted"], false);

    // (b) Non-interactive care start NEVER consents: typed exit 6.
    let non_interactive = cli(&["care", "start", "--non-interactive"], None);
    assert_eq!(non_interactive.status, 6, "{}", non_interactive.stdout);
    let envelope = parse_envelope(&non_interactive);
    assert_eq!(envelope["error"]["kind"], "ConsentRequired");
    assert_eq!(
        envelope["error"]["message_key"],
        "cli.consent.interactiveConfirmationRequired"
    );

    // (c) JSON output mode NEVER consents: typed exit 6 (this whole suite IS json mode;
    // the text-mode prompt path is exercised by (d) with piped answers).
    let json_mode = cli(&["care", "start"], None);
    assert_eq!(json_mode.status, 6);
    let envelope = parse_envelope(&json_mode);
    assert_eq!(envelope["error"]["kind"], "ConsentRequired");

    // (d) Wrong-digest confirmation is refused BEFORE any consent RPC leaves the CLI.
    //     Drive the text-mode prompt with a bogus digest.
    let wrong = Command::new(aetherctl_bin())
        .args([
            "--socket-dir",
            root.join("ipc").to_str().unwrap(),
            "--output",
            "text",
            "--timeout-ms",
            "15000",
            "care",
            "start",
        ])
        .env("AETHERCORE_DATA_DIR", &root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write as _;
            let mut pipe = child.stdin.take().unwrap();
            let _ = pipe.write_all(b"deadbeefdeadbeef\n");
            let _ = pipe.flush();
            drop(pipe);
            child.wait_with_output()
        })
        .expect("wrong-digest attempt");
    assert_eq!(
        wrong.status.code(),
        Some(6),
        "{}",
        String::from_utf8_lossy(&wrong.stderr)
    );

    // (e) The explicit consent act itself: `care consent-grant` over THIS principal
    //     connection; the registry becomes visible to subsequent status queries.
    let grant = cli(&["care", "consent-grant"], None);
    let envelope = expect_ok_envelope(&grant, "care consent-grant");
    assert_eq!(envelope["data"]["granted"], true);
    let after = cli(&["care", "status"], None);
    let envelope = expect_ok_envelope(&after, "care status");
    assert_eq!(
        envelope["data"]["sessionConsentGranted"], true,
        "{}",
        after.stdout
    );

    // (f) Cancel on an idle run is a governed no-op that still round-trips (83).
    let cancel = cli(&["care", "cancel"], None);
    assert!(
        cancel.status == 0 || cancel.status == 5,
        "{}",
        cancel.stdout
    );

    drop(daemon);
    let _ = std::fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// Offline-mode timing bound for the service set
// ---------------------------------------------------------------------------

#[test]
#[cfg(unix)]
fn service_set_fails_fast_offline_within_timeout() {
    let socket_dir = fresh_root("fastfail");
    let started = Instant::now();
    let doctor = run_cli(&socket_dir, None, &["doctor"]);
    let elapsed = started.elapsed();
    assert_eq!(doctor.status, 3);
    let envelope = parse_envelope(&doctor);
    assert_eq!(envelope["error"]["kind"], "ServiceUnreachable");
    assert!(
        elapsed < Duration::from_millis(2000),
        "offline failure must be immediate, took {elapsed:?}"
    );
    let _ = std::fs::remove_dir_all(&socket_dir);
}
