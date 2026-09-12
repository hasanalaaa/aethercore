//! Phase 39 — ATTACK TESTS for the LocalSystem information-disclosure defect.
//!
//! These are written to FAIL against the unfixed router. They construct exactly the
//! request an unprivileged local caller would put on the wire and assert the service
//! REFUSES it.
//!
//! Why a unix socket proves a Windows defect: `router::handle_request` is one
//! platform-neutral function. The Windows named-pipe host and the unix-socket host both
//! dispatch into it (see `unix_composition.rs`'s module docs: "the SAME router logic
//! that serves named pipes on Windows serves UDS frames here"). The authorization
//! decision under test lives in that shared function, so exercising it over UDS
//! exercises the identical code the LocalSystem service runs. What UDS cannot reproduce
//! is the privilege GAP (here the peer is the same user as the service); the VM gate
//! covers that on the real installed product.
//!
//! Requires the `unix-ipc` feature (the unix composition only exists there).

#![cfg(all(unix, feature = "unix-ipc"))]

use std::{
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::Duration,
};

use aethercore_contracts::{
    MAX_CLIENT_SESSION_FRAME_BYTES, MAX_SERVER_SESSION_FRAME_BYTES, PROTOCOL_VERSION,
    v1::{
        self, ClientFrame, ClientHello, Request, RequestHeader, ServerFrame, SessionRequest,
        client_frame, server_frame,
    },
};
use aethercore_ipc::Transport;

/// The canonical AWS documentation example key. It matches the secrets provider's
/// `(AKIA|ASIA)[0-9A-Z]{16}` detector, so a scan that reaches it returns a finding whose
/// evidence carries the first four characters — the disclosure this test is about.
const PLANTED_SECRET: &str = "AKIAIOSFODNN7EXAMPLE";

fn service_bin() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_aethercore-maintenance-service"))
}

struct ServiceProcess {
    child: Child,
    stdout: BufReader<std::process::ChildStdout>,
    #[allow(dead_code)]
    stderr: std::process::ChildStderr,
}

impl Drop for ServiceProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn spawn_service(data_dir: &Path) -> ServiceProcess {
    let mut child = Command::new(service_bin())
        .args([
            "--foreground",
            "--unix-ipc-data-dir",
            data_dir.to_str().expect("utf8 data dir"),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn maintenance service");
    let stdout = child.stdout.take().expect("stdout");
    let stderr = child.stderr.take().expect("stderr");
    ServiceProcess {
        child,
        stdout: BufReader::new(stdout),
        stderr,
    }
}

fn wait_for_socket_line(service: &mut ServiceProcess) -> String {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        assert!(
            std::time::Instant::now() < deadline,
            "service did not report its socket path in time"
        );
        let mut line = String::new();
        let bytes = service
            .stdout
            .read_line(&mut line)
            .expect("read service stdout");
        if bytes == 0 {
            panic!("service exited before binding the socket");
        }
        if let Some(path) = line.strip_prefix("unix-ipc-socket: ") {
            return path.trim().to_string();
        }
    }
}

fn hello_frame() -> ClientFrame {
    ClientFrame {
        payload: Some(client_frame::Payload::Hello(ClientHello {
            protocol_version: PROTOCOL_VERSION,
            client_name: "phase39-authorization".into(),
            client_version: env!("CARGO_PKG_VERSION").into(),
            replay_after_sequence: 0,
        })),
    }
}

fn session_request(request: Request) -> ClientFrame {
    ClientFrame {
        payload: Some(client_frame::Payload::Request(SessionRequest {
            request: Some(request),
            deadline_unix_ms: 0,
            cancellation_id: String::new(),
        })),
    }
}

fn audit_request(request_id: &str, targets_json: &str) -> Request {
    Request {
        header: Some(RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.into(),
        }),
        payload: Some(v1::request::Payload::RunSecurityAudit(
            v1::RunSecurityAuditRequest {
                targets_json: targets_json.as_bytes().to_vec(),
            },
        )),
    }
}

fn export_journal_request(request_id: &str, owner_principal_key: &str) -> Request {
    Request {
        header: Some(RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.into(),
        }),
        payload: Some(v1::request::Payload::ExportJournal(
            v1::ExportJournalRequest {
                from_unix_ms: 0,
                to_unix_ms: 0,
                owner_principal_key: owner_principal_key.into(),
            },
        )),
    }
}

fn exchange(session: &mut aethercore_ipc::UnixSocketSession, frame: &ClientFrame) -> ServerFrame {
    use prost::Message as _;
    session
        .send_frame(&frame.encode_to_vec(), MAX_CLIENT_SESSION_FRAME_BYTES)
        .expect("send frame");
    let mut buffer = Vec::new();
    session
        .recv_frame(&mut buffer, MAX_SERVER_SESSION_FRAME_BYTES)
        .expect("recv frame");
    ServerFrame::decode(buffer.as_slice()).expect("decode server frame")
}

fn response_of(frame: ServerFrame) -> v1::Response {
    match frame.payload {
        Some(server_frame::Payload::Response(response)) => response,
        other => panic!("expected Response frame, got {other:?}"),
    }
}

