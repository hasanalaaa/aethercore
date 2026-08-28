//! Phase 34 — SSH transport executor over the OS OpenSSH client.
//!
//! Runs ssh via `Command` (typed argv, no local shell), enforces an operation
//! timeout with cancellation, captures stdout/stderr with bounds, and maps
//! every ending to the honest typed result space. If ssh is missing:
//! `NotAvailable`. No password prompt is possible (BatchMode).

use crate::domain::FleetHost;
use crate::trust::{
    MAX_CAPTURE_BYTES, RemoteOutcomeKind, RemoteOutput, RemoteResult, TrustDecision, TrustStore,
    build_remote_argv, classify_ssh_exit, ssh_binary,
};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Cancellation flag shared between the orchestrator and in-flight transports.
#[derive(Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// One typed, bounded remote execution.
pub struct SshTransport {
    pub connect_timeout: Duration,
    pub operation_timeout: Duration,
    pub cancel: CancelToken,
    known_hosts_path: Option<std::path::PathBuf>,
    ssh_binary_override: Option<std::path::PathBuf>,
    spawn_hook: Option<SpawnHook>,
}

/// Deterministic observation seam immediately before the OS process spawn.
/// Production leaves it unset; hermetic proofs use it with a harmless binary
/// override, so no network or live SSH success is fabricated.
pub type SpawnHook = std::sync::Arc<dyn Fn(&[String]) + Send + Sync>;

/// The remote command words for one allowed operation. The set of operations
/// is CLOSED — there is no constructor accepting arbitrary user strings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteOperation {
    /// Version + capability handshake: `aetherctl --version` equivalent.
    VersionProbe,
    /// Read-only capability listing.
    Capabilities,
    /// Read-only security audit (serialized JSON targets on argv).
    SecurityAudit { targets_json: String },
    /// Read-only compliance report collection to a remote temp path.
    ComplianceCollect { profile: &'static str },
    /// Print a remote report file (bounded) for local verification.
    ReportPrint { path: String },
}

impl RemoteOperation {
    /// Fixed command words per operation. The only interpolated values are
    /// the audit targets JSON (strictly validated/quoted) and the report
    /// path (validated: absolute, no whitespace/metacharacters — and it is
    /// shell-quoted regardless). `--output json` keeps parsing deterministic.
    pub fn command_words(self) -> Result<Vec<String>, TransportError> {
        match self {
            RemoteOperation::VersionProbe => Ok(vec!["aetherctl".into(), "--version".into()]),
            RemoteOperation::Capabilities => Ok(vec![
                "aetherctl".into(),
                "--output".into(),
                "json".into(),
                "capabilities".into(),
            ]),
            RemoteOperation::SecurityAudit { targets_json } => {
                validate_targets_json(&targets_json)?;
                Ok(vec![
                    "aetherctl".into(),
                    "--output".into(),
                    "json".into(),
                    "sec".into(),
                    "audit".into(),
                    "--targets-json".into(),
                    targets_json,
                ])
            }
            RemoteOperation::ComplianceCollect { profile } => {
                if !matches!(profile, "cis-l1" | "cis-l2") {
                    return Err(TransportError::InvalidOperation(format!(
                        "unknown profile {profile}"
                    )));
                }
                Ok(vec![
                    "aetherctl".into(),
                    "--output".into(),
                    "json".into(),
                    "sec".into(),
                    "audit".into(),
                    "--profile".into(),
                    profile.to_string(),
                    "--format".into(),
                    "json".into(),
                ])
            }
            RemoteOperation::ReportPrint { path } => {
                validate_remote_path(&path)?;
                Ok(vec!["cat".into(), path])
            }
        }
    }
}

fn validate_targets_json(raw: &str) -> Result<(), TransportError> {
    // Must parse as JSON; must be an array of objects with only allowed keys.
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|error| TransportError::InvalidOperation(format!("targets json: {error}")))?;
    let items = value
        .as_array()
        .ok_or_else(|| TransportError::InvalidOperation("targets must be a JSON array".into()))?;
    if items.len() > 64 {
        return Err(TransportError::InvalidOperation("too many targets".into()));
    }
    for item in items {
        let object = item.as_object().ok_or_else(|| {
            TransportError::InvalidOperation("each target must be an object".into())
        })?;
        for key in object.keys() {
            if !matches!(key.as_str(), "kind" | "path") {
                return Err(TransportError::InvalidOperation(format!(
                    "unexpected target key {key:?}"
                )));
            }
        }
        let kind = object
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| TransportError::InvalidOperation("target kind missing".into()))?;
        if !matches!(
            kind,
            "ssh" | "sudoers" | "fs" | "authlog" | "secrets" | "firewall"
        ) {
            return Err(TransportError::InvalidOperation(format!(
                "unknown target kind {kind:?}"
            )));
        }
        if let Some(path) = object.get("path").and_then(serde_json::Value::as_str) {
            validate_remote_path(path)?;
        }
    }
    Ok(())
}

