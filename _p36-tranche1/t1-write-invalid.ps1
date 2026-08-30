$ErrorActionPreference = 'Continue'
# Test whether the WRITE side of the client works at all: the ServerHello DID arrive on
# connect, so the server wrote it. Test whether the request frame reaches the server by
# checking whether the server's "duplicate client hello ignored" style warnings appear when
# we send garbage. Instead: drive the SERVER worker state directly - if the worker thread
# got the request, a second ipc_request_probe with an INVALID request id should produce
# a fast 400 error response (proving responses flow). Test: send request with EMPTY
# request_id -> server must answer 400 invalid request id instantly.
# Modify verb probe: add "invalid" verb that sends empty request id.
$ws = 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery'
$src = @'
use aethercore_contracts::v1::{request, Request, RequestHeader};

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "valid".to_string());
    let (request_id, payload) = if mode == "invalid" {
        (" ".to_string(), request::Payload::GetCareStatus(aethercore_contracts::v1::GetCareStatusRequest {}))
    } else {
        (format!("p36d-{}", std::process::id()), request::Payload::GetCareStatus(aethercore_contracts::v1::GetCareStatusRequest {}))
    };
    let client = aethercore_ipc::SessionClient::connect(
        "p36-probe4", env!("CARGO_PKG_VERSION"), 0,
        std::sync::Arc::new(|_: aethercore_contracts::v1::EventEnvelope| {}),
        std::sync::Arc::new(|_: aethercore_contracts::v1::StreamReset| {}),
        std::sync::Arc::new(|| {})).expect("connect");
    println!("connect=OK");
    let t = std::time::Instant::now();
    let req = Request {
        header: Some(RequestHeader {
            protocol_version: aethercore_contracts::PROTOCOL_VERSION,
            request_id: request_id.clone(),
        }),
        payload: Some(payload),
    };
    match client.request(req, std::time::Duration::from_secs(6)) {
        Ok(r) => println!("REQUEST=OK status={} dur_ms={}", r.status_code, t.elapsed().as_millis()),
        Err(e) => println!("REQUEST=FAIL {e} dur_ms={}", t.elapsed().as_millis()),
    }
    drop(client);
}
'@
Set-Content "$ws\crates\ipc\examples\ipc_invalid_probe.rs" $src -Encoding utf8
Write-Output 'example written'
