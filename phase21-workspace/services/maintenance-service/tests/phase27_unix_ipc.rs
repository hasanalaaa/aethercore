//! Phase 27 — GD gate: live unix-socket IPC round trip against the REAL service binary.
//!
//! Spawns `aethercore-maintenance-service --foreground --unix-ipc-test` with an isolated
//! data dir, connects a real `UnixSocketSession` client through the full v7 handshake
//! (Hello → ServerHello), then executes GetPlatformCapabilities + Ping + GetEngineSource
//! request/response cycles over the actual socket. Also proves stale-path recovery: a
//! second service start must rebind the socket path left behind by the first.
//!
//! This test requires the `unix-ipc` feature (the composition only exists there).

#![cfg(all(unix, feature = "unix-ipc"))]

use std::{
    io::{BufRead, BufReader},
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

fn service_bin() -> &'static std::path::Path {
    std::path::Path::new(env!("CARGO_BIN_EXE_aethercore-maintenance-service"))
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

fn spawn_service(data_dir: &std::path::Path) -> ServiceProcess {
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
            client_name: "phase27-integration".into(),
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

fn ping_request() -> Request {
    Request {
        header: Some(RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "gd-ping-0000000001".into(),
        }),
        payload: Some(aethercore_contracts::v1::request::Payload::Ping(
            v1::PingRequest {},
        )),
    }
}

fn capabilities_request() -> Request {
    Request {
        header: Some(RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "gd-caps-0000000001".into(),
        }),
        payload: Some(
            aethercore_contracts::v1::request::Payload::GetPlatformCapabilities(
                v1::GetPlatformCapabilitiesRequest {},
            ),
        ),
    }
}

fn engine_source_request() -> Request {
    Request {
        header: Some(RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: "gd-src-00000000001".into(),
        }),
        payload: Some(aethercore_contracts::v1::request::Payload::GetEngineSource(
            v1::GetEngineSourceRequest {},
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

#[test]
#[cfg(unix)]
fn unix_socket_round_trip_capabilities_ping_engine_source() {
    use prost::Message as _;
    use std::os::unix::fs::PermissionsExt;

    // Short /tmp root: macOS $TMPDIR (~70 chars) + socket path would exceed sun_path=104.
    let temp = std::path::PathBuf::from(format!("/tmp/axt-gd-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    // First lifecycle.
    let mut service = spawn_service(&temp);
    let socket_path = wait_for_socket_line(&mut service);

    // The private-dir contract holds on the REAL bound socket: 0700 dir, 0600 file.
    let dir_meta = std::fs::metadata(&temp.join("ipc")).expect("ipc dir");
    assert_eq!(dir_meta.permissions().mode() & 0o777, 0o700, "dir is 0700");
    let sock_meta = std::fs::metadata(&socket_path).expect("socket file");
    assert_eq!(
        sock_meta.permissions().mode() & 0o777,
        0o600,
        "socket is 0600"
    );

    let mut session =
        aethercore_ipc::UnixSocketSession::connect(std::path::Path::new(&socket_path))
            .expect("client connects over the real unix socket");

    // Full v7 handshake.
    let hello = exchange(&mut session, &hello_frame());
    match hello.payload {
        Some(server_frame::Payload::Hello(hello)) => {
            assert_eq!(hello.protocol_version, PROTOCOL_VERSION);
            assert!(!hello.session_id.is_empty());
        }
        other => panic!("expected ServerHello, got {other:?}"),
    }

    // Ping round trip over the REAL socket.
    let pong = exchange(&mut session, &session_request(ping_request()));
    match pong.payload {
        Some(server_frame::Payload::Response(response)) => {
            assert_eq!(response.status_code, 0, "ping succeeds: {response:?}");
            match response.payload {
                Some(v1::response::Payload::Pong(pong)) => {
                    assert!(!pong.service_version.is_empty());
                }
                other => panic!("expected PongResponse, got {other:?}"),
            }
        }
        other => panic!("expected Response frame, got {other:?}"),
    }

    // GetPlatformCapabilities round trip — the honest matrix over the wire.
    let caps = exchange(&mut session, &session_request(capabilities_request()));
    match caps.payload {
        Some(server_frame::Payload::Response(response)) => {
            assert_eq!(response.status_code, 0, "capabilities succeed");
            match response.payload {
                Some(v1::response::Payload::PlatformCapabilitiesResponse(matrix)) => {
                    assert_eq!(matrix.platform, "macos");
                    let cpu = matrix
                        .capabilities
                        .iter()
                        .find(|c| c.name == "telemetryCpu")
                        .expect("telemetryCpu row present");
                    let availability = cpu.availability.as_ref().expect("availability set");
                    assert_eq!(availability.state, "native");
                    let drivers = matrix
                        .capabilities
                        .iter()
                        .find(|c| c.name == "driverServicing")
                        .expect("driverServicing row present");
                    let availability = drivers.availability.as_ref().expect("availability set");
                    assert_eq!(availability.state, "notAvailable");
                }
                other => panic!("expected PlatformCapabilitiesResponse, got {other:?}"),
            }
        }
        other => panic!("expected Response frame, got {other:?}"),
    }

    // GetEngineSource round trip — native on this macOS host.
    let source = exchange(&mut session, &session_request(engine_source_request()));
    match source.payload {
        Some(server_frame::Payload::Response(response)) => {
            assert_eq!(response.status_code, 0, "engine source succeeds");
            match response.payload {
                Some(v1::response::Payload::EngineSourceResponse(source)) => {
                    assert_eq!(source.source, "native");
                    assert_eq!(source.platform, "macos");
                }
                other => panic!("expected EngineSourceResponse, got {other:?}"),
            }
        }
        other => panic!("expected Response frame, got {other:?}"),
    }

    drop(session);
    drop(service);

    // Second lifecycle: the first process was killed WITHOUT cleanup, so the socket
    // file still exists. The new bind MUST recover from that stale path (typed stale
    // recovery, not silent reuse and not a failure).
    assert!(
        std::path::Path::new(&socket_path).exists(),
        "stale socket file left behind for recovery proof"
    );
    let mut second = spawn_service(&temp);
    let rebound = wait_for_socket_line(&mut second);
    assert_eq!(
        rebound, socket_path,
        "second lifecycle rebinds the same rendezvous"
    );
    // And it actually serves again.
    let mut session2 = aethercore_ipc::UnixSocketSession::connect(std::path::Path::new(&rebound))
        .expect("reconnect after restart");
    let hello2 = exchange(&mut session2, &hello_frame());
    matches!(hello2.payload, Some(server_frame::Payload::Hello(_)));
}
