#![forbid(unsafe_code)]

pub const PROTOCOL_VERSION: u32 = 7;

// Requests are intentionally tiny: every operation is a typed identifier/request rather than an
// arbitrary command or payload. Driver inventory responses can legitimately be much larger on
// systems with extensive PnP trees, so response capacity is bounded independently.
pub const MAX_REQUEST_ID_BYTES: usize = 128;
pub const MAX_REQUEST_FRAME_BYTES: usize = 256 * 1024;
pub const MAX_RESPONSE_FRAME_BYTES: usize = 8 * 1024 * 1024;
// Client session frames carry only bounded RPC/control envelopes; they do not need the multi-MiB
// allowance required for large device/diagnostic responses sent by the service.
pub const MAX_CLIENT_SESSION_FRAME_BYTES: usize = 384 * 1024;
pub const MAX_SERVER_SESSION_FRAME_BYTES: usize = 8 * 1024 * 1024;
pub const DEFAULT_REQUEST_DEADLINE_MS: i64 = 15_000;
pub const MAX_REQUEST_DEADLINE_MS: i64 = 120_000;

pub mod v1 {
    include!(concat!(env!("OUT_DIR"), "/aethercore.v1.rs"));
}
