$ErrorActionPreference = 'Continue'
Write-Output '=== E: raw protocol repro - connect, hello, send GetPerformanceSnapshot request, wait 12s ==='
# The probe speaks one full round trip using the SAME client stack the CLI uses.
$code = @'
fn main() {
    use aethercore_contracts::v1::{request, ClientFrame, client_frame, ServerFrame};
    let noop_event: std::sync::Arc<dyn Fn(aethercore_contracts::v1::EventEnvelope) + Send + Sync> = std::sync::Arc::new(|_| {});
    let noop_reset: std::sync::Arc<dyn Fn(aethercore_contracts::v1::StreamReset) + Send + Sync> = std::sync::Arc::new(|_| {});
    let noop_disconnect: std::sync::Arc<dyn Fn() + Send + Sync> = std::sync::Arc::new(|| {});
    let t0 = std::time::Instant::now();
    let client = match aethercore_ipc::SessionClient::connect("p36-probe", env!("CARGO_PKG_VERSION"), 0,
        std::sync::Arc::new(|_: aethercore_contracts::v1::EventEnvelope| {}),
        std::sync::Arc::new(|_: aethercore_contracts::v1::StreamReset| {}),
        std::sync::Arc::new(|| {})) {
        Ok(c) => { println!("step1_connect=OK dur_ms={}", t0.elapsed().as_millis()); c },
        Err(e) => { println!("step1_connect=FAIL {e}"); return; }
    };
    let t0 = std::time::Instant::now();
    let req = aethercore_contracts::v1::Request {
        header: Some(aethercore_contracts::v1::RequestHeader {
            protocol_version: aethercore_contracts::PROTOCOL_VERSION,
            request_id: format!("p36-{}", std::process::id()),
        }),
        payload: Some(aethercore_contracts::v1::request::Payload::GetPerformanceSnapshot(
            aethercore_contracts::v1::GetPerformanceSnapshotRequest {},
        )),
    };
    match client.request(req, std::time::Duration::from_secs(12)) {
        Ok(resp) => println!("step2_request=OK status={} dur_ms={}", resp.status_code, t0.elapsed().as_millis()),
        Err(e) => println!("step2_request=FAIL {} dur_ms={}", e, t0.elapsed().as_millis()),
    }
}
'@
