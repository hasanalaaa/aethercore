use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};

use aethercore_contracts::{
    DEFAULT_REQUEST_DEADLINE_MS, MAX_REQUEST_DEADLINE_MS, PROTOCOL_VERSION,
    v1::{
        ErrorCode, ErrorInfo, Response, ResponseHeader, ServerFrame, ServerHello, StreamReset,
        StreamResetReason, client_frame, server_frame,
    },
};
use aethercore_operation_kernel::{RequestContext, SubscriptionItem};
use chrono::Utc;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{router::ServiceContext, streaming};

const MAX_SESSIONS: usize = 32;
const MAX_SESSIONS_PER_USER_SID: usize = 4;
const MAX_INFLIGHT_PER_SESSION: usize = 8;
const MAX_GLOBAL_INFLIGHT_REQUESTS: usize = 64;
const IPC_BOOTSTRAP_ENQUEUE_TIMEOUT: Duration = Duration::from_secs(2);
static ACTIVE_SESSIONS: AtomicUsize = AtomicUsize::new(0);
static ACTIVE_REQUESTS: AtomicUsize = AtomicUsize::new(0);
static USER_SESSIONS: OnceLock<Mutex<HashMap<String, usize>>> = OnceLock::new();

struct SessionGuard;
impl Drop for SessionGuard {
    fn drop(&mut self) {
        ACTIVE_SESSIONS.fetch_sub(1, Ordering::SeqCst);
    }
}

struct UserSessionGuard {
    user_sid: String,
}

fn user_sessions() -> &'static Mutex<HashMap<String, usize>> {
    USER_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn admit_user_session(user_sid: &str) -> Option<UserSessionGuard> {
    if user_sid.trim().is_empty() {
        return None;
    }
    let mut sessions = user_sessions().lock().unwrap_or_else(|p| p.into_inner());
    let count = sessions.entry(user_sid.to_owned()).or_default();
    if *count >= MAX_SESSIONS_PER_USER_SID {
        return None;
    }
    *count = count.saturating_add(1);
    Some(UserSessionGuard {
        user_sid: user_sid.to_owned(),
    })
}

impl Drop for UserSessionGuard {
    fn drop(&mut self) {
        let mut sessions = user_sessions().lock().unwrap_or_else(|p| p.into_inner());
        let remove = if let Some(count) = sessions.get_mut(&self.user_sid) {
            *count = count.saturating_sub(1);
            *count == 0
        } else {
            false
        };
        if remove {
            sessions.remove(&self.user_sid);
        }
    }
}

fn try_acquire_slot(counter: &AtomicUsize, limit: usize) -> bool {
    let mut current = counter.load(Ordering::Acquire);
    loop {
        if current >= limit {
            return false;
        }
        match counter.compare_exchange_weak(
            current,
            current + 1,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => return true,
            Err(observed) => current = observed,
        }
    }
}

struct GlobalRequestGuard;
impl GlobalRequestGuard {
    fn try_acquire() -> Option<Self> {
        try_acquire_slot(&ACTIVE_REQUESTS, MAX_GLOBAL_INFLIGHT_REQUESTS).then_some(Self)
    }
}
impl Drop for GlobalRequestGuard {
    fn drop(&mut self) {
        ACTIVE_REQUESTS.fetch_sub(1, Ordering::AcqRel);
    }
}

struct InflightGuard {
    count: Arc<AtomicUsize>,
    request_ids: Arc<Mutex<HashSet<String>>>,
    request_id: String,
}
impl Drop for InflightGuard {
    fn drop(&mut self) {
        self.count.fetch_sub(1, Ordering::SeqCst);
        self.request_ids
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&self.request_id);
    }
}

