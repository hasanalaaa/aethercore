$ErrorActionPreference = 'Continue'
# KEY HYPOTHESIS: the event pump write blocks the server writer thread, so the RESPONSE
# frame can never be written while a subscriber is attached. Test: does a request round-trip
# SUCCEED if the client uses the byte-level raw transport (no SessionClient event pump)?
# Simplest decisive test: temporarily detach the event pump by connecting a client that
# sends Goodbye IMMEDIATELY after connect, then check if a SECOND client can round-trip.
# Cheaper decisive test: connect SessionClient, DON'T send any request, wait 3s, drop,
# then try again - if server was stuck in bootstrap-write to the first client (which
# never reads events), the second connect/hello may also stall.
Write-Output '=== T1: hello-only client (no request), then probe again ==='
Write-Output '--- probe A: connect + hold 3s + drop ---'
$code = @'
fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    let noop: std::sync::Arc<dyn Fn(aethercore_contracts::v1::EventEnvelope) + Send + Sync> = std::sync::Arc::new(|_| {});
    let client = aethercore_ipc::SessionClient::connect(
        "p36-probe2", env!("CARGO_PKG_VERSION"), 0,
        std::sync::Arc::new(|_: aethercore_contracts::v1::EventEnvelope| {}),
        std::sync::Arc::new(|_: aethercore_contracts::v1::StreamReset| {}),
        std::sync::Arc::new(|| {})).expect("connect");
    if mode == "hold" {
        std::thread::sleep(std::time::Duration::from_secs(3));
        println!("held 3s");
    } else if mode == "request" {
        let req = aethercore_contracts::v1::Request {
            header: Some(aethercore_contracts::v1::RequestHeader {
                protocol_version: aethercore_contracts::PROTOCOL_VERSION,
                request_id: format!("p36b-{}", std::process::id()),
            }),
            payload: Some(aethercore_contracts::v1::request::Payload::GetPerformanceSnapshot(
                aethercore_contracts::v1::GetPerformanceSnapshotRequest {},
            )),
        };
        match client.request(req, std::time::Duration::from_secs(8)) {
            Ok(r) => println!("REQUEST=OK status={}", r.status_code),
            Err(e) => println!("REQUEST=FAIL {e}"),
        }
    }
    drop(client);
}
'@
