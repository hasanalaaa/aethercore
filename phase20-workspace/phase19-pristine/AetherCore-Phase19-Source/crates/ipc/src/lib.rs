use aethercore_contracts::{
    MAX_CLIENT_SESSION_FRAME_BYTES, MAX_REQUEST_FRAME_BYTES, MAX_RESPONSE_FRAME_BYTES,
    MAX_SERVER_SESSION_FRAME_BYTES,
    v1::{ClientFrame, Request, Response, ServerFrame},
};
use prost::Message;
use thiserror::Error;

pub const PIPE_NAME: &str = r"\\.\pipe\AetherCore.Maintenance.v7";
const DEV_PIPE_TOKEN_ARG: &str = "--dev-pipe-token";
const DEV_PIPE_TOKEN_ENV: &str = "AETHERCORE_DEV_PIPE_TOKEN";

#[cfg(debug_assertions)]
fn pipe_name_with_dev_token(token: &str) -> Result<String> {
    if token.len() != 32 || !token.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
        return Err(IpcError::Protocol("invalid development pipe token".into()));
    }
    Ok(format!("{PIPE_NAME}.{token}"))
}

/// Resolve the single IPC rendezvous name for this process.
///
/// Release builds always use the fixed product pipe. Debug builds may opt into an isolated,
/// per-launch pipe by receiving a 128-bit lowercase-hex token from `run-dev.ps1`. The elevated
/// console service receives the token as an explicit command-line argument; the non-elevated
/// desktop inherits the same token through its environment. This development-only rendezvous is
/// compiled out of release behavior and avoids weakening production endpoint authentication.
pub fn configured_pipe_name() -> Result<String> {
    #[cfg(debug_assertions)]
    {
        let env_token = std::env::var(DEV_PIPE_TOKEN_ENV).ok();
        let arg_token = if env_token.is_none() {
            let mut args = std::env::args();
            let mut found = None;
            while let Some(arg) = args.next() {
                if arg == DEV_PIPE_TOKEN_ARG {
                    found = args.next();
                    break;
                }
            }
            found
        } else {
            None
        };
        if let Some(token) = env_token.or(arg_token) {
            return pipe_name_with_dev_token(&token);
        }
    }
    Ok(PIPE_NAME.to_owned())
}

