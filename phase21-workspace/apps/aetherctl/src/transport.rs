//! Transport layer (T2): reuse, never reinvent.
//!
//! - cfg(unix): `aethercore_ipc::UnixSocketSession` carries the byte-identical v7
//!   framing/auth contract (4-byte LE length prefix + protobuf body; the 0700/0600
//!   permission boundary is enforced by the listener side).
//! - cfg(windows): the existing frozen named-pipe client (`aethercore_ipc::SessionClient`)
//!   is reused unchanged.
//!
//! Service detection combines a live socket probe with a PID-file cross-check into three
//! typed states (see docs/phase28/ARCHITECTURE.md §4):
//!   Reachable { pid }        — endpoint answers; pid present when a live PID file exists
//!   Offline                  — no endpoint and no live daemon on record
//!   StaleEndpointRecovered   — endpoint file exists but nothing lives behind it
//!                              (leftover of SIGKILL/crash; the daemon rebinds it on its
//!                              next start — proven by the Phase 28 proof suite)

use crate::cli::Config;
use crate::error::CliError;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use aethercore_ipc::unix_impl::default_socket_dir;

#[cfg(unix)]
use aethercore_contracts::v1::{ClientFrame, ClientHello, client_frame, server_frame};

/// Socket file name served by the maintenance service (crates/ipc contract).
pub const SOCKET_FILE_NAME: &str = "aethercore-maintenance.sock";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceState {
    Reachable { pid: Option<u32> },
    Offline,
    StaleEndpointRecovered,
}

impl ServiceState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ServiceState::Reachable { .. } => "Reachable",
            ServiceState::Offline => "Offline",
            ServiceState::StaleEndpointRecovered => "StaleEndpointRecovered",
        }
    }

    pub fn message_key(&self) -> &'static str {
        match self {
            ServiceState::Reachable { .. } => "cli.detect.reachable",
            ServiceState::Offline => "cli.detect.offline",
            ServiceState::StaleEndpointRecovered => "cli.detect.staleEndpoint",
        }
    }
}

#[cfg(unix)]
pub fn default_endpoint_dir() -> std::path::PathBuf {
    default_socket_dir()
}

/// The rendezvous directory a command actually probes (honors --socket-dir).
#[cfg(unix)]
pub fn endpoint_dir(config: &Config) -> std::path::PathBuf {
    config
        .socket_dir
        .clone()
        .unwrap_or_else(default_endpoint_dir)
}

#[cfg(windows)]
pub fn endpoint_dir(config: &Config) -> std::path::PathBuf {
    // The Windows lane rides the frozen named pipe; --socket-dir does not apply.
    let _ = config;
    default_endpoint_dir()
}

#[cfg(windows)]
pub fn default_endpoint_dir() -> std::path::PathBuf {
    // The Windows lane rides the frozen named pipe; the dir concept only backs the
    // PID-file cross-check mirror of product_data_root().
    match std::env::var_os("AETHERCORE_DATA_DIR") {
        Some(dir) => std::path::PathBuf::from(dir),
        None => std::env::var_os("ProgramData")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from(r"C:\ProgramData"))
            .join(aethercore_product_identity::PRODUCT_NAME),
    }
}

#[cfg(unix)]
pub fn endpoint_path(config: &Config) -> std::path::PathBuf {
    config
        .socket_dir
        .clone()
        .unwrap_or_else(default_endpoint_dir)
        .join(SOCKET_FILE_NAME)
}

/// Resolves the daemon PID file exactly like services/maintenance-service writes it:
/// `<product data root>/state/aethercore.pid`, root from $AETHERCORE_DATA_DIR (or an
/// explicit $AETHERCORE_PID_FILE override for tests and custom installs).
pub fn pid_file_path() -> Option<std::path::PathBuf> {
    if let Ok(explicit) = std::env::var("AETHERCORE_PID_FILE")
        && !explicit.trim().is_empty()
    {
        return Some(std::path::PathBuf::from(explicit));
    }
    if let Ok(dir) = std::env::var("AETHERCORE_DATA_DIR")
        && !dir.trim().is_empty()
    {
        return Some(
            std::path::PathBuf::from(dir)
                .join("state")
                .join("aethercore.pid"),
        );
    }
    #[cfg(windows)]
    {
        return Some(
            std::env::var_os("ProgramData")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from(r"C:\ProgramData"))
                .join(aethercore_product_identity::PRODUCT_NAME)
                .join("state")
                .join("aethercore.pid"),
        );
    }
    #[cfg(not(windows))]
    None
}

