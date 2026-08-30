#![cfg(windows)]

//! One live named-pipe request/response round trip.
//!
//! The Windows IPC path had no integration coverage at all before Phase 36, which is how a fully
//! synchronous pipe survived this long: the handshake writes precede the session loop's first
//! read, so `service detect` passed while every request/response verb deadlocked. This test
//! reproduces that exact shape - the server is parked in a read while the response is written -
//! so it fails against a non-overlapped pipe and passes against an overlapped one.

use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use aethercore_contracts::{
    PROTOCOL_VERSION,
    v1::{
        Request, RequestHeader, Response, ResponseHeader, ServerFrame, ServerHello, client_frame,
        server_frame,
    },
};
use aethercore_ipc::{PipeServerListener, SessionClient};

const REQUEST_ID: &str = "p36-overlapped-roundtrip";
/// Generous beside the sub-millisecond real cost, short enough that a deadlock fails the run.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
/// Long enough for the session loop to be inside its next read before the response is queued,
/// which is the ordering that deadlocks a synchronous file object.
const RESPONSE_DELAY: Duration = Duration::from_millis(250);

#[test]
fn request_response_round_trip_completes_over_live_named_pipe() {
    // Debug builds resolve an isolated per-launch pipe from this token, so the test never touches
    // the product rendezvous name, the installed service, or its production security descriptor.
    unsafe {
        std::env::set_var(
            "AETHERCORE_DEV_PIPE_TOKEN",
            "0f36a1c2d4e5b6978a0b1c2d3e4f5061",
        );
    }

    // Created before the client thread starts, so the endpoint exists when the client connects.
    let listener = PipeServerListener::claim_first().expect("claim dev pipe namespace");

    // The client runs off-thread so `accept` below can complete. `PipeServerListener` owns a bare
    // HANDLE and is not `Send`, so it stays here; the accepted session is what crosses threads.
    let (done_tx, done_rx) = mpsc::channel();
    thread::spawn(move || {
        let outcome = (|| -> Result<Response, String> {
            let client = SessionClient::connect(
                "p36-overlapped-test",
                env!("CARGO_PKG_VERSION"),
                0,
                Arc::new(|_| {}),
                Arc::new(|_| {}),
                Arc::new(|| {}),
            )
            .map_err(|error| error.to_string())?;
            client
                .request(
                    Request {
                        header: Some(RequestHeader {
                            protocol_version: PROTOCOL_VERSION,
                            request_id: REQUEST_ID.to_owned(),
                        }),
                        ..Default::default()
                    },
                    REQUEST_TIMEOUT,
                )
                .map_err(|error| error.to_string())
        })();
        let _ = done_tx.send(outcome);
    });

    let mut session = listener.accept().expect("accept client");
    let writer = session.writer();

    // The handshake write precedes the first read and therefore succeeds even on a synchronous
    // pipe. That is precisely why `service detect` never caught this.
    let hello = session.read().expect("client hello");
    assert!(matches!(
        hello.payload,
        Some(client_frame::Payload::Hello(_))
    ));
    writer
        .write(&ServerFrame {
            payload: Some(server_frame::Payload::Hello(ServerHello {
                protocol_version: PROTOCOL_VERSION,
                max_inflight_requests: 4,
                ..Default::default()
            })),
        })
        .expect("server hello");

    let framed = session.read().expect("client request");
    let request_id = match framed.payload {
        Some(client_frame::Payload::Request(session_request)) => session_request
            .request
            .and_then(|request| request.header)
            .map(|header| header.request_id)
            .expect("request header"),
        other => panic!("expected a request frame, got {other:?}"),
    };

    // Queue the response only once the session is parked in the read below. On a synchronous file
    // object the write then serializes behind that outstanding read and never lands.
    let responder = writer.clone();
    thread::spawn(move || {
        thread::sleep(RESPONSE_DELAY);
        let _ = responder.write(&ServerFrame {
            payload: Some(server_frame::Payload::Response(Response {
                header: Some(ResponseHeader {
                    protocol_version: PROTOCOL_VERSION,
                    request_id,
                }),
                ..Default::default()
            })),
        });
    });

    // Parked here for the whole duration of the response write; returns once the client goes.
    let server = thread::spawn(move || {
        let _ = session.read();
    });

    // Bounded: a transport deadlock must fail the test rather than hang the run.
    let response = match done_rx.recv_timeout(REQUEST_TIMEOUT + Duration::from_secs(5)) {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => panic!("request/response round trip failed: {error}"),
        Err(_) => panic!(
            "round trip never returned: the response write serialized behind the server's outstanding read"
        ),
    };

    assert_eq!(
        response.header.expect("response header").request_id,
        REQUEST_ID,
        "response must correlate with the request that produced it"
    );

    let _ = server.join();
}
