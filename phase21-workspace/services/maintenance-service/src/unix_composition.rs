//! Phase 27 (T1) — Unix composition: the SAME router logic that serves named pipes on
//! Windows serves UDS frames here. One wire contract, byte-identical framing:
//!
//! - `UnixSocketListener::bind(default_socket_dir())` enforces the existing Phase 26
//!   `ensure_private_dir` contract (0700 dir, 0600 socket, stale-path recovery,
//!   live-stomp refusal) — no new endpoint semantics are introduced.
//! - Principal binding (CX-4/QD-026-001): the connecting peer is bound to the
//!   socket-owning user via the documented macOS permission boundary. The service runs
//!   as one user; the 0700/0600 pair means only that same OS user can connect, so the
//!   peer principal == the service's own uid. SO_PEERCRED deepening stays deferred and
//!   is NOT claimed.
//! - Session lifecycle mirrors `server.rs`: Hello handshake → ServerHello → replay →
//!   request workers under the kernel's cancellation registry. Streaming events reuse
//!   `streaming::publish`, so every RPC handler is transport-agnostic by construction.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};

use aethercore_contracts::{
    MAX_CLIENT_SESSION_FRAME_BYTES, MAX_SERVER_SESSION_FRAME_BYTES, PROTOCOL_VERSION,
    v1::{ClientFrame, ServerFrame, ServerHello, client_frame, server_frame},
};
use aethercore_ipc::Transport;
use chrono::Utc;
use prost::Message as _;
use tracing::{info, warn};
use uuid::Uuid;

use crate::router::ServiceContext;

const IPC_BOOTSTRAP_ENQUEUE_TIMEOUT: Duration = Duration::from_secs(2);
/// Mirrors server.rs session admission so unix never admits more persistent sessions
/// than the Windows pipe host would.
const MAX_UNIX_SESSIONS: usize = 32;
/// Phase 28 (QD-026-003 closure): after the stop flag fires, the accept loop stops
/// immediately but already-connected sessions get this bounded window to finish the
/// single in-flight request/response cycle before the process exits.
const GRACEFUL_DRAIN_WINDOW: Duration = Duration::from_secs(5);

static UNIX_SESSIONS: AtomicUsize = AtomicUsize::new(0);

/// Optional startup hook (set by the service binary) that announces the bound socket
/// path — used by integration tests and the daemon log to discover the rendezvous.
static SOCKET_ANNOUNCER: std::sync::Mutex<Option<Box<dyn Fn(&str) + Send>>> =
    std::sync::Mutex::new(None);

pub fn set_socket_announcer(f: Box<dyn Fn(&str) + Send>) {
    if let Ok(mut slot) = SOCKET_ANNOUNCER.lock() {
        *slot = Some(f);
    }
}

fn announce_socket(path: &std::path::Path) {
    if let Ok(slot) = SOCKET_ANNOUNCER.lock() {
        if let Some(f) = slot.as_ref() {
            f(path.to_str().unwrap_or(""));
        }
    }
}