/// Liveness cross-check: signals nothing, just probes (kill(pid, 0)).
#[cfg(unix)]
fn pid_alive(pid: u32) -> bool {
    // SAFETY: libc::kill with signal 0 performs a pure permission/liveness probe and
    // cannot deliver anything to the target process.
    let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(windows)]
fn pid_alive(_pid: u32) -> bool {
    // Honest limitation of the frozen Windows CLI lane (QD-028-001): the recorded pid is
    // reported without a liveness probe; the socket/pipe handshake remains the authority.
    true
}

/// Reads + cross-checks the daemon PID file. Returns Some(pid) only when the recorded
/// process is alive.
pub fn read_live_pid() -> Option<u32> {
    let path = pid_file_path()?;
    let raw = std::fs::read_to_string(path).ok()?;
    let pid: u32 = raw.trim().parse().ok()?;
    if pid_alive(pid) { Some(pid) } else { None }
}

// ---------------------------------------------------------------------------
// Client session
// ---------------------------------------------------------------------------

static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_request_id() -> String {
    format!(
        "aetherctl-{}-{}",
        std::process::id(),
        REQUEST_COUNTER.fetch_add(1, Ordering::SeqCst)
    )
}

fn now_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub struct CallOutcome {
    pub status_code: u32,
    pub error_message_key: Option<String>,
    pub error_detail: Option<String>,
    pub payload: Option<aethercore_contracts::v1::response::Payload>,
}

/// One-shot command session over the shared wire contract.
pub struct ServiceClient {
    #[cfg(unix)]
    inner: Option<aethercore_ipc::UnixSocketSession>,
    #[cfg(windows)]
    inner: Option<std::sync::Arc<aethercore_ipc::SessionClient>>,
    timeout: Duration,
}

impl ServiceClient {
    /// Connects within the configured budget or maps to typed unreachable errors.
    /// Never blocks beyond --timeout-ms.
    pub fn connect(config: &Config) -> Result<Self, CliError> {
        #[cfg(unix)]
        {
            let path = endpoint_path(config);
            if !path.exists() {
                return Err(CliError::service_unreachable("cli.detect.offline"));
            }
            let deadline = Instant::now() + config.timeout;
            let mut last = String::new();
            let session = loop {
                match aethercore_ipc::UnixSocketSession::connect(&path) {
                    Ok(session) => break session,
                    Err(error) => {
                        last = error.to_string();
                        match error {
                            aethercore_ipc::TransportError::EndpointStale(_) => {
                                return Err(CliError::service_unreachable(
                                    "cli.detect.staleEndpoint",
                                ));
                            }
                            aethercore_ipc::TransportError::PermissionDenied(_) => {
                                return Err(CliError::service_unreachable(
                                    "cli.detect.endpointPermissionDenied",
                                ));
                            }
                            _ => {}
                        }
                    }
                }
                if Instant::now() >= deadline {
                    return Err(CliError::service_unreachable(
                        "cli.detect.connectDeadlineElapsed",
                    ));
                }
                std::thread::sleep(Duration::from_millis(25));
            };
            let _ = &last;
            session.set_io_timeouts(config.timeout).map_err(|error| {
                CliError::local_io_with("cli.transport.timeoutSetup", error.to_string())
            })?;
            let mut client = ServiceClient {
                inner: Some(session),
                timeout: config.timeout,
            };
            client.handshake()?;
            Ok(client)
        }
        #[cfg(windows)]
        {
            let _ = config.socket_dir;
            let noop_event = std::sync::Arc::new(|_: aethercore_contracts::v1::EventEnvelope| {})
                as std::sync::Arc<dyn Fn(aethercore_contracts::v1::EventEnvelope) + Send + Sync>;
            let noop_reset = std::sync::Arc::new(|_: aethercore_contracts::v1::StreamReset| {})
                as std::sync::Arc<dyn Fn(aethercore_contracts::v1::StreamReset) + Send + Sync>;
            let noop_disconnect =
                std::sync::Arc::new(|| {}) as std::sync::Arc<dyn Fn() + Send + Sync>;
            let client = aethercore_ipc::SessionClient::connect(
                "aetherctl",
                env!("CARGO_PKG_VERSION"),
                0,
                noop_event,
                noop_reset,
                noop_disconnect,
            )
            .map_err(|_| CliError::service_unreachable("cli.detect.offline"))?;
            Ok(ServiceClient {
                inner: Some(client),
                timeout: config.timeout,
            })
        }
    }

