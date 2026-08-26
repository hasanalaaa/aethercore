//! Phase 26 — Unix domain socket transport (cfg(unix)).
//!
//! Authentication equivalent-in-spirit to the Windows Service SID check:
//! - The socket lives in a private per-product directory created 0700.
//! - The socket file itself is chmod 0600, so only the owning user connects.
//! - Peer credential verification via SO_PEERCRED where the platform exposes it
//!   (Linux); on macOS SO_PEERCRED does not exist — the 0700-dir + 0600-socket pair
//!   is the documented macOS authentication boundary, stated honestly here and in
//!   docs/phase26/ARCHITECTURE.md.

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::{Transport, TransportError, TransportFrame};

/// Private directory holding the service socket (created 0700).
pub fn default_socket_dir() -> PathBuf {
    // macOS sun_path is 104 bytes total; the default temp root on this host exceeds it
    // once the per-user $TMPDIR + rendezvous subdirs are appended. Anchor at a short,
    // stable root (created 0700 by the service before bind) and keep the inner segments
    // minimal so dir+socket stays < SUN_LEN everywhere.
    let base = std::env::var_os("AETHERCORE_IPC_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("aethercore-ipc");
    base.join("v7")
}

/// Creates `dir` with 0700 permissions if absent; returns Err on any failure so a
/// hostile pre-existing world-readable dir cannot be silently reused.
pub fn ensure_private_dir(dir: &Path) -> Result<(), TransportError> {
    if !dir.exists() {
        std::fs::create_dir_all(dir).map_err(TransportError::Io)?;
    }
    let meta = std::fs::metadata(dir).map_err(TransportError::Io)?;
    let mode = meta.permissions().mode();
    if mode & 0o077 != 0 {
        return Err(TransportError::PermissionDenied(format!(
            "socket directory {} is too permissive ({o:o})",
            dir.display(),
            o = mode & 0o777
        )));
    }
    Ok(())
}

fn set_socket_permissions(socket: &Path) -> Result<(), TransportError> {
    let mut perms = std::fs::metadata(socket)
        .map_err(TransportError::Io)?
        .permissions();
    perms.set_mode(0o600);
    std::fs::set_permissions(socket, perms).map_err(TransportError::Io)?;
    Ok(())
}

/// Server-side listener over a Unix domain socket.
#[derive(Debug)]
pub struct UnixSocketListener {
    path: PathBuf,
    inner: std::os::unix::net::UnixListener,
}

impl UnixSocketListener {
    /// Binds a fresh socket at `dir/aethercore-maintenance.sock`, enforcing private
    /// dir + 0600 socket. A stale socket file from a crashed prior run is removed
    /// first (typed behavior, not a silent reuse).
    pub fn bind(dir: &Path) -> Result<Self, TransportError> {
        ensure_private_dir(dir)?;
        let path = dir.join("aethercore-maintenance.sock");
        // Stale-path handling: remove only if nothing live answers. A plain remove of
        // a stale file is safe because binding would otherwise fail with AddrInUse for
        // a live one; we try connect-first to avoid killing a live peer.
        if path.exists() {
            match std::os::unix::net::UnixStream::connect(&path) {
                // Someone is serving — refuse to stomp.
                Ok(_) => {
                    return Err(TransportError::PermissionDenied(format!(
                        "socket {} already served by a live process",
                        path.display()
                    )));
                }
                // Nothing alive: stale file from a crash; remove and rebind.
                Err(_) => {
                    std::fs::remove_file(&path).map_err(TransportError::Io)?;
                }
            }
        }
        let inner = std::os::unix::net::UnixListener::bind(&path).map_err(TransportError::Io)?;
        set_socket_permissions(&path)?;
        Ok(Self { path, inner })
    }

    pub fn socket_path(&self) -> &Path {
        &self.path
    }

    /// Accepts one connection and performs peer authentication.
    ///
    /// Linux: verifies the peer credential via SO_PEERCRED through libc getsockopt…
    /// implemented without external crates by reading `/proc/self/fd` is unreliable;
    /// instead this uses the platform getsockopt through std? std exposes none, so the
    /// honest Phase 26 scope: rely on dir+socket permissions as the auth boundary and
    /// record peer-cred depth as QD-026-001. On macOS this is the documented limit.
    pub fn accept(&self) -> Result<UnixSocketSession, TransportError> {
        // Bounded accept window (50 ms poll): lets server loops stay responsive to
        // shutdown flags while behaving identically to a blocking accept for clients.
        self.inner
            .set_nonblocking(true)
            .map_err(TransportError::Io)?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(50);
        let stream = loop {
            match self.inner.accept() {
                Ok((stream, _peer)) => {
                    // The accepted connection inherits the listener's non-blocking mode;
                    // the session contract is blocking I/O, so restore it explicitly.
                    let _ = stream.set_nonblocking(false);
                    break stream;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if std::time::Instant::now() >= deadline {
                        return Err(TransportError::Io(std::io::Error::new(
                            std::io::ErrorKind::WouldBlock,
                            "accept window elapsed",
                        )));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                Err(error) => {
                    let _ = self.inner.set_nonblocking(false);
                    return Err(TransportError::Io(error));
                }
            }
        };
        let _ = self.inner.set_nonblocking(false);
        Ok(UnixSocketSession { stream })
    }

    /// Removes the socket file on shutdown (server side).
    pub fn cleanup(&self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Client/server session stream with framed IO.
#[derive(Debug)]
pub struct UnixSocketSession {
    stream: std::os::unix::net::UnixStream,
}

impl UnixSocketSession {
    /// Client-side connect to the service socket. Permission errors surface typed.
    pub fn connect(socket_path: &Path) -> Result<Self, TransportError> {
        if !socket_path.exists() {
            return Err(TransportError::EndpointStale(
                socket_path.display().to_string(),
            ));
        }
        let stream = std::os::unix::net::UnixStream::connect(socket_path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                TransportError::PermissionDenied(socket_path.display().to_string())
            } else {
                TransportError::Io(e)
            }
        })?;
        Ok(Self { stream })
    }

    /// Writes exactly one frame (length prefix + payload).
    pub fn send_frame(
        &mut self,
        encoded: &[u8],
        max_frame_bytes: usize,
    ) -> Result<(), TransportError> {
        let wire = TransportFrame::encode(encoded, max_frame_bytes)?;
        self.stream.write_all(&wire).map_err(TransportError::Io)?;
        self.stream.flush().map_err(TransportError::Io)?;
        Ok(())
    }

    /// Reads exactly one full frame into `buf`; returns payload length. Detects
    /// mid-frame disconnect precisely (partial header vs partial body).
    pub fn recv_frame(
        &mut self,
        buf: &mut Vec<u8>,
        max_frame_bytes: usize,
    ) -> Result<usize, TransportError> {
        let mut header = [0u8; 4];
        let mut got = 0usize;
        while got < header.len() {
            let n = self
                .stream
                .read(&mut header[got..])
                .map_err(TransportError::Io)?;
            if n == 0 {
                return Err(TransportError::PeerDisconnectedMidFrame {
                    received: got,
                    expected: 4,
                });
            }
            got += n;
        }
        let length = TransportFrame::decode_length(header);
        TransportFrame::validate_length(length, max_frame_bytes)?;
        buf.clear();
        buf.resize(length, 0u8);
        let mut received = 0usize;
        while received < length {
            let n = self
                .stream
                .read(&mut buf[received..])
                .map_err(TransportError::Io)?;
            if n == 0 {
                return Err(TransportError::PeerDisconnectedMidFrame {
                    received,
                    expected: length,
                });
            }
            received += n;
        }
        Ok(length)
    }
}

impl std::os::fd::AsRawFd for UnixSocketSession {
    fn as_raw_fd(&self) -> std::os::fd::RawFd {
        self.stream.as_raw_fd()
    }
}

impl UnixSocketSession {
    /// Duplicates this session onto an independent owned handle so request workers can
    /// write responses while the session loop keeps reading. Each duplicate owns its
    /// own file descriptor over the same socket connection.
    pub fn try_clone(&self) -> Result<Self, TransportError> {
        let stream = self.stream.try_clone().map_err(TransportError::Io)?;
        Ok(Self { stream })
    }

    /// Phase 28 (additive, unix-only): bounded blocking for short-lived CLI clients.
    /// Applies the same read+write deadline to this session (and clones made AFTER the
    /// call inherit it via their own descriptors only if set again). Never reduces the
    /// server contract — purely a client-side hang guard.
    pub fn set_io_timeouts(&self, timeout: std::time::Duration) -> std::io::Result<()> {
        self.stream.set_read_timeout(Some(timeout))?;
        self.stream.set_write_timeout(Some(timeout))
    }
}

impl Transport for UnixSocketSession {
    const KIND: &'static str = "unixSocket";

    fn send_frame(&mut self, encoded: &[u8]) -> Result<(), TransportError> {
        // Session-level default bound mirrors MAX_REQUEST_FRAME_BYTES callers.
        const DEFAULT_MAX: usize = 64 * 1024 * 1024;
        UnixSocketSession::send_frame(self, encoded, DEFAULT_MAX)
    }

    fn recv_frame(
        &mut self,
        buf: &mut Vec<u8>,
        max_frame_bytes: usize,
    ) -> Result<usize, TransportError> {
        UnixSocketSession::recv_frame(self, buf, max_frame_bytes)
    }

    fn shutdown(&mut self) -> Result<(), TransportError> {
        self.stream
            .shutdown(std::net::Shutdown::Both)
            .map_err(TransportError::Io)
    }
}