fn validate_remote_path(path: &str) -> Result<(), TransportError> {
    if path.is_empty()
        || path.len() > 4096
        || !path.starts_with('/')
        || path.bytes().any(|b| {
            b.is_ascii_control()
                || matches!(
                    b,
                    b' ' | b'\'' | b'"' | b'`' | b'$' | b';' | b'&' | b'|' | b'<' | b'>'
                )
        })
    {
        return Err(TransportError::InvalidOperation(format!(
            "unsafe remote path {path:?}"
        )));
    }
    Ok(())
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TransportError {
    #[error("ssh binary not available on this host")]
    SshMissing,
    #[error("invalid operation requested: {0}")]
    InvalidOperation(String),
    #[error("transport I/O: {0}")]
    Io(String),
    #[error("AetherCore-owned known_hosts store is required")]
    TrustStoreRequired,
    #[error("AetherCore-owned known_hosts store is missing")]
    KnownHostsMissing,
}

/// A parsed remote version/capability handshake result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteCompatibility {
    pub aetherctl_version: String,
    pub remote_schema: String,
    pub supported: bool,
}

/// Highest remote contract this build can drive; older/newer remotes are
/// typed Incompatible rather than best-effort driven.
pub const REMOTE_CONTRACT_VERSION: &str = "fleet.remote.v1";

pub fn parse_compatibility(stdout: &str) -> Option<RemoteCompatibility> {
    // Expected shape from `aetherctl --output json version`:
    // {"schema":"aethercore.aetherctl.v1",...,"data":{"version":"x.y.z",...}}
    let envelope: serde_json::Value = serde_json::from_str(stdout.trim()).ok()?;
    if envelope.get("schema")?.as_str()? != "aethercore.aetherctl.v1" {
        return None;
    }
    let version = envelope.get("data")?.get("version")?.as_str()?.to_string();
    Some(RemoteCompatibility {
        aetherctl_version: version,
        remote_schema: "aethercore.aetherctl.v1".to_string(),
        supported: true,
    })
}