/// Everything the attacker's request produced, rendered so a FAILING assertion prints
/// the actual disclosure rather than just a status code.
fn disclosure_report(response: &v1::Response) -> String {
    let mut out = format!("status_code={}", response.status_code);
    if let Some(error) = response.error.as_ref() {
        out.push_str(&format!(
            " error.message_key={:?} detail={:?}",
            error.message_key, error.technical_detail
        ));
    }
    if let Some(v1::response::Payload::SecurityAuditResponse(report)) = response.payload.as_ref() {
        for lane in &report.lanes {
            out.push_str(&format!(
                "\n  lane={} status={} findings={} body={}",
                lane.lane,
                lane.status,
                lane.finding_count,
                String::from_utf8_lossy(&lane.findings_json)
            ));
        }
    }
    out
}

/// `--unix-ipc-data-dir` only relocates the socket rendezvous; the product database and
/// data root stay machine-wide, so two service instances cannot run at once. Tests in
/// this file therefore take turns. Field order matters: the guard is declared last so it
/// is released only after the service child has been killed and reaped.
static ONE_SERVICE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct Fixture {
    _service: ServiceProcess,
    session: aethercore_ipc::UnixSocketSession,
    temp: PathBuf,
    /// A directory holding a planted credential, OUTSIDE any root the caller owns.
    victim_dir: PathBuf,
    /// A directory holding a planted credential, INSIDE the caller's own scope.
    own_dir: PathBuf,
    _turn: std::sync::MutexGuard<'static, ()>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.temp);
        let _ = std::fs::remove_dir_all(&self.own_dir);
    }
}

fn plant_secret(dir: &Path) {
    std::fs::create_dir_all(dir).expect("create planted secret dir");
    std::fs::write(
        dir.join("credentials"),
        format!("aws_access_key_id = {PLANTED_SECRET}\n"),
    )
    .expect("plant secret");
}

