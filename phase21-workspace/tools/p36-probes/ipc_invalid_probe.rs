use aethercore_contracts::v1::{request, Request, RequestHeader};

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "valid".to_string());
    let (request_id, payload) = if mode == "invalid" {
        (
            " ".to_string(),
            request::Payload::GetCareStatus(aethercore_contracts::v1::GetCareStatusRequest {}),
        )
    } else {
        (
            format!("p36d-{}", std::process::id()),
            request::Payload::GetCareStatus(aethercore_contracts::v1::GetCareStatusRequest {}),
        )
    };
    let client = aethercore_ipc::SessionClient::connect(
        "p36-probe4",
        env!("CARGO_PKG_VERSION"),
        0,
        std::sync::Arc::new(|_: aethercore_contracts::v1::EventEnvelope| {}),
        std::sync::Arc::new(|_: aethercore_contracts::v1::StreamReset| {}),
        std::sync::Arc::new(|| {}),
    )
    .expect("connect");
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
        Ok(r) => println!(
            "REQUEST=OK status={} dur_ms={}",
            r.status_code,
            t.elapsed().as_millis()
        ),
        Err(e) => println!("REQUEST=FAIL {e} dur_ms={}", t.elapsed().as_millis()),
    }
    drop(client);
}
