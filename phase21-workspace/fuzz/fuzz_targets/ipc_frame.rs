#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Keep the legacy RPC decoder covered for compatibility clients, but Phase 10's primary
    // transport attack surface is the persistent ClientFrame session envelope.
    let _ = aethercore_ipc::decode_request_frame_bytes(data);
    let _ = aethercore_ipc::decode_client_frame_bytes(data);
});