/// Phase 34 corrective (F): compatibility verdict over the REAL remote
/// command output. Semver alone must NOT authorize compatibility: the
/// remote envelope must carry the product's remote contract marker
/// (`remoteContract`) matching [`REMOTE_CONTRACT_VERSION`]. A wrong or
/// missing marker is typed `Incompatible` even when the version parses.
///
/// Accepted input is the exact stdout consumed by the fleet handshake:
/// `aetherctl --output json capabilities` (carrying the marker) or
/// `aetherctl --output json version` (marker-less → incompatible).
pub fn compatibility_verdict(stdout: &str) -> RemoteCompatibility {
    const MARKER_KEY: &str = "remoteContract";
    let envelope: serde_json::Value = match serde_json::from_str(stdout.trim()) {
        Ok(value) => value,
        Err(_) => {
            return RemoteCompatibility {
                aetherctl_version: String::new(),
                remote_schema: String::new(),
                supported: false,
            };
        }
    };
    let schema_ok = envelope.get("schema").and_then(serde_json::Value::as_str)
        == Some("aethercore.aetherctl.v1");
    let version = envelope
        .get("data")
        .and_then(|data| data.get("version"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let marker = envelope
        .get("data")
        .and_then(|data| data.get(MARKER_KEY))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    // Supported requires: product envelope + parseable version + EXACT
    // remote contract marker. Any mismatch → Incompatible (supported=false).
    let supported = schema_ok && !version.is_empty() && marker == REMOTE_CONTRACT_VERSION;
    RemoteCompatibility {
        aetherctl_version: version,
        remote_schema: marker,
        supported,
    }
}

impl SshTransport {
    pub fn new(operation_timeout: Duration) -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            operation_timeout,
            cancel: CancelToken::default(),
            known_hosts_path: None,
            ssh_binary_override: None,
            spawn_hook: None,
        }
    }

    pub fn with_known_hosts_path(mut self, path: &std::path::Path) -> Self {
        self.known_hosts_path = Some(path.to_path_buf());
        self
    }

    pub fn with_ssh_binary(mut self, path: &std::path::Path) -> Self {
        self.ssh_binary_override = Some(path.to_path_buf());
        self
    }

    pub fn with_spawn_hook<F>(mut self, hook: F) -> Self
    where
        F: Fn(&[String]) + Send + Sync + 'static,
    {
        self.spawn_hook = Some(std::sync::Arc::new(hook));
        self
    }

    /// Executes one allowed remote operation. Typed honest outcomes.
    pub fn execute(
        &self,
        host: &FleetHost,
        identity_file: Option<&str>,
        operation: RemoteOperation,
    ) -> Result<RemoteResult, TransportError> {
        if self.cancel.is_cancelled() {
            return Ok(RemoteResult::kind(RemoteOutcomeKind::Cancelled)
                .detail("cancelled before spawn".to_string()));
        }
        let known_hosts_path = self
            .known_hosts_path
            .as_deref()
            .ok_or(TransportError::TrustStoreRequired)?;
        if !known_hosts_path.is_file() {
            return Err(TransportError::KnownHostsMissing);
        }
        let ssh = self
            .ssh_binary_override
            .clone()
            .or_else(ssh_binary)
            .ok_or(TransportError::SshMissing)?;
        let words = operation.command_words()?;
        let word_refs: Vec<&str> = words.iter().map(String::as_str).collect();
        let argv = build_remote_argv(
            &ssh,
            host,
            identity_file,
            known_hosts_path,
            self.connect_timeout,
            &word_refs,
        );

        let mut command = Command::new(&argv[0]);
        command
            .args(&argv[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // No shell: argv[0] is the ssh binary; args are individual elements.
        if let Some(hook) = &self.spawn_hook {
            hook(&argv);
        }
        let mut child: Child = command
            .spawn()
            .map_err(|error| TransportError::Io(error.to_string()))?;

        let started = Instant::now();
        let deadline = started + self.operation_timeout;
        let status = loop {
            if self.cancel.is_cancelled() {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(RemoteResult::kind(RemoteOutcomeKind::Cancelled)
                    .detail("cancelled before completion".to_string()));
            }
            match child
                .try_wait()
                .map_err(|error| TransportError::Io(error.to_string()))?
            {
                Some(status) => break status,
                None if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Ok(RemoteResult::kind(RemoteOutcomeKind::Timeout)
                        .detail("operation timeout elapsed".to_string()));
                }
                None => std::thread::sleep(Duration::from_millis(20)),
            }
        };

        let exit_code = status.code().unwrap_or(-1);
        // Capture remaining output with bounds.
        let mut stdout_pipe = child.stdout.take();
        let mut stderr_pipe = child.stderr.take();
        let stdout = capture_bounded(&mut stdout_pipe);
        let stderr = capture_bounded(&mut stderr_pipe);
        let truncated = stdout.1 || stderr.1;
        let mut result = classify_ssh_exit(exit_code, &stderr.0);
        result.output = Some(RemoteOutput {
            stdout: stdout.0,
            stderr: stderr.0,
            truncated,
        });
        if exit_code == 0 {
            result.exit_code = Some(0);
        }
        Ok(result)
    }
}

/// Trust-admitting transport used by both the CLI scheduler and the desktop
/// Fleet commands. The admission check happens before `SshTransport::execute`,
/// so untrusted, revoked, malformed, or mismatched hosts cannot reach spawn.
pub struct TrustedSshTransport {
    inner: SshTransport,
    trust_store: TrustStore,
}

impl TrustedSshTransport {
    pub fn new(trust_store: TrustStore, operation_timeout: Duration) -> Self {
        let inner = SshTransport::new(operation_timeout).with_known_hosts_path(trust_store.path());
        Self { inner, trust_store }
    }

    pub fn with_ssh_binary(mut self, path: &std::path::Path) -> Self {
        self.inner = self.inner.with_ssh_binary(path);
        self
    }

    pub fn with_spawn_hook<F>(mut self, hook: F) -> Self
    where
        F: Fn(&[String]) + Send + Sync + 'static,
    {
        self.inner = self.inner.with_spawn_hook(hook);
        self
    }

    pub fn admit(&self, host: &FleetHost) -> Result<TrustDecision, crate::trust::TrustError> {
        self.trust_store.admit(host)
    }

    pub fn execute_operation(&self, host: &FleetHost, operation: RemoteOperation) -> RemoteResult {
        let decision = match self.trust_store.admit(host) {
            Ok(decision) => decision,
            Err(error) => {
                return RemoteResult::kind(RemoteOutcomeKind::NotVerified)
                    .detail(format!("trust record rejected: {error}"));
            }
        };
        match decision {
            TrustDecision::Trusted => {
                let identity = match &host.auth {
                    crate::domain::AuthReference::Agent => None,
                    crate::domain::AuthReference::KeyFile { path }
                    | crate::domain::AuthReference::Certificate { path } => Some(path.as_str()),
                };
                self.inner
                    .execute(host, identity, operation)
                    .unwrap_or_else(|error| {
                        let outcome = match error {
                            TransportError::SshMissing | TransportError::KnownHostsMissing => {
                                RemoteOutcomeKind::NotAvailable
                            }
                            _ => RemoteOutcomeKind::Failed,
                        };
                        RemoteResult::kind(outcome).detail(error.to_string())
                    })
            }
            TrustDecision::NotVerified => RemoteResult::kind(RemoteOutcomeKind::NotVerified)
                .detail("host is not authorized in the AetherCore trust store"),
            TrustDecision::HostKeyMismatch => {
                RemoteResult::kind(RemoteOutcomeKind::HostKeyMismatch)
                    .detail("authorized public host key differs from the inventory pin")
            }
        }
    }

    pub fn execute(&self, host: &FleetHost) -> RemoteResult {
        self.execute_for_profile(host, "cis-l1")
    }

    pub fn execute_for_profile(&self, host: &FleetHost, profile_id: &str) -> RemoteResult {
        let profile: &'static str = match profile_id {
            "cis-l1" => "cis-l1",
            "cis-l2" => "cis-l2",
            other => {
                return RemoteResult::kind(RemoteOutcomeKind::Incompatible)
                    .detail(format!("unsupported compliance profile {other}"));
            }
        };
        self.execute_operation(host, RemoteOperation::ComplianceCollect { profile })
    }
}