struct UnixSessionGuard;
impl Drop for UnixSessionGuard {
    fn drop(&mut self) {
        UNIX_SESSIONS.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Resolves the single unix IPC rendezvous directory (Phase 26 contract).
pub fn default_socket_dir() -> std::path::PathBuf {
    aethercore_ipc::unix_impl::default_socket_dir()
}

/// Runs the maintenance-service accept loop over the unix domain socket until `stop`.
///
/// Composition note: this function receives the fully built Windows-grade
/// [`ServiceContext`] — the identical kernel, coordinators and performance engine that
/// the named-pipe host composes on Windows. There is exactly one router; only the
/// transport differs per cfg.
pub fn run_unix_server(
    stop: Arc<AtomicBool>,
    context: ServiceContext,
    data_dir_override: Option<&std::path::Path>,
) -> anyhow::Result<()> {
    let dir = data_dir_override
        .map(|root| root.join("ipc"))
        .unwrap_or_else(default_socket_dir);
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| anyhow::anyhow!("create ipc root {}: {error}", parent.display()))?;
    }
    // Phase 26 contract enforcement happens HERE on the rendezvous dir itself: create it
    // private (0700) before bind(), so ensure_private_dir inside bind() observes a 0700
    // directory regardless of the process umask.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if !dir.exists() {
            std::fs::create_dir_all(&dir)
                .map_err(|error| anyhow::anyhow!("create {}: {error}", dir.display()))?;
        }
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
            .map_err(|error| anyhow::anyhow!("chmod {}: {error}", dir.display()))?;
    }
    // bind() enforces ensure_private_dir (0700), removes a stale socket from a crashed
    // prior run after proving nothing lives behind it, and refuses to stomp a live one.
    let mut listener = aethercore_ipc::UnixSocketListener::bind(&dir)
        .map_err(|error| anyhow::anyhow!("bind unix IPC socket in {}: {error}", dir.display()))?;
    info!(
        socket = %listener.socket_path().display(),
        "operation kernel IPC v7 ready (unix domain socket)"
    );
    announce_socket(listener.socket_path());

    while !stop.load(Ordering::SeqCst) {
        // Poll-style accept: a bounded nonblocking window keeps the stop flag
        // responsive to SIGINT/SIGTERM even with zero connected clients.
        match listener.accept() {
            Ok(session) => {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                let previous = UNIX_SESSIONS.fetch_add(1, Ordering::SeqCst);
                if previous >= MAX_UNIX_SESSIONS {
                    UNIX_SESSIONS.fetch_sub(1, Ordering::SeqCst);
                    warn!("persistent unix IPC session cap reached; dropping client");
                    continue;
                }
                let ctx = context.clone();
                let spawned = thread::Builder::new()
                    .name("aether-unix-session".into())
                    .spawn(move || {
                        let _guard = UnixSessionGuard;
                        if let Err(error) = serve_unix_session(session, ctx) {
                            warn!(error = %error, "unix IPC session ended");
                        }
                    });
                if spawned.is_err() {
                    UNIX_SESSIONS.fetch_sub(1, Ordering::SeqCst);
                    warn!("failed to create unix IPC session worker");
                }
            }
            Err(error) => {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                // The bounded accept window elapses every 50 ms by design — that is the
                // shutdown-responsiveness poll, not a failure. Only real errors log.
                let is_window = error.to_string().contains("accept window elapsed");
                if !is_window {
                    warn!(error = %error, "unix socket accept failed");
                    thread::sleep(Duration::from_millis(200));
                }
            }
        }
    }
    // Phase 28 (QD-026-003): graceful drain — stop accepting (the loop above has
    // exited), then let connected sessions finish their in-flight frame within a
    // bounded window before the socket file is removed and the process exits.
    let drain_deadline = std::time::Instant::now() + GRACEFUL_DRAIN_WINDOW;
    while UNIX_SESSIONS.load(Ordering::SeqCst) > 0 && std::time::Instant::now() < drain_deadline {
        thread::sleep(Duration::from_millis(10));
    }
    listener.cleanup();
    info!("maintenance service stopped (unix)");
    Ok(())
}