fn start() -> Fixture {
    // A previous test's panic must not block the rest of the file.
    let turn = ONE_SERVICE_AT_A_TIME
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    // Short /tmp root: macOS $TMPDIR (~70 chars) + socket path would exceed sun_path=104.
    let temp = PathBuf::from(format!(
        "/tmp/axt-p39-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    // OUTSIDE the caller's scope. On the unix composition the peer principal is the
    // socket-owning user, so its owner root is $HOME; /tmp is deliberately not under it.
    let victim_dir = temp.join("victim-home/.aws");
    plant_secret(&victim_dir);

    // INSIDE the caller's scope: the same content under the caller's own home, so the
    // legitimate path is proven to still work against identical data.
    let home = PathBuf::from(std::env::var_os("HOME").expect("HOME is set"));
    let own_dir = home.join(format!(
        ".cache/aethercore-p39-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    plant_secret(&own_dir);

    let mut service = spawn_service(&temp);
    let socket_path = wait_for_socket_line(&mut service);
    let mut session = aethercore_ipc::UnixSocketSession::connect(Path::new(&socket_path))
        .expect("client connects over the real unix socket");
    match exchange(&mut session, &hello_frame()).payload {
        Some(server_frame::Payload::Hello(_)) => {}
        other => panic!("expected ServerHello, got {other:?}"),
    }
    Fixture {
        _service: service,
        session,
        temp,
        victim_dir,
        own_dir,
        _turn: turn,
    }
}

fn secrets_targets(dir: &Path) -> String {
    format!(
        r#"[{{"kind":"secretsDir","dir":{}}}]"#,
        serde_json::to_string(&dir.to_string_lossy()).expect("json string")
    )
}

/// THE ATTACK. An unprivileged caller names an absolute path it does not own. The
/// service must refuse before any provider runs.
#[test]
fn run_security_audit_refuses_an_absolute_target_outside_the_calling_principal_scope() {
    let mut fixture = start();
    let targets = secrets_targets(&fixture.victim_dir.clone());
    let response = response_of(exchange(
        &mut fixture.session,
        &session_request(audit_request("p39-audit-attack-01", &targets)),
    ));

    assert_eq!(
        response.status_code,
        403,
        "an out-of-scope audit target must be REFUSED, not scanned.\n\
         The service read a path the caller does not own and returned what it found:\n{}",
        disclosure_report(&response)
    );
    assert!(
        response.payload.is_none(),
        "a refused audit must carry no report payload:\n{}",
        disclosure_report(&response)
    );
    assert_eq!(
        response
            .error
            .as_ref()
            .map(|e| e.message_key.as_str())
            .unwrap_or_default(),
        "sec.targetOutsideOwnerScope",
        "the refusal must be typed as an owner-scope denial:\n{}",
        disclosure_report(&response)
    );
}

/// The same attack shape through the multi-path variants, so the guard is proven to sit
/// on every path-bearing target rather than only on `secretsDir`.
#[test]
fn run_security_audit_refuses_out_of_scope_paths_in_every_target_variant() {
    let mut fixture = start();
    let victim_file = fixture.victim_dir.join("credentials");
    let victim = serde_json::to_string(&victim_file.to_string_lossy()).expect("json string");
    let cases = [
        (
            "sshdConfig",
            format!(r#"[{{"kind":"sshdConfig","path":{victim}}}]"#),
        ),
        (
            "sudoers",
            format!(r#"[{{"kind":"sudoers","path":{victim}}}]"#),
        ),
        (
            "passwordPolicy",
            format!(r#"[{{"kind":"passwordPolicy","path":{victim}}}]"#),
        ),
        (
            "authLogs",
            format!(r#"[{{"kind":"authLogs","paths":[{victim}]}}]"#),
        ),
        (
            "filesystemPaths",
            format!(r#"[{{"kind":"filesystemPaths","paths":[{victim}]}}]"#),
        ),
    ];
    for (index, (kind, targets)) in cases.iter().enumerate() {
        let response = response_of(exchange(
            &mut fixture.session,
            &session_request(audit_request(
                &format!("p39-audit-variant-{index:02}"),
                targets,
            )),
        ));
        assert_eq!(
            response.status_code,
            403,
            "target variant {kind} must be refused when it names a path outside the \
             caller's scope:\n{}",
            disclosure_report(&response)
        );
    }
}

/// A junction/symlink planted INSIDE an allowed root must be refused, not followed.
/// Same posture as `aethercore-install-hardener`'s purge-data path.
#[test]
fn run_security_audit_refuses_a_link_planted_inside_the_owner_root() {
    let mut fixture = start();
    let link = fixture.own_dir.join("escape");
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink(&fixture.victim_dir, &link).expect("plant link inside owner root");

    let targets = secrets_targets(&link);
    let response = response_of(exchange(
        &mut fixture.session,
        &session_request(audit_request("p39-audit-link-01", &targets)),
    ));

    assert_eq!(
        response.status_code,
        403,
        "a link inside an allowed root must be refused rather than followed out of \
         scope:\n{}",
        disclosure_report(&response)
    );
    assert_eq!(
        response
            .error
            .as_ref()
            .map(|e| e.message_key.as_str())
            .unwrap_or_default(),
        "sec.reparsePointRefused",
        "the refusal must name the link as the cause:\n{}",
        disclosure_report(&response)
    );
}

/// THE LEGITIMATE PATH. A caller auditing its OWN scope still works end to end, and the
/// lane really runs — it finds the planted credential rather than merely not refusing.
#[test]
fn run_security_audit_still_serves_a_caller_auditing_its_own_scope() {
    let mut fixture = start();
    let targets = secrets_targets(&fixture.own_dir.clone());
    let response = response_of(exchange(
        &mut fixture.session,
        &session_request(audit_request("p39-audit-legit-01", &targets)),
    ));

    assert_eq!(
        response.status_code,
        0,
        "auditing the caller's own scope must succeed:\n{}",
        disclosure_report(&response)
    );
    let report = match response.payload.as_ref() {
        Some(v1::response::Payload::SecurityAuditResponse(report)) => report,
        other => panic!("expected SecurityAuditResponse, got {other:?}"),
    };
    let secrets = report
        .lanes
        .iter()
        .find(|lane| lane.lane == "secrets")
        .expect("secrets lane present");
    assert_eq!(secrets.status, "ok", "secrets lane ran: {secrets:?}");
    assert!(
        secrets.finding_count >= 1,
        "the caller's own scope is really scanned, not silently narrowed away: {secrets:?}"
    );
}

/// THE SECOND ATTACK. A caller supplies another owner's binding key. The service must
/// refuse: no authorization exists that permits reading another owner's records.
#[test]
fn export_journal_refuses_a_foreign_owner_principal_key() {
    let mut fixture = start();
    // Shape-valid but not ours: `binding_key()` is a 64-hex SHA-256.
    let foreign = "f".repeat(64);
    let response = response_of(exchange(
        &mut fixture.session,
        &session_request(export_journal_request("p39-journal-attack-1", &foreign)),
    ));

    assert_eq!(
        response.status_code,
        403,
        "a foreign owner_principal_key must be refused, not honoured:\n{}",
        disclosure_report(&response)
    );
    assert!(
        response.payload.is_none(),
        "a refused export must carry no envelope:\n{}",
        disclosure_report(&response)
    );
    assert_eq!(
        response
            .error
            .as_ref()
            .map(|e| e.message_key.as_str())
            .unwrap_or_default(),
        "journal.ownerScopeForbidden",
        "the refusal must be typed as an owner-scope denial:\n{}",
        disclosure_report(&response)
    );
}

/// The legitimate journal path: empty owner means "my own records" and still works.
#[test]
fn export_journal_still_serves_the_calling_principals_own_records() {
    let mut fixture = start();
    let response = response_of(exchange(
        &mut fixture.session,
        &session_request(export_journal_request("p39-journal-legit-1", "")),
    ));
    assert_eq!(
        response.status_code,
        0,
        "an empty owner_principal_key is the caller's own scope and must still \
         work:\n{}",
        disclosure_report(&response)
    );
    match response.payload.as_ref() {
        Some(v1::response::Payload::ExportJournalResponse(_)) => {}
        other => panic!("expected ExportJournalResponse, got {other:?}"),
    }
}