#[derive(Debug, Error)]
pub enum IpcError {
    #[error("unsupported platform")]
    Unsupported,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol encode: {0}")]
    Encode(#[from] prost::EncodeError),
    #[error("protocol decode: {0}")]
    Decode(#[from] prost::DecodeError),
    #[error("frame too large: {actual} bytes (limit {limit})")]
    FrameTooLarge { actual: usize, limit: usize },
    #[error("trailing bytes after framed message: {0}")]
    TrailingBytes(usize),
    #[error("protocol violation: {0}")]
    Protocol(String),
    #[error("request deadline exceeded")]
    DeadlineExceeded,
    #[error("session disconnected")]
    Disconnected,
    #[error("session outbound backpressure limit reached")]
    OutboundBackpressure,
    #[error("untrusted named-pipe server: {0}")]
    UntrustedPeer(String),
    #[error("windows api: {0}")]
    Windows(String),
}

pub type Result<T> = std::result::Result<T, IpcError>;

fn write_message_with_limit<W: std::io::Write, M: Message>(writer: &mut W, message: &M, limit: usize) -> Result<()> {
    let bytes = message.encode_to_vec();
    if bytes.len() > limit { return Err(IpcError::FrameTooLarge { actual: bytes.len(), limit }); }
    writer.write_all(&(bytes.len() as u32).to_le_bytes())?;
    writer.write_all(&bytes)?;
    writer.flush()?;
    Ok(())
}

fn read_message_with_limit<R: std::io::Read, M: Message + Default>(reader: &mut R, limit: usize) -> Result<M> {
    let mut header = [0u8; 4];
    reader.read_exact(&mut header)?;
    let len = u32::from_le_bytes(header) as usize;
    if len == 0 || len > limit { return Err(IpcError::FrameTooLarge { actual: len, limit }); }
    let mut bytes = vec![0u8; len];
    reader.read_exact(&mut bytes)?;
    Ok(M::decode(bytes.as_slice())?)
}

// Legacy payload-frame helpers are retained as deterministic fuzz targets. Production v7 uses
// ClientFrame/ServerFrame session envelopes below.
pub(crate) fn write_request<W: std::io::Write>(writer: &mut W, request: &Request) -> Result<()> { write_message_with_limit(writer, request, MAX_REQUEST_FRAME_BYTES) }
pub(crate) fn read_request<R: std::io::Read>(reader: &mut R) -> Result<Request> { read_message_with_limit(reader, MAX_REQUEST_FRAME_BYTES) }
pub(crate) fn write_response<W: std::io::Write>(writer: &mut W, response: &Response) -> Result<()> { write_message_with_limit(writer, response, MAX_RESPONSE_FRAME_BYTES) }
pub(crate) fn read_response<R: std::io::Read>(reader: &mut R) -> Result<Response> { read_message_with_limit(reader, MAX_RESPONSE_FRAME_BYTES) }
pub(crate) fn write_client_frame<W: std::io::Write>(writer: &mut W, frame: &ClientFrame) -> Result<()> { write_message_with_limit(writer, frame, MAX_CLIENT_SESSION_FRAME_BYTES) }
pub(crate) fn read_client_frame<R: std::io::Read>(reader: &mut R) -> Result<ClientFrame> { read_message_with_limit(reader, MAX_CLIENT_SESSION_FRAME_BYTES) }
pub(crate) fn write_server_frame<W: std::io::Write>(writer: &mut W, frame: &ServerFrame) -> Result<()> { write_message_with_limit(writer, frame, MAX_SERVER_SESSION_FRAME_BYTES) }
pub(crate) fn read_server_frame<R: std::io::Read>(reader: &mut R) -> Result<ServerFrame> { read_message_with_limit(reader, MAX_SERVER_SESSION_FRAME_BYTES) }

pub fn decode_request_frame_bytes(frame: &[u8]) -> Result<Request> {
    let mut cursor = std::io::Cursor::new(frame);
    let request = read_request(&mut cursor)?;
    let consumed = cursor.position() as usize;
    if consumed != frame.len() { return Err(IpcError::TrailingBytes(frame.len() - consumed)); }
    Ok(request)
}

pub fn decode_client_frame_bytes(frame: &[u8]) -> Result<ClientFrame> {
    let mut cursor = std::io::Cursor::new(frame);
    let message = read_client_frame(&mut cursor)?;
    let consumed = cursor.position() as usize;
    if consumed != frame.len() { return Err(IpcError::TrailingBytes(frame.len() - consumed)); }
    Ok(message)
}

#[cfg(windows)] mod windows_impl;
#[cfg(windows)] pub use windows_impl::{PipeServerListener, PipeServerSession, PipeServerWriter, SessionClient, connect};

#[cfg(not(windows))]
pub fn connect(_: &Request) -> Result<Response> { Err(IpcError::Unsupported) }

#[cfg(test)]
mod tests {
    use super::*;
    use aethercore_contracts::{PROTOCOL_VERSION, v1::{RequestHeader, request}};

    #[cfg(debug_assertions)]
    #[test]
    fn development_pipe_token_is_strict_and_namespace_scoped() {
        let token = "0123456789abcdef0123456789abcdef";
        assert_eq!(pipe_name_with_dev_token(token).unwrap(), format!("{PIPE_NAME}.{token}"));
        for invalid in ["", "0123", "0123456789ABCDEF0123456789ABCDEF", "gggggggggggggggggggggggggggggggg", "0123456789abcdef0123456789abcde/"] {
            assert!(matches!(pipe_name_with_dev_token(invalid), Err(IpcError::Protocol(_))));
        }
    }

    fn ping() -> Request { Request { header: Some(RequestHeader { protocol_version: PROTOCOL_VERSION, request_id: "a1b2c3d4-test-request".into() }), payload: Some(request::Payload::Ping(aethercore_contracts::v1::PingRequest {})) } }

    #[test] fn request_frame_round_trips(){let input=ping();let mut bytes=Vec::new();write_request(&mut bytes,&input).unwrap();let mut slice=bytes.as_slice();let decoded=read_request(&mut slice).unwrap();assert_eq!(decoded.header.unwrap().request_id,"a1b2c3d4-test-request");}
    #[test] fn request_limit_is_smaller_than_response_limit(){assert!(MAX_REQUEST_FRAME_BYTES<MAX_RESPONSE_FRAME_BYTES);}
    #[test] fn session_direction_limits_match_trust_and_payload_shape(){assert!(MAX_CLIENT_SESSION_FRAME_BYTES>=MAX_REQUEST_FRAME_BYTES);assert!(MAX_CLIENT_SESSION_FRAME_BYTES<MAX_RESPONSE_FRAME_BYTES);assert!(MAX_SERVER_SESSION_FRAME_BYTES>=MAX_RESPONSE_FRAME_BYTES);}
    #[test] fn rejects_zero_and_oversized_request_before_allocation(){let zero_bytes=0u32.to_le_bytes();let mut zero=zero_bytes.as_slice();assert!(matches!(read_request(&mut zero),Err(IpcError::FrameTooLarge{actual:0,..})));let too_large=(MAX_REQUEST_FRAME_BYTES as u32+1).to_le_bytes();let mut input=too_large.as_slice();assert!(matches!(read_request(&mut input),Err(IpcError::FrameTooLarge{..})));}
    #[test] fn rejects_truncated_header_and_payload(){let mut header=[1u8,2,3].as_slice();assert!(matches!(read_request(&mut header),Err(IpcError::Io(_))));let mut payload=Vec::new();payload.extend_from_slice(&16u32.to_le_bytes());payload.extend_from_slice(&[0x08,0x01]);let mut payload=payload.as_slice();assert!(matches!(read_request(&mut payload),Err(IpcError::Io(_))));}
    #[test] fn rejects_invalid_protobuf_and_trailing_bytes(){let invalid=[1u8,0,0,0,0xff];assert!(matches!(decode_request_frame_bytes(&invalid),Err(IpcError::Decode(_))));let input=ping();let mut bytes=Vec::new();write_request(&mut bytes,&input).unwrap();bytes.push(0x42);assert!(matches!(decode_request_frame_bytes(&bytes),Err(IpcError::TrailingBytes(1))));}
    #[test] fn deterministic_malformed_frame_corpus_never_panics(){let mut state=0x9e37_79b9_7f4a_7c15u64;for len in 0..2048usize{let mut bytes=vec![0u8;len%257];for byte in &mut bytes{state^=state<<13;state^=state>>7;state^=state<<17;*byte=state as u8;}assert!(std::panic::catch_unwind(||decode_client_frame_bytes(&bytes)).is_ok());}}
}
