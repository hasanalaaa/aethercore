use aethercore_contracts::v1::request;

fn main() {
    let verb = std::env::args().nth(1).unwrap_or_else(|| "perf".to_string());
    let payload = match verb.as_str() {
        "optstatus" => request::Payload::GetOptimizationStatus(
            aethercore_contracts::v1::GetOptimizationStatusRequest { plan_id: String::new() },
        ),
        "carestatus" => request::Payload::GetCareStatus(
            aethercore_contracts::v1::GetCareStatusRequest {},
        ),
        _ => request::Payload::GetPerformanceSnapshot(
            aethercore_contracts::v1::GetPerformanceSnapshotRequest {},
        ),
    };
    let client = aethercore_ipc::SessionClient::connect(
        "p36-probe3",
        env!("CARGO_PKG_VERSION"),
        0,
        std::sync::Arc::new(|_: aethercore_contracts::v1::EventEnvelope| {}),
        std::sync::Arc::new(|_: aethercore_contracts::v1::StreamReset| {}),
        std::sync::Arc::new(|| {}),
    )
    .expect("connect");
    println!("connect=OK");
    let t = std::time::Instant::now();
    let req = aethercore_contracts::v1::Request {
        header: Some(aethercore_contracts::v1::RequestHeader {
            protocol_version: aethercore_contracts::PROTOCOL_VERSION,
            request_id: format!("p36c-{}", std::process::id()),
        }),
        payload: Some(payload),
    };
    match client.request(req, std::time::Duration::from_secs(8)) {
        Ok(r) => println!(
            "REQUEST=OK status={} dur_ms={}",
            r.status_code,
            t.elapsed().as_millis()
        ),
        Err(e) => println!("REQUEST=FAIL {e} dur_ms={}", t.elapsed().as_millis()),
    }
    drop(client);
}
