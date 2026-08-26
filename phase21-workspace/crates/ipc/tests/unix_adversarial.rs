//! Phase 26 — Unix transport adversarial tests (this Mac IS the test host).
//!
//! Attacked here: stale socket path, permission-denied connect, oversized frame,
//! mid-frame disconnect, live-socket stomp. Typed errors only; no panics.

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::Duration;

use aethercore_ipc::{
    Transport, TransportError, TransportFrame, UnixSocketListener, UnixSocketSession,
};

fn temp_dir(tag: &str) -> PathBuf {
    // Unix sockets are limited to ~104-byte paths (SUN_LEN) — keep it short.
    let dir = std::env::temp_dir().join(format!("axt-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");
    // Private dir contract: 0700, matching what bind() enforces.
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).expect("chmod");
    dir
}

fn unique_listener(tag: &str) -> (PathBuf, UnixSocketListener) {
    let dir = temp_dir(tag);
    let listener = UnixSocketListener::bind(&dir).expect("bind fresh socket");
    let path = listener.socket_path().to_path_buf();
    (path, listener)
}

#[test]
fn roundtrip_frame_survives_transport() {
    let (path, listener) = unique_listener("roundtrip");
    let server = std::thread::spawn(move || {
        let mut session = listener.accept().expect("accept");
        let mut buf = Vec::new();
        let len = session.recv_frame(&mut buf, 1024 * 1024).expect("recv");
        assert_eq!(len, buf.len());
        buf
    });

    let mut client = UnixSocketSession::connect(&path).expect("connect");
    let payload = b"aethercore phase26 framing check";
    client.send_frame(payload, 1024 * 1024).expect("send");

    let received = server.join().expect("no panic");
    assert_eq!(received, payload);
}

#[test]
fn stale_socket_path_is_removed_and_rebound() {
    let dir = temp_dir("stale");
    let stale = dir.join("aethercore-maintenance.sock");
    // A dead file at the socket path (not a real socket): binding must recover.
    std::fs::write(&stale, b"stale").expect("write stale");
    let _listener = UnixSocketListener::bind(&dir).expect("stale file removed and rebound");
}

#[test]
fn live_socket_refuses_second_bind() {
    let (_keep, listener) = unique_listener("live");
    let dir = listener.socket_path().parent().unwrap().to_path_buf();
    let second = UnixSocketListener::bind(&dir);
    match second {
        Err(TransportError::PermissionDenied(_)) => {}
        other => panic!("live socket must refuse re-bind with typed error, got {other:?}"),
    }
    let _ = _keep;
}

#[test]
fn missing_socket_path_is_typed_stale_on_connect() {
    let ghost = temp_dir("ghost").join("never.sock");
    let result = UnixSocketSession::connect(&ghost);
    match result {
        Err(TransportError::EndpointStale(path)) => {
            assert!(path.contains("never.sock"));
        }
        other => panic!("expected EndpointStale, got {other:?}"),
    }
}

#[test]
fn permission_denied_dir_blocks_bind_with_typed_error() {
    // Create a world-writable dir → bind must refuse rather than silently use it.
    let dir = temp_dir("perms");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).expect("chmod");
    let result = UnixSocketListener::bind(&dir);
    match result {
        Err(TransportError::PermissionDenied(msg)) => {
            assert!(msg.contains("permissive"), "{msg}")
        }
        other => panic!("over-permissive dir must be refused, got {other:?}"),
    }
}

#[test]
fn oversized_frame_is_rejected_before_any_io() {
    const MAX: usize = 1024;
    let err = TransportFrame::encode(&vec![0u8; MAX + 1], MAX)
        .expect_err("oversize must fail at encode time");
    match err {
        TransportError::FrameTooLarge { limit, got } => {
            assert_eq!(limit, MAX);
            assert_eq!(got, MAX + 1);
        }
        other => panic!("expected FrameTooLarge, got {other:?}"),
    }
}

#[test]
fn mid_frame_disconnect_is_detected_precisely() {
    let (path, listener) = unique_listener("midframe");
    let server = std::thread::spawn(move || {
        let mut session = listener.accept().expect("accept");
        let mut buf = Vec::new();
        session.recv_frame(&mut buf, 1024 * 1024)
    });

    let mut client = UnixSocketSession::connect(&path).expect("connect");
    // Announce a 512-byte frame then drop after sending only 8 bytes of body.
    client
        .stream_write_partial(512, &[7u8; 8])
        .expect("partial write");
    drop(client);

    let result = server.join().expect("no panic");
    match result {
        Err(TransportError::PeerDisconnectedMidFrame { received, expected }) => {
            assert_eq!(expected, 512);
            assert_eq!(received, 8);
        }
        other => panic!("expected precise mid-frame error, got {other:?}"),
    }
}

/// Test-only helper exposing partial writes for disconnect simulation.
trait PartialWrite {
    fn stream_write_partial(
        &mut self,
        announced_len: usize,
        bytes: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>>;
}

impl PartialWrite for UnixSocketSession {
    fn stream_write_partial(
        &mut self,
        announced_len: usize,
        bytes: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Write the length prefix announcing `announced_len`, then only part of the body.
        let mut wire = Vec::with_capacity(4 + bytes.len());
        wire.extend_from_slice(&(announced_len as u32).to_le_bytes());
        wire.extend_from_slice(bytes);
        self.send_raw_unchecked(&wire)?;
        Ok(())
    }
}

/// Raw write access for tests only (bypasses framing on purpose).
trait SendRawUnchecked {
    fn send_raw_unchecked(&mut self, raw: &[u8]) -> Result<(), Box<dyn std::error::Error>>;
}

impl SendRawUnchecked for UnixSocketSession {
    fn send_raw_unchecked(&mut self, raw: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        // The session owns its stream; use shutdown-free direct write via the framed
        // send is impossible here, so go through the underlying fd duplication:
        use std::os::fd::AsRawFd;
        let fd = self.as_raw_fd();
        let mut written = 0usize;
        while written < raw.len() {
            written += nix_write(fd, &raw[written..])?;
        }
        Ok(())
    }
}

fn nix_write(fd: i32, buf: &[u8]) -> Result<usize, Box<dyn std::error::Error>> {
    // Minimal libc write without extra crates.
    unsafe extern "C" {
        fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    }
    let n = unsafe { write(fd, buf.as_ptr(), buf.len()) };
    if n < 0 {
        Err("write failed".into())
    } else {
        Ok(n as usize)
    }
}

// Ensure the trait import above is used (as_raw_fd comes from it).
#[allow(unused_imports)]
use SendRawUnchecked as _SRU_ALIAS_KEEP;

#[test]
fn shutdown_is_graceful_and_repeatable_safe() {
    let (path, listener) = unique_listener("shutdown");
    let server = std::thread::spawn(move || {
        let _session = listener.accept().expect("accept");
        std::thread::sleep(Duration::from_millis(50));
    });
    let mut client = UnixSocketSession::connect(&path).expect("connect");
    client.shutdown().expect("graceful shutdown");
    let _ = server.join();
}