fn serve_unix_session(
    mut session: aethercore_ipc::UnixSocketSession,
    context: ServiceContext,
) -> anyhow::Result<()> {
    // Principal identity is captured once at accept time and is immutable for the life
    // of this session. Boundary (CX-4/QD-026-001): the 0700 dir + 0600 socket restrict
    // connectivity to the socket-owning user, so the bound principal IS that OS user.
    // This does not claim deeper credential verification than the platform exposes.
    let peer = aethercore_security::socket_owner_principal();
    let owner = peer.binding_key();

    let first = recv_client_frame(&mut session)?;
    let hello = match first.payload {
        Some(client_frame::Payload::Hello(value)) => value,
        _ => anyhow::bail!("client hello required"),
    };
    let session_id = Uuid::new_v4().to_string();

    // Subscription installation and replay capture stay inside one EventBus critical
    // section — the same replay->live race-freedom argument as the Windows host.
    let (subscription, replay) = context
        .kernel
        .events()
        .subscribe(&owner, hello.replay_after_sequence);

    write_server_frame(
        &mut session,
        &ServerFrame {
            payload: Some(server_frame::Payload::Hello(ServerHello {
                protocol_version: PROTOCOL_VERSION,
                session_id: session_id.clone(),
                service_version: env!("CARGO_PKG_VERSION").into(),
                server_time_unix_ms: Utc::now().timestamp_millis(),
                current_sequence: replay.current_sequence,
                max_inflight_requests: 8,
                replay_floor_sequence: replay.replay_floor_sequence,
                replay_complete: replay.complete,
            })),
        },
    )?;

    if hello.protocol_version != PROTOCOL_VERSION {
        return Ok(());
    }

    if replay.complete {
        for event in replay.events {
            write_server_frame(
                &mut session,
                &ServerFrame {
                    payload: Some(server_frame::Payload::Event(event)),
                },
            )?;
        }
    } else {
        use aethercore_contracts::v1::{StreamReset, StreamResetReason};
        let reason = if hello.replay_after_sequence > replay.current_sequence {
            StreamResetReason::SequenceReset
        } else {
            StreamResetReason::ReplayWindowExceeded
        };
        write_server_frame(
            &mut session,
            &ServerFrame {
                payload: Some(server_frame::Payload::StreamReset(StreamReset {
                    reason: reason as i32,
                    current_sequence: replay.current_sequence,
                    replay_floor_sequence: replay.replay_floor_sequence,
                    message_key: "ipc.streamReset.hydrationRequired".into(),
                })),
            },
        )?;
    }

    // Request loop: one worker thread per in-flight request, bounded by the kernel's
    // cancellation registry — identical dispatch semantics to server.rs on Windows.
    loop {
        let frame = match recv_client_frame(&mut session) {
            Ok(frame) => frame,
            Err(_) => break,
        };
        match frame.payload {
            Some(client_frame::Payload::Cancel(cancel)) => {
                let scoped = format!("{session_id}:{}", cancel.cancellation_id);
                context.kernel.cancellations().cancel(&scoped);
            }
            Some(client_frame::Payload::Goodbye(_)) => break,
            Some(client_frame::Payload::Hello(_)) => {
                warn!(session_id = %session_id, "duplicate client hello ignored");
            }
            Some(client_frame::Payload::Request(mut envelope)) => {
                let Some(req) = envelope.request.take() else {
                    continue;
                };
                let now = Utc::now().timestamp_millis();
                let deadline = if envelope.deadline_unix_ms <= 0 {
                    now.saturating_add(aethercore_contracts::DEFAULT_REQUEST_DEADLINE_MS)
                } else {
                    envelope.deadline_unix_ms
                };
                let scoped = format!(
                    "{session_id}:{}",
                    if envelope.cancellation_id.trim().is_empty() {
                        format!(
                            "cancel:{}",
                            req.header
                                .as_ref()
                                .map(|h| h.request_id.as_str())
                                .unwrap_or("")
                        )
                    } else {
                        envelope.cancellation_id
                    }
                );
                let token = context.kernel.cancellations().try_register(&scoped)?;
                let request_context = aethercore_operation_kernel::RequestContext {
                    session_id: session_id.clone(),
                    request_id: req
                        .header
                        .as_ref()
                        .map(|header| header.request_id.clone())
                        .unwrap_or_default(),
                    owner_principal_key: owner.clone(),
                    deadline_unix_ms: deadline,
                    cancellation: token,
                };
                let ctx = context.clone();
                let peer = peer.clone();
                let writer_session = match session.try_clone() {
                    Ok(writer) => writer,
                    Err(error) => {
                        warn!(error = %error, "unix session clone failed; ending session");
                        break;
                    }
                };
                let cancels = context.kernel.cancellations().clone();
                let worker_scoped = scoped.clone();
                let spawn = thread::Builder::new()
                    .name("aether-unix-request".into())
                    .spawn(move || {
                        let response = ctx.handle(&peer, &request_context, req);
                        cancels.remove(&worker_scoped);
                        let mut writer = writer_session;
                        let _ = write_server_frame(
                            &mut writer,
                            &ServerFrame {
                                payload: Some(server_frame::Payload::Response(response)),
                            },
                        );
                    });
                if let Err(error) = spawn {
                    context.kernel.cancellations().remove(&scoped);
                    warn!(error = %error, "failed to create unix request worker");
                }
            }
            None => {}
        }
    }
    context.kernel.cancellations().cancel_session(&session_id);
    Ok(())
}

fn recv_client_frame(
    session: &mut aethercore_ipc::UnixSocketSession,
) -> anyhow::Result<ClientFrame> {
    let mut buffer = Vec::new();
    session.recv_frame(&mut buffer, MAX_CLIENT_SESSION_FRAME_BYTES)?;
    // TransportFrame already stripped the length prefix; decode the protobuf body
    // directly (decode_client_frame_bytes expects a still-framed payload).
    use prost::Message as _;
    let frame = ClientFrame::decode(buffer.as_slice())?;
    Ok(frame)
}

fn write_server_frame(
    session: &mut aethercore_ipc::UnixSocketSession,
    frame: &ServerFrame,
) -> anyhow::Result<()> {
    session.send_frame(&frame.encode_to_vec(), MAX_SERVER_SESSION_FRAME_BYTES)?;
    Ok(())
}
