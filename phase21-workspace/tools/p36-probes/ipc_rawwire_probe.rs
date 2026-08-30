use aethercore_contracts::v1::{
    ClientFrame, ClientHello, Request, RequestHeader, client_frame, request,
};

// Raw-wire bisection probe (no SessionClient threads):
//  1. open pipe, send Hello, read ServerHello   -> transport RX/TX baseline
//  2. send a SECOND Hello                       -> if the server READ it, it logs
//     "duplicate client hello ignored" (service log breadcrumb for server-read proof)
//  3. send a Request (GetCareStatus), read one response frame; the watcher thread
//     closes a cloned handle after 10s so the read cannot hang forever.
fn main() {
    use std::os::windows::fs::OpenOptionsExt;
    // 1. manual pipe open (mirrors open_pipe())
    let pipe_name = aethercore_ipc::configured_pipe_name().expect("pipe name");
    let mut options = std::fs::OpenOptions::new();
    options
        .access_mode(0x0012_0003)
        .share_mode(0)
        .security_qos_flags(0x0001_0000);
    let mut reader = options.open(&pipe_name).expect("pipe open");
    let mut writer = reader.try_clone().expect("clone");
    println!("step_open=OK");

    // 2. hello round trip on the raw handle
    let hello = ClientFrame {
        payload: Some(client_frame::Payload::Hello(ClientHello {
            protocol_version: aethercore_contracts::PROTOCOL_VERSION,
            client_name: "p36-rawwire".into(),
            client_version: env!("CARGO_PKG_VERSION").into(),
            replay_after_sequence: 0,
        })),
    };
    aethercore_ipc::probe::write_client_frame(&mut writer, &hello).expect("write hello");
    println!("step_hello_write=OK");
    let server_hello =
        aethercore_ipc::probe::read_server_frame(&mut reader).expect("read hello");
    match server_hello.payload {
        Some(aethercore_contracts::v1::server_frame::Payload::Hello(h)) => {
            println!("step_hello_read=OK session={}", h.session_id);
        }
        _ => {
            println!("step_hello_read=UNEXPECTED");
            return;
        }
    }

    if std::env::args().nth(1).as_deref() == Some("hello-only") {
        println!("step_done=hello-only");
        return;
    }

    // 3. duplicate hello breadcrumb (server read-side proof)
    aethercore_ipc::probe::write_client_frame(&mut writer, &hello).expect("write dup hello");
    println!("step_dup_hello_write=OK (watch service log for 'duplicate client hello ignored')");

    // 4. request write + bounded response read
    let req = ClientFrame {
        payload: Some(client_frame::Payload::Request(
            aethercore_contracts::v1::SessionRequest {
                request: Some(Request {
                    header: Some(RequestHeader {
                        protocol_version: aethercore_contracts::PROTOCOL_VERSION,
                        request_id: "p36raw-1".into(),
                    }),
                    payload: Some(request::Payload::GetCareStatus(
                        aethercore_contracts::v1::GetCareStatusRequest {},
                    )),
                }),
                deadline_unix_ms: 0,
                cancellation_id: String::new(),
            },
        )),
    };
    aethercore_ipc::probe::write_client_frame(&mut writer, &req).expect("write request");
    println!("step_request_write=OK");

    // bounded response wait: close the pipe from a helper thread after 10s
    let mut killer = reader.try_clone().expect("clone for killer");
    let killer_thread = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(10));
        drop(killer);
    });
    let t0 = std::time::Instant::now();
    match aethercore_ipc::probe::read_server_frame(&mut reader) {
        Ok(frame) => match frame.payload {
            Some(aethercore_contracts::v1::server_frame::Payload::Response(r)) => println!(
                "step_response=OK status={} dur_ms={}",
                r.status_code,
                t0.elapsed().as_millis()
            ),
            Some(_) => println!(
                "step_response=OTHER_FRAME dur_ms={}",
                t0.elapsed().as_millis()
            ),
            None => println!("step_response=EMPTY dur_ms={}", t0.elapsed().as_millis()),
        },
        Err(e) => println!("step_response=FAIL {e} dur_ms={}", t0.elapsed().as_millis()),
    }
    let _ = killer_thread.join();
    println!("step_done=all");
}