pub fn run(stop: Arc<AtomicBool>, context: ServiceContext) -> anyhow::Result<()> {
    let pipe_name = aethercore_ipc::configured_pipe_name()?;
    info!(pipe = %pipe_name, "operation kernel IPC v7 ready");
    // Claim the fixed namespace once, then maintain continuous ownership: after every successful
    // accept, create the successor listener synchronously *before* the accepted session can be
    // dropped or moved to a worker. This removes a post-disconnect gap where another local process
    // could otherwise become first creator between sequential listener instances.
    let mut listener = aethercore_ipc::PipeServerListener::claim_first()
        .map_err(|error| anyhow::anyhow!("claim IPC pipe namespace: {error}"))?;
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok(session) => {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                // Keep the accepted service-owned instance alive until its successor exists. If
                // successor creation fails, stop fail-closed rather than release and later join a
                // potentially foreign namespace.
                listener = aethercore_ipc::PipeServerListener::create_successor()
                    .map_err(|error| anyhow::anyhow!("create successor IPC listener: {error}"))?;
                let previous = ACTIVE_SESSIONS.fetch_add(1, Ordering::SeqCst);
                if previous >= MAX_SESSIONS {
                    ACTIVE_SESSIONS.fetch_sub(1, Ordering::SeqCst);
                    warn!("persistent IPC session cap reached; dropping client");
                    continue;
                }
                let ctx = context.clone();
                if let Err(error) = thread::Builder::new()
                    .name("aether-ipc-session".into())
                    .spawn(move || {
                        let _guard = SessionGuard;
                        if let Err(error) = serve_session(session, ctx) {
                            warn!(error = %error, "IPC session ended");
                        }
                    })
                {
                    // Admission happened before thread creation. If the OS refuses a new thread,
                    // restore the global session counter and drop the accepted pipe fail-closed.
                    // The successor listener already exists, so namespace ownership is preserved.
                    ACTIVE_SESSIONS.fetch_sub(1, Ordering::SeqCst);
                    warn!(error = %error, "failed to create IPC session worker");
                }
            }
            Err(error) => {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                error!(error = %error, "named pipe accept failed");
                // The listener was consumed by the failed accept. Reclaim with FIRST_INSTANCE so
                // recovery never joins a namespace created by another process while this service
                // had no live pipe object.
                thread::sleep(Duration::from_millis(200));
                listener =
                    aethercore_ipc::PipeServerListener::claim_first().map_err(|claim_error| {
                        anyhow::anyhow!(
                            "reclaim IPC pipe namespace after accept failure: {claim_error}"
                        )
                    })?;
            }
        }
    }
    info!("maintenance service stopped");
    Ok(())
}