    #[cfg(unix)]
    fn handshake(&mut self) -> Result<(), CliError> {
        use prost::Message as _;
        let frame = ClientFrame {
            payload: Some(client_frame::Payload::Hello(ClientHello {
                protocol_version: aethercore_contracts::PROTOCOL_VERSION,
                client_name: "aetherctl".into(),
                client_version: env!("CARGO_PKG_VERSION").into(),
                replay_after_sequence: 0,
            })),
        };
        let encoded = frame.encode_to_vec();
        self.send_raw(&encoded)?;
        let reply = self.recv_server_frame()?;
        match reply.payload {
            Some(server_frame::Payload::Hello(hello)) => {
                if hello.protocol_version != aethercore_contracts::PROTOCOL_VERSION {
                    return Err(CliError::ProtocolViolation {
                        detail: format!(
                            "server protocol {} != client protocol {}",
                            hello.protocol_version,
                            aethercore_contracts::PROTOCOL_VERSION
                        ),
                    });
                }
                Ok(())
            }
            _ => Err(CliError::ProtocolViolation {
                detail: "expected ServerHello after ClientHello".to_string(),
            }),
        }
    }

    #[cfg(windows)]
    fn handshake(&mut self) -> Result<(), CliError> {
        // SessionClient performs the handshake inside connect(); nothing further here.
        Ok(())
    }

    #[cfg(unix)]
    fn send_raw(&mut self, encoded: &[u8]) -> Result<(), CliError> {
        let session = self
            .inner
            .as_mut()
            .ok_or_else(|| CliError::ProtocolViolation {
                detail: "session already closed".to_string(),
            })?;
        session
            .send_frame(
                encoded,
                aethercore_contracts::MAX_CLIENT_SESSION_FRAME_BYTES,
            )
            .map_err(map_transport)
    }

    #[cfg(unix)]
    fn recv_server_frame(&mut self) -> Result<aethercore_contracts::v1::ServerFrame, CliError> {
        use prost::Message as _;
        let session = self
            .inner
            .as_mut()
            .ok_or_else(|| CliError::ProtocolViolation {
                detail: "session already closed".to_string(),
            })?;
        let mut buffer = Vec::new();
        session
            .recv_frame(
                &mut buffer,
                aethercore_contracts::MAX_SERVER_SESSION_FRAME_BYTES,
            )
            .map_err(map_transport)?;
        aethercore_contracts::v1::ServerFrame::decode(buffer.as_slice()).map_err(|error| {
            CliError::ProtocolViolation {
                detail: format!("server frame decode failed: {error}"),
            }
        })
    }