impl crate::orchestrator::FleetTransport for TrustedSshTransport {
    fn execute(&self, host: &FleetHost) -> RemoteResult {
        TrustedSshTransport::execute(self, host)
    }

    fn execute_for_profile(&self, host: &FleetHost, profile_id: &str) -> RemoteResult {
        TrustedSshTransport::execute_for_profile(self, host, profile_id)
    }
}

fn capture_bounded(pipe: &mut Option<impl Read>) -> (String, bool) {
    let mut buffer = Vec::new();
    if let Some(pipe) = pipe {
        let mut chunk = [0u8; 8192];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if buffer.len() < MAX_CAPTURE_BYTES {
                        let remaining = MAX_CAPTURE_BYTES - buffer.len();
                        buffer.extend_from_slice(&chunk[..n.min(remaining)]);
                    } else {
                        break;
                    }
                }
            }
        }
    }
    let truncated = buffer.len() >= MAX_CAPTURE_BYTES;
    (String::from_utf8_lossy(&buffer).into_owned(), truncated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AuthReference, FleetHost};
    use crate::trust::{TrustedHostKey, fingerprint_of_blob};
    use std::collections::BTreeSet;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

    fn trust_fixture() -> (FleetHost, TrustStore, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "aethercore-transport-proof-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let store = TrustStore::open(&dir).unwrap();
        let key = "AAAAC3NzaC1lZDI1NTE5AAAAIB3qSmVOUdM=";
        let fp = fingerprint_of_blob(key).unwrap();
        let record = TrustedHostKey::authorize(
            "host-a",
            "a.example.internal",
            22,
            "ssh-ed25519",
            key,
            &fp,
            42,
        )
        .unwrap();
        store.trust(&record).unwrap();
        let mut host = FleetHost::new(
            "host-a",
            "Host A",
            "a.example.internal",
            22,
            "ops",
            AuthReference::Agent,
            BTreeSet::new(),
        )
        .unwrap();
        host.pin_trust(&fp, "ssh-ed25519", 42).unwrap();
        (host, store, dir)
    }

    #[test]
    fn trust_1_authorized_schedule_reaches_spawn_once() {
        let (host, store, dir) = trust_fixture();
        let spawns = std::sync::Arc::new(AtomicUsize::new(0));
        let seen = std::sync::Arc::clone(&spawns);
        let transport = TrustedSshTransport::new(store, Duration::from_secs(2))
            .with_ssh_binary(Path::new("/usr/bin/true"))
            .with_spawn_hook(move |argv| {
                seen.fetch_add(1, Ordering::SeqCst);
                assert!(argv.iter().any(|arg| arg == "StrictHostKeyChecking=yes"));
                assert!(
                    argv.iter()
                        .any(|arg| arg.starts_with("UserKnownHostsFile="))
                );
            });
        let result = transport.execute_for_profile(&host, "cis-l2");
        assert_eq!(result.outcome, RemoteOutcomeKind::Success);
        assert_eq!(spawns.load(Ordering::SeqCst), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn trust_2_missing_record_blocks_spawn() {
        let dir = std::env::temp_dir().join(format!(
            "aethercore-transport-proof-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let store = TrustStore::open(&dir).unwrap();
        let mut host = FleetHost::new(
            "host-a",
            "Host A",
            "a.example.internal",
            22,
            "ops",
            AuthReference::Agent,
            BTreeSet::new(),
        )
        .unwrap();
        let spawns = std::sync::Arc::new(AtomicUsize::new(0));
        let seen = std::sync::Arc::clone(&spawns);
        let transport = TrustedSshTransport::new(store, Duration::from_secs(2))
            .with_ssh_binary(Path::new("/usr/bin/true"))
            .with_spawn_hook(move |_| {
                seen.fetch_add(1, Ordering::SeqCst);
            });
        assert_eq!(
            transport.execute(&host).outcome,
            RemoteOutcomeKind::NotVerified
        );
        assert_eq!(spawns.load(Ordering::SeqCst), 0);
        host.enabled = false;
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn trust_3_mismatched_record_blocks_spawn() {
        let (host, store, dir) = trust_fixture();
        let new_key = "AAAAC3NzaC1lZDI1NTE5AAAAIfffffffffffffff=";
        let new_fp = fingerprint_of_blob(new_key).unwrap();
        let record = TrustedHostKey::authorize(
            "host-a",
            "a.example.internal",
            22,
            "ssh-ed25519",
            new_key,
            &new_fp,
            43,
        )
        .unwrap();
        store.trust(&record).unwrap();
        let spawns = std::sync::Arc::new(AtomicUsize::new(0));
        let seen = std::sync::Arc::clone(&spawns);
        let transport = TrustedSshTransport::new(store, Duration::from_secs(2))
            .with_ssh_binary(Path::new("/usr/bin/true"))
            .with_spawn_hook(move |_| {
                seen.fetch_add(1, Ordering::SeqCst);
            });
        assert_eq!(
            transport.execute(&host).outcome,
            RemoteOutcomeKind::HostKeyMismatch
        );
        assert_eq!(spawns.load(Ordering::SeqCst), 0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn trust_4_malformed_record_is_typed_and_blocks_spawn() {
        let dir = std::env::temp_dir().join(format!(
            "aethercore-transport-proof-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let store = TrustStore::open(&dir).unwrap();
        std::fs::write(
            dir.join("trusted_keys.json"),
            r#"[{"host_id":"host-a","hostname":"a.example.internal","port":22,"key_type":"ssh-ed25519","public_key_base64":"-----BEGIN OPENSSH PRIVATE KEY-----","host_key_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","trusted_unix_ms":1,"provenance":null}]"#,
        )
        .unwrap();
        let host = FleetHost::new(
            "host-a",
            "Host A",
            "a.example.internal",
            22,
            "ops",
            AuthReference::Agent,
            BTreeSet::new(),
        )
        .unwrap();
        let spawns = std::sync::Arc::new(AtomicUsize::new(0));
        let seen = std::sync::Arc::clone(&spawns);
        let transport = TrustedSshTransport::new(store, Duration::from_secs(2))
            .with_ssh_binary(Path::new("/usr/bin/true"))
            .with_spawn_hook(move |_| {
                seen.fetch_add(1, Ordering::SeqCst);
            });
        assert_eq!(
            transport.execute(&host).outcome,
            RemoteOutcomeKind::NotVerified
        );
        assert_eq!(spawns.load(Ordering::SeqCst), 0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn trust_5_revocation_regenerates_and_blocks_subsequent_run() {
        let (host, store, dir) = trust_fixture();
        store.untrust("a.example.internal", 22).unwrap();
        assert!(
            std::fs::read_to_string(store.path())
                .unwrap()
                .trim()
                .is_empty()
        );
        let spawns = std::sync::Arc::new(AtomicUsize::new(0));
        let seen = std::sync::Arc::clone(&spawns);
        let transport = TrustedSshTransport::new(store, Duration::from_secs(2))
            .with_ssh_binary(Path::new("/usr/bin/true"))
            .with_spawn_hook(move |_| {
                seen.fetch_add(1, Ordering::SeqCst);
            });
        assert_eq!(
            transport.execute(&host).outcome,
            RemoteOutcomeKind::NotVerified
        );
        assert_eq!(spawns.load(Ordering::SeqCst), 0);
        let _ = std::fs::remove_dir_all(dir);
    }
}