fn serve_session(
    mut session: aethercore_ipc::PipeServerSession,
    context: ServiceContext,
) -> anyhow::Result<()> {
    // Principal identity is captured once from the exact connected pipe token and is immutable for
    // the lifetime of this session. Every RPC and every event subscription uses this owner key.
    let peer = aethercore_security::inspect_named_pipe_client(session.raw_handle())
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    // The global cap protects service memory/thread count; the SID-local cap prevents one local
    // account (including repeated logon sessions) from monopolizing every persistent pipe slot.
    // The elevated consent broker shares the same user SID, so the quota intentionally leaves
    // room for the desktop, broker and short-lived recovery/reconnect overlap.
    let _user_session_guard = admit_user_session(&peer.user_sid)
        .ok_or_else(|| anyhow::anyhow!("per-user persistent IPC session cap reached"))?;
    let owner = peer.binding_key();

    let first = session
        .read()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let hello = match first.payload {
        Some(client_frame::Payload::Hello(value)) => value,
        _ => anyhow::bail!("client hello required"),
    };
    let session_id = Uuid::new_v4().to_string();
    let writer = session.writer();

    // Subscription installation and replay capture are one EventBus critical section. There is no
    // replay->live race: an event is either in ReplayBatch or queued to this subscription.
    let (subscription, replay) = context
        .kernel
        .events()
        .subscribe(&owner, hello.replay_after_sequence);

    writer
        .write_bootstrap(
            &ServerFrame {
                payload: Some(server_frame::Payload::Hello(ServerHello {
                    protocol_version: PROTOCOL_VERSION,
                    session_id: session_id.clone(),
                    service_version: env!("CARGO_PKG_VERSION").into(),
                    server_time_unix_ms: Utc::now().timestamp_millis(),
                    current_sequence: replay.current_sequence,
                    max_inflight_requests: MAX_INFLIGHT_PER_SESSION as u32,
                    replay_floor_sequence: replay.replay_floor_sequence,
                    replay_complete: replay.complete,
                })),
            },
            IPC_BOOTSTRAP_ENQUEUE_TIMEOUT,
        )
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;

    if hello.protocol_version != PROTOCOL_VERSION {
        return Ok(());
    }

    if replay.complete {
        for event in replay.events {
            writer
                .write_bootstrap(
                    &ServerFrame {
                        payload: Some(server_frame::Payload::Event(event)),
                    },
                    IPC_BOOTSTRAP_ENQUEUE_TIMEOUT,
                )
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        }
    } else {
        let reason = if hello.replay_after_sequence > replay.current_sequence {
            StreamResetReason::SequenceReset
        } else {
            StreamResetReason::ReplayWindowExceeded
        };
        writer
            .write_bootstrap(
                &ServerFrame {
                    payload: Some(server_frame::Payload::StreamReset(StreamReset {
                        reason: reason as i32,
                        current_sequence: replay.current_sequence,
                        replay_floor_sequence: replay.replay_floor_sequence,
                        message_key: "ipc.streamReset.hydrationRequired".into(),
                    })),
                },
                IPC_BOOTSTRAP_ENQUEUE_TIMEOUT,
            )
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    }

    let alive = Arc::new(AtomicBool::new(true));
    let pump_alive = alive.clone();
    let pump_writer = writer.clone();
    let pump_context = context.clone();
    let pump_owner = owner.clone();
    thread::Builder::new()
        .name("aether-ipc-event-pump".into())
        .spawn(move || {
            while pump_alive.load(Ordering::Acquire) {
                match subscription.recv_timeout(Duration::from_millis(500)) {
                    SubscriptionItem::Event(event) => {
                        if pump_writer
                            .write(&ServerFrame {
                                payload: Some(server_frame::Payload::Event(*event)),
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    SubscriptionItem::Lagged => {
                        // Queue overflow is never silent. Drop stale queued telemetry, reset the
                        // consumer sequence to the current bus edge, and republish a complete typed
                        // current-state image before continuing live delivery.
                        let current = pump_context.kernel.events().current_sequence(&pump_owner);
                        let floor = pump_context
                            .kernel
                            .events()
                            .replay_floor_sequence(&pump_owner);
                        if pump_writer
                            .write(&ServerFrame {
                                payload: Some(server_frame::Payload::StreamReset(StreamReset {
                                    reason: StreamResetReason::SubscriberLagged as i32,
                                    current_sequence: current,
                                    replay_floor_sequence: floor,
                                    message_key: "ipc.streamReset.subscriberLagged".into(),
                                })),
                            })
                            .is_err()
                        {
                            break;
                        }
                        streaming::publish_hydration(&pump_context, &pump_owner);
                    }
                    SubscriptionItem::Timeout => {}
                    SubscriptionItem::Disconnected => break,
                }
            }
        })
        .map_err(|error| anyhow::anyhow!("failed to create IPC event pump: {error}"))?;

    // A normal successful reconnect consumes only replay deltas. Hydration is explicit through
    // HydrateSession (renderer/bootstrap) and automatic only after a replay/reset failure. This
    // prevents connection establishment itself from manufacturing principal-stream events.
    if !replay.complete {
        streaming::publish_hydration(&context, &owner);
    }

    let inflight = Arc::new(AtomicUsize::new(0));
    let active_request_ids = Arc::new(Mutex::new(HashSet::<String>::new()));
    while let Ok(frame) = session.read() {
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
                let request_id = req
                    .header
                    .as_ref()
                    .map(|header| header.request_id.clone())
                    .unwrap_or_default();
                let prior = inflight.fetch_add(1, Ordering::SeqCst);
                if prior >= MAX_INFLIGHT_PER_SESSION {
                    inflight.fetch_sub(1, Ordering::SeqCst);
                    let _ = writer.write(&ServerFrame {
                        payload: Some(server_frame::Payload::Response(error_response(
                            &request_id,
                            429,
                            ErrorCode::Busy,
                            "session request concurrency limit reached",
                        ))),
                    });
                    continue;
                }
                {
                    let mut ids = active_request_ids.lock().unwrap_or_else(|p| p.into_inner());
                    if !ids.insert(request_id.clone()) {
                        inflight.fetch_sub(1, Ordering::SeqCst);
                        let _ = writer.write(&ServerFrame {
                            payload: Some(server_frame::Payload::Response(error_response(
                                &request_id,
                                409,
                                ErrorCode::Conflict,
                                "duplicate active request id in session",
                            ))),
                        });
                        continue;
                    }
                }
                let request_guard = InflightGuard {
                    count: inflight.clone(),
                    request_ids: active_request_ids.clone(),
                    request_id: request_id.clone(),
                };

                let now = Utc::now().timestamp_millis();
                let deadline = if envelope.deadline_unix_ms <= 0 {
                    now.saturating_add(DEFAULT_REQUEST_DEADLINE_MS)
                } else {
                    envelope.deadline_unix_ms
                };
                if deadline <= now || deadline > now.saturating_add(MAX_REQUEST_DEADLINE_MS) {
                    let _ = writer.write(&ServerFrame {
                        payload: Some(server_frame::Payload::Response(error_response(
                            &request_id,
                            408,
                            ErrorCode::DeadlineExceeded,
                            "invalid or expired request deadline",
                        ))),
                    });
                    continue;
                }

                let cancellation_id = if envelope.cancellation_id.trim().is_empty() {
                    format!("cancel:{request_id}")
                } else {
                    envelope.cancellation_id
                };
                if cancellation_id.len() > 256 {
                    let _ = writer.write(&ServerFrame {
                        payload: Some(server_frame::Payload::Response(error_response(
                            &request_id,
                            400,
                            ErrorCode::InvalidRequest,
                            "invalid cancellation id",
                        ))),
                    });
                    continue;
                }

                // Per-session limits are insufficient when a disconnected session leaves a native
                // call that cannot reach a cooperative cancellation checkpoint. A service-global
                // worker budget bounds that failure mode across reconnect churn and fails closed.
                let Some(global_request_guard) = GlobalRequestGuard::try_acquire() else {
                    let _ = writer.write(&ServerFrame {
                        payload: Some(server_frame::Payload::Response(error_response(
                            &request_id,
                            503,
                            ErrorCode::Busy,
                            "service request worker limit reached",
                        ))),
                    });
                    continue;
                };

                let scoped = format!("{session_id}:{cancellation_id}");
                let token = match context.kernel.cancellations().try_register(&scoped) {
                    Ok(token) => token,
                    Err(error) => {
                        let _ = writer.write(&ServerFrame {
                            payload: Some(server_frame::Payload::Response(error_response(
                                &request_id,
                                409,
                                ErrorCode::Conflict,
                                &error.to_string(),
                            ))),
                        });
                        continue;
                    }
                };
                let req_ctx = RequestContext {
                    session_id: session_id.clone(),
                    request_id: request_id.clone(),
                    owner_principal_key: owner.clone(),
                    deadline_unix_ms: deadline,
                    cancellation: token,
                };
                let ctx = context.clone();
                let peer = peer.clone();
                let worker_writer = writer.clone();
                let cancels = context.kernel.cancellations().clone();
                let worker_cancels = cancels.clone();
                let worker_scoped = scoped.clone();
                let spawn = thread::Builder::new()
                    .name("aether-ipc-request".into())
                    .spawn(move || {
                        let _request_guard = request_guard;
                        let _global_request_guard = global_request_guard;
                        let response = ctx.handle(&peer, &req_ctx, req);
                        worker_cancels.remove(&worker_scoped);
                        let _ = worker_writer.write(&ServerFrame {
                            payload: Some(server_frame::Payload::Response(response)),
                        });
                    });
                if let Err(error) = spawn {
                    // The closure (and both RAII admission guards) is dropped on spawn failure, but
                    // the cancellation registry entry was installed before thread creation and must
                    // be removed explicitly so a future request cannot collide with a ghost token.
                    cancels.remove(&scoped);
                    warn!(error = %error, request_id = %request_id, "failed to create request worker");
                    let _ = writer.write(&ServerFrame {
                        payload: Some(server_frame::Payload::Response(error_response(
                            &request_id,
                            503,
                            ErrorCode::Busy,
                            "service request worker unavailable",
                        ))),
                    });
                }
            }
            None => {}
        }
    }
    alive.store(false, Ordering::Release);
    // A disconnected client cannot continue to own cancellable request work. Durable mutations are
    // not blindly aborted here; only request-scoped cancellation tokens are signalled.
    context.kernel.cancellations().cancel_session(&session_id);
    Ok(())
}

fn error_response(request_id: &str, status: u32, code: ErrorCode, detail: &str) -> Response {
    Response {
        header: Some(ResponseHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.into(),
        }),
        status_code: status,
        #[allow(deprecated)] // wire-contract placeholder field (proto field 3)
        error_message: detail.into(),
        error: Some(ErrorInfo {
            code: code as i32,
            domain: "ipc".into(),
            message_key: "ipc.requestRejected".into(),
            message_args: vec![],
            correlation_id: request_id.into(),
            technical_detail: detail.into(),
            retryable: matches!(code, ErrorCode::Busy | ErrorCode::DeadlineExceeded),
        }),
        payload: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_request_slot_admission_is_strictly_bounded() {
        let counter = AtomicUsize::new(0);
        for _ in 0..MAX_GLOBAL_INFLIGHT_REQUESTS {
            assert!(try_acquire_slot(&counter, MAX_GLOBAL_INFLIGHT_REQUESTS));
        }
        assert!(!try_acquire_slot(&counter, MAX_GLOBAL_INFLIGHT_REQUESTS));
        assert_eq!(
            counter.load(Ordering::Acquire),
            MAX_GLOBAL_INFLIGHT_REQUESTS
        );
    }
}