    /// One request/response cycle. Streaming events interleaved before the response are
    /// drained (they are advisory duplicates of state the response itself carries).
    pub fn call(
        &mut self,
        payload: aethercore_contracts::v1::request::Payload,
    ) -> Result<CallOutcome, CliError> {
        let request = aethercore_contracts::v1::Request {
            header: Some(aethercore_contracts::v1::RequestHeader {
                protocol_version: aethercore_contracts::PROTOCOL_VERSION,
                request_id: next_request_id(),
            }),
            payload: Some(payload),
        };
        let deadline_unix_ms = now_unix_ms()
            .saturating_add(self.timeout.as_millis() as i64)
            .min(now_unix_ms() + aethercore_contracts::MAX_REQUEST_DEADLINE_MS);
        let envelope = aethercore_contracts::v1::SessionRequest {
            request: Some(request),
            deadline_unix_ms,
            cancellation_id: String::new(),
        };

        #[cfg(unix)]
        {
            use prost::Message as _;
            let frame = ClientFrame {
                payload: Some(client_frame::Payload::Request(envelope)),
            };
            self.send_raw(&frame.encode_to_vec())?;
            loop {
                let reply = self.recv_server_frame()?;
                match reply.payload {
                    Some(server_frame::Payload::Response(response)) => {
                        return Ok(CallOutcome {
                            status_code: response.status_code,
                            error_message_key: response
                                .error
                                .as_ref()
                                .map(|e| e.message_key.clone()),
                            error_detail: response
                                .error
                                .as_ref()
                                .map(|e| e.technical_detail.clone()),
                            payload: response.payload,
                        });
                    }
                    Some(server_frame::Payload::Event(_)) => continue,
                    Some(server_frame::Payload::StreamReset(reset)) => {
                        return Err(CliError::ProtocolViolation {
                            detail: format!("stream reset: {}", reset.message_key),
                        });
                    }
                    Some(server_frame::Payload::Hello(_)) => {
                        return Err(CliError::ProtocolViolation {
                            detail: "unexpected duplicate ServerHello".to_string(),
                        });
                    }
                    None => continue,
                }
            }
        }
        #[cfg(windows)]
        {
            let client = self
                .inner
                .clone()
                .ok_or_else(|| CliError::ProtocolViolation {
                    detail: "session already closed".to_string(),
                })?;
            let wire_request = envelope
                .request
                .ok_or_else(|| CliError::ProtocolViolation {
                    detail: "session request lost its payload".to_string(),
                })?;
            let response = client
                .request(wire_request, self.timeout)
                .map_err(|error| CliError::Rejected {
                    message_key: format!("cli.windowsLane.{error}"),
                    detail: Some(error.to_string()),
                })?;
            Ok(CallOutcome {
                status_code: response.status_code,
                error_message_key: response.error.as_ref().map(|e| e.message_key.clone()),
                error_detail: response.error.as_ref().map(|e| e.technical_detail.clone()),
                payload: response.payload,
            })
        }
    }
}

#[cfg(unix)]
fn map_transport(error: aethercore_ipc::TransportError) -> CliError {
    match error {
        aethercore_ipc::TransportError::PeerDisconnectedMidFrame { .. } => {
            CliError::service_unreachable("cli.detect.peerDisconnectedMidFrame")
        }
        other => {
            let text = other.to_string();
            if text.contains("timed out") || text.contains("Resource temporarily unavailable") {
                CliError::Timeout
            } else {
                CliError::local_io_with("cli.transport.io", text)
            }
        }
    }
}

/// Detects the service state WITHOUT mutating anything (read-only probe).
#[cfg(unix)]
pub fn detect_service(config: &Config) -> ServiceState {
    let path = endpoint_path(config);
    if !path.exists() {
        return match read_live_pid() {
            Some(pid) => ServiceState::Reachable { pid: Some(pid) },
            None => ServiceState::Offline,
        };
    }
    match aethercore_ipc::UnixSocketSession::connect(&path) {
        Ok(session) => {
            let _ = session.set_io_timeouts(Duration::from_millis(750));
            drop(session);
            ServiceState::Reachable {
                pid: read_live_pid(),
            }
        }
        Err(_) => {
            // Endpoint file exists but refuses connection while no live daemon holds the
            // PID record: the stale-endpoint signature (daemon-side recovery rebinds it
            // on next start; the CLI never deletes anything).
            match read_live_pid() {
                Some(pid) => ServiceState::Reachable { pid: Some(pid) },
                None => ServiceState::StaleEndpointRecovered,
            }
        }
    }
}

#[cfg(windows)]
pub fn detect_service(config: &Config) -> ServiceState {
    match ServiceClient::connect(config) {
        Ok(_) => ServiceState::Reachable {
            pid: read_live_pid(),
        },
        Err(_) => ServiceState::Offline,
    }
}
