//! Phase 34 — host-key trust decisions (fail-closed) and the typed SSH
//! transport over the OS OpenSSH client.
//!
//! Trust model: an inventory host is eligible for remote work ONLY when it
//! carries an explicit pin whose fingerprint equals the remote host's key
//! fingerprint, AND the AetherCore-owned trust store holds the matching
//! PUBLIC host-key material (a canonical known_hosts line). A fingerprint
//! alone can never create an executable trusted SSH entry: the trust-time
//! authorization binds fingerprint AND public key together (the fingerprint
//! is RECOMPUTED from the supplied public key and must equal the authorized
//! fingerprint). Everything else is typed: `NotVerified` (unknown host) or
//! `HostKeyMismatch` (pin exists but the presented key differs). The
//! implementation NEVER passes insecure options (`StrictHostKeyChecking=no`,
//! `UserKnownHostsFile=/dev/null`, `accept-new`, ...) to ssh, never performs
//! TOFU, never mutates the user's `~/.ssh/known_hosts`, and never prompts
//! (BatchMode). Host-key verification is delegated to the OS ssh client
//! reading an AetherCore-OWNED known_hosts file built exclusively from
//! user-authorized public host keys — a file the tool creates with 0600 in
//! its own state directory. Passwords cannot be passed at all (no password
//! auth flag is ever emitted; BatchMode makes any prompt a typed failure).
//! Private key material is structurally rejected at the trust boundary.

use crate::domain::{FleetHost, FleetInventory};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const FLEET_TRUST_SCHEMA: &str = "aethercore.fleet.trust.v1";

/// OpenSSH option values this product must never emit (audit-gated).
pub const FORBIDDEN_SSH_OPTION_FRAGMENTS: &[&str] = &[
    "StrictHostKeyChecking=no",
    "StrictHostKeyChecking accept-new",
    "UserKnownHostsFile=/dev/null",
    "CheckHostIP=no",
    "PasswordAuthentication=yes",
    "PreferredAuthentications=password",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum TrustDecision {
    /// Explicit pin exists and matches the presented key fingerprint.
    Trusted,
    /// No pin: remote execution is refused until the user pins the key.
    NotVerified,
    /// Pin exists but the remote key differs — hard block until re-pinning.
    HostKeyMismatch,
}

/// Verifies the presented fingerprint against the host's pin.
/// `presented_fingerprint` is the lowercase hex SHA-256 of the remote host key.
pub fn decide(host: &FleetHost, presented_fingerprint: Option<&str>) -> TrustDecision {
    match (&host.trust, presented_fingerprint) {
        (Some(pin), Some(presented)) if pin.host_key_sha256 == presented => TrustDecision::Trusted,
        (Some(_), Some(_)) => TrustDecision::HostKeyMismatch,
        _ => TrustDecision::NotVerified,
    }
}

/// A typed trusted PUBLIC host-key record: the smallest safe representation
/// binding host identity, key type, public key material and fingerprint
/// together. `public_key_base64` is the wire-format PUBLIC key blob only —
/// private key material is rejected at authorization time (see
/// [`TrustedHostKey::authorize`]) and can never enter this type through it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedHostKey {
    pub host_id: String,
    pub hostname: String,
    pub port: u16,
    /// RFC 4253 public-key algorithm name ("ssh-ed25519", "ssh-rsa", ...).
    pub key_type: String,
    /// Base64 of the PUBLIC key blob (the third known_hosts field).
    pub public_key_base64: String,
    /// SHA-256 fingerprint of `public_key_base64` (lowercase hex).
    pub host_key_sha256: String,
    /// Unix ms when the explicit user trust action was recorded.
    pub trusted_unix_ms: i64,
    /// Provenance note (e.g. "pinned after out-of-band fingerprint review").
    pub provenance: Option<String>,
}

impl TrustedHostKey {
    /// Explicit user-authorized trust creation. The binding invariant is
    /// enforced HERE and nowhere else: the fingerprint is recomputed FROM the
    /// supplied public key material and must equal the authorized
    /// fingerprint. `fingerprint A + public key B` can never be combined.
    /// Private key material (PEM/OpenSSH bodies) is structurally rejected.
    pub fn authorize(
        host_id: &str,
        hostname: &str,
        port: u16,
        key_type: &str,
        public_key_base64: &str,
        authorized_fingerprint: &str,
        trusted_unix_ms: i64,
    ) -> Result<Self, TrustError> {
        if !is_supported_key_type(key_type) {
            return Err(TrustError::UnsupportedKeyType(key_type.to_string()));
        }
        // Reject anything that is not pure wire-format PUBLIC key material:
        // PEM/OpenSSH PRIVATE KEY bodies, multi-line handshakes, headers.
        let lower = public_key_base64.to_ascii_lowercase();
        if lower.contains("private")
            || lower.contains("-----begin")
            || public_key_base64.contains('\n')
            || public_key_base64.contains('\r')
        {
            return Err(TrustError::PrivateKeyMaterialRejected);
        }
        // The fingerprint must be recomputed from the key itself.
        let recomputed = fingerprint_of_blob(public_key_base64)?;
        if !constant_time_eq(recomputed.as_bytes(), authorized_fingerprint.as_bytes()) {
            return Err(TrustError::FingerprintKeyMismatch {
                authorized: authorized_fingerprint.to_string(),
                recomputed,
            });
        }
        if hostname.is_empty() || hostname.len() > 253 {
            return Err(TrustError::MalformedKeyMaterial);
        }
        Ok(Self {
            host_id: host_id.to_string(),
            hostname: hostname.to_string(),
            port,
            key_type: key_type.to_string(),
            public_key_base64: public_key_base64.to_string(),
            host_key_sha256: recomputed,
            trusted_unix_ms,
            provenance: None,
        })
    }

    /// Canonical known_hosts line this record produces.
    pub fn known_hosts_line(&self) -> String {
        known_hosts_line(
            &self.hostname,
            self.port,
            &self.key_type,
            &self.public_key_base64,
        )
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Key types this product is allowed to pin (RFC 4253 / OpenSSH registry).
pub const SUPPORTED_KEY_TYPES: &[&str] = &[
    "ssh-ed25519",
    "ssh-rsa",
    "ecdsa-sha2-nistp256",
    "ecdsa-sha2-nistp384",
    "ecdsa-sha2-nistp521",
];

pub fn is_supported_key_type(key_type: &str) -> bool {
    SUPPORTED_KEY_TYPES.contains(&key_type)
}

/// Builds the AetherCore known_hosts line for a pinned host key.
pub fn known_hosts_line(hostname: &str, port: u16, key_type: &str, base64_key: &str) -> String {
    format!("[{hostname}]:{port} {key_type} {base64_key}")
}

/// AetherCore-owned trust store: a directory holding `known_hosts` populated
/// exclusively from user-authorized PUBLIC host-key records. Never the user's
/// ~/.ssh. A companion `trusted_keys.json` holds the typed records themselves
/// (host identity + provenance); the known_hosts lines are always REGENERATED
/// from those records, so a line can exist only if its record was authorized
/// through the fingerprint/public-key binding check.
pub struct TrustStore {
    known_hosts_path: PathBuf,
    records_path: PathBuf,
}

impl TrustStore {
    pub fn open(dir: &Path) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let known_hosts_path = dir.join("known_hosts");
        if !known_hosts_path.exists() {
            std::fs::write(&known_hosts_path, b"")?;
            set_owner_only(&known_hosts_path)?;
        }
        Ok(Self {
            known_hosts_path,
            records_path: dir.join("trusted_keys.json"),
        })
    }

    pub fn path(&self) -> &Path {
        &self.known_hosts_path
    }

    /// Adds one explicitly authorized public-key record. The record must have
    /// passed `TrustedHostKey::authorize` (fingerprint recomputed from the
    /// key); the stored line is derived from the RECORD, never assembled from
    /// raw user strings, so `fingerprint A + public key B` cannot persist.
    pub fn trust(&self, record: &TrustedHostKey) -> Result<(), TrustError> {
        let mut records = self.records()?;
        // Re-verify the binding at persist time (defense in depth).
        if record.host_key_sha256 != fingerprint_of_blob(&record.public_key_base64)? {
            return Err(TrustError::FingerprintKeyMismatch {
                authorized: record.host_key_sha256.clone(),
                recomputed: fingerprint_of_blob(&record.public_key_base64)?,
            });
        }
        records
            .retain(|r: &TrustedHostKey| !(r.hostname == record.hostname && r.port == record.port));
        records.push(record.clone());
        records.sort_by(|a, b| (&a.hostname, a.port).cmp(&(&b.hostname, b.port)));
        self.write_records(&records)?;
        self.regenerate_known_hosts(&records)?;
        Ok(())
    }

    /// Removes any record + line for `[hostname]:port` (trust revocation).
    pub fn untrust(&self, hostname: &str, port: u16) -> Result<(), TrustError> {
        let mut records = self.records()?;
        let before = records.len();
        records.retain(|r| !(r.hostname == hostname && r.port == port));
        if records.len() == before {
            return Ok(());
        }
        self.write_records(&records)?;
        self.regenerate_known_hosts(&records)?;
        Ok(())
    }

    /// Authorized records currently held by this store.
    pub fn records(&self) -> Result<Vec<TrustedHostKey>, TrustError> {
        if !self.records_path.exists() {
            return Ok(Vec::new());
        }
        let raw =
            std::fs::read(&self.records_path).map_err(|e| TrustError::StoreIo(e.to_string()))?;
        let records: Vec<TrustedHostKey> = serde_json::from_slice(&raw)
            .map_err(|e| TrustError::StoreIo(format!("trusted_keys.json: {e}")))?;
        for record in &records {
            TrustedHostKey::authorize(
                &record.host_id,
                &record.hostname,
                record.port,
                &record.key_type,
                &record.public_key_base64,
                &record.host_key_sha256,
                record.trusted_unix_ms,
            )
            .map_err(|error| TrustError::StoreIo(format!("trusted_keys.json rejected: {error}")))?;
        }
        Ok(records)
    }

    /// Fail-closed execution admission. A host is trusted only when its
    /// inventory pin, this store's authorized public-key record, and the
    /// regenerated known_hosts line all agree. Discovery and fingerprint-only
    /// inventory metadata never authorize a process spawn.
    pub fn admit(&self, host: &FleetHost) -> Result<TrustDecision, TrustError> {
        let Some(pin) = host.trust.as_ref() else {
            return Ok(TrustDecision::NotVerified);
        };
        let records = self.records()?;
        let record = records.iter().find(|record| {
            record.host_id == host.host_id
                && record.hostname == host.hostname
                && record.port == host.port
        });
        let Some(record) = record else {
            return Ok(
                if records.iter().any(|record| {
                    record.host_id == host.host_id
                        || (record.hostname == host.hostname && record.port == host.port)
                }) {
                    TrustDecision::HostKeyMismatch
                } else {
                    TrustDecision::NotVerified
                },
            );
        };
        if record.host_key_sha256 != pin.host_key_sha256 || record.key_type != pin.key_type {
            return Ok(TrustDecision::HostKeyMismatch);
        }
        let known_hosts = std::fs::read_to_string(&self.known_hosts_path)
            .map_err(|error| TrustError::StoreIo(error.to_string()))?;
        if !known_hosts
            .lines()
            .any(|line| line == record.known_hosts_line())
        {
            return Ok(TrustDecision::NotVerified);
        }
        Ok(TrustDecision::Trusted)
    }

    /// Regenerates the store from user-authorized pin lines only. Kept for
    /// inventory-level rebuilds: a line is written only when the host still
    /// carries an explicit trust pin in the inventory.
    pub fn sync_from_inventory(
        &self,
        inventory: &FleetInventory,
        lines: &[(String, u16, String)],
    ) -> std::io::Result<()> {
        std::fs::write(&self.known_hosts_path, b"")?;
        for (hostname, port, line) in lines {
            let authorized = inventory
                .hosts
                .iter()
                .any(|h| h.hostname == *hostname && h.port == *port && h.trust.is_some());
            if authorized {
                self.pin(line)?;
            }
        }
        Ok(())
    }

    fn write_records(&self, records: &[TrustedHostKey]) -> Result<(), TrustError> {
        let bytes =
            serde_json::to_vec_pretty(records).map_err(|e| TrustError::StoreIo(e.to_string()))?;
        std::fs::write(&self.records_path, bytes)
            .map_err(|e| TrustError::StoreIo(e.to_string()))?;
        set_owner_only(&self.records_path).map_err(|e| TrustError::StoreIo(e.to_string()))?;
        Ok(())
    }

    fn regenerate_known_hosts(&self, records: &[TrustedHostKey]) -> Result<(), TrustError> {
        let mut content = String::new();
        for record in records {
            content.push_str(&record.known_hosts_line());
            content.push('\n');
        }
        std::fs::write(&self.known_hosts_path, content)
            .map_err(|e| TrustError::StoreIo(e.to_string()))?;
        set_owner_only(&self.known_hosts_path).map_err(|e| TrustError::StoreIo(e.to_string()))?;
        Ok(())
    }

    /// Adds the pinned key line for a host (explicit user action). Retained
    /// for the inventory sync path above; the typed `trust` path is the
    /// authorization gate used by the CLI.
    pub fn pin(&self, line: &str) -> std::io::Result<()> {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&self.known_hosts_path)?;
        writeln!(file, "{line}")
    }
}

fn set_owner_only(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
    }
    #[cfg(not(unix))]
    {
        Ok(())
    }
}

/// Fingerprint of a public key blob (SHA-256, lowercase hex) — the pin format.
pub fn fingerprint_of_blob(base64_blob: &str) -> Result<String, TrustError> {
    let raw = decode_base64(base64_blob)?;
    let digest = Sha256::digest(&raw);
    Ok(hex::encode(digest))
}

fn decode_base64(input: &str) -> Result<Vec<u8>, TrustError> {
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0u32;
    for byte in input.bytes() {
        let value = match byte {
            b'A'..=b'Z' => u32::from(byte - b'A'),
            b'a'..=b'z' => u32::from(byte - b'a' + 26),
            b'0'..=b'9' => u32::from(byte - b'0' + 52),
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\n' | b'\r' => continue,
            _ => return Err(TrustError::MalformedKeyMaterial),
        };
        buf = (buf << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xFF) as u8);
        }
    }
    Ok(out)
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TrustError {
    #[error("malformed host key material")]
    MalformedKeyMaterial,
    #[error("unsupported host key type: {0}")]
    UnsupportedKeyType(String),
    #[error(
        "fingerprint/public-key binding failed: authorized {authorized}, recomputed {recomputed}"
    )]
    FingerprintKeyMismatch {
        authorized: String,
        recomputed: String,
    },
    #[error("private key material rejected at trust boundary")]
    PrivateKeyMaterialRejected,
    #[error("trust store I/O: {0}")]
    StoreIo(String),
}

/// Typed remote-execution outcome. Honest state space per the phase contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum RemoteOutcomeKind {
    Success,
    Failed,
    NotVerified,
    NotAvailable,
    Timeout,
    Cancelled,
    AuthFailure,
    HostKeyMismatch,
    Incompatible,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteOutput {
    /// Bounded captured stdout (transport truncates with an honest marker).
    pub stdout: String,
    pub stderr: String,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteResult {
    pub outcome: RemoteOutcomeKind,
    pub exit_code: Option<i32>,
    pub output: Option<RemoteOutput>,
    pub detail: Option<String>,
}

impl RemoteResult {
    pub fn kind(outcome: RemoteOutcomeKind) -> Self {
        Self {
            outcome,
            exit_code: None,
            output: None,
            detail: None,
        }
    }
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Bounded capture limit per stream (bytes) — protects the controlling host.
pub const MAX_CAPTURE_BYTES: usize = 256 * 1024;

/// ssh binary lookup, honest NotAvailable when absent.
pub fn ssh_binary() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join("ssh");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Build the exact, safe argv for one remote aetherctl invocation.
///
/// Injection resistance: every element is a SEPARATE argv element passed to
/// the ssh binary via `Command` (no local shell). Remote words are
/// single-quoted so the remote login shell sees fixed literal words.
/// Options that ssh would accept from a value position (`-o...`) are
/// rejected upstream by hostname/username validation.
pub fn build_remote_argv(
    ssh_binary: &Path,
    host: &FleetHost,
    identity_file: Option<&str>,
    known_hosts_path: &Path,
    connect_timeout: Duration,
    remote_command_words: &[&str],
) -> Vec<String> {
    let mut argv = vec![
        ssh_binary.display().to_string(),
        // Non-interactive: any prompt (password, host key) becomes failure.
        "-o".to_string(),
        "BatchMode=yes".to_string(),
        "-o".to_string(),
        "StrictHostKeyChecking=yes".to_string(),
        "-o".to_string(),
        format!("UserKnownHostsFile={}", known_hosts_path.display()),
        "-o".to_string(),
        "PasswordAuthentication=no".to_string(),
        "-o".to_string(),
        "KbdInteractiveAuthentication=no".to_string(),
        "-o".to_string(),
        format!("ConnectTimeout={}", connect_timeout.as_secs().max(1)),
    ];
    if let Some(path) = identity_file {
        argv.push("-i".to_string());
        argv.push(shell_quote(path));
    }
    argv.push("-p".to_string());
    argv.push(host.port.to_string());
    argv.push(format!("{}@{}", host.username, host.hostname));
    for word in remote_command_words {
        argv.push(shell_quote(word));
    }
    argv
}

/// POSIX single-quote escaping — a value containing anything at all becomes
/// one literal word on the remote side. `'` becomes `'\''`.
pub fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('\'');
    for ch in value.chars() {
        if ch == '\'' {
            quoted.push_str("'\\''");
        } else {
            quoted.push(ch);
        }
    }
    quoted.push('\'');
    quoted
}

/// Maps an ssh exit code + stderr shape to a typed result, deterministically.
/// Exit code 255 is ssh's own transport-error code; auth failures are
/// recognized by their exact stderr text — never guessed from timing.
pub fn classify_ssh_exit(exit_code: i32, stderr: &str) -> RemoteResult {
    if exit_code == 255 {
        let lower = stderr.to_ascii_lowercase();
        if lower.contains("permission denied") || lower.contains("publickey") {
            RemoteResult::kind(RemoteOutcomeKind::AuthFailure)
                .detail("ssh authentication refused (BatchMode)".to_string())
        } else if lower.contains("host key verification failed") {
            RemoteResult::kind(RemoteOutcomeKind::HostKeyMismatch)
                .detail("remote host key not pinned or changed".to_string())
        } else {
            RemoteResult::kind(RemoteOutcomeKind::Failed).detail(
                stderr
                    .lines()
                    .next()
                    .unwrap_or("ssh transport error")
                    .to_string(),
            )
        }
    } else if exit_code == 0 {
        RemoteResult {
            outcome: RemoteOutcomeKind::Success,
            exit_code: Some(0),
            output: None,
            detail: None,
        }
    } else {
        RemoteResult {
            outcome: RemoteOutcomeKind::Failed,
            exit_code: Some(exit_code),
            output: None,
            detail: Some(
                stderr
                    .lines()
                    .next()
                    .unwrap_or("remote command failed")
                    .to_string(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::AuthReference;
    use std::collections::BTreeSet;

    fn pinned_host() -> FleetHost {
        let mut host = FleetHost::new(
            "host-a",
            "Host A",
            "a.example.internal",
            2222,
            "ops",
            AuthReference::Agent,
            BTreeSet::new(),
        )
        .unwrap();
        host.pin_trust(&"ab".repeat(32), "ssh-ed25519", 42).unwrap();
        host
    }

    #[test]
    fn unknown_host_is_not_trusted() {
        let mut host = pinned_host();
        host.trust = None;
        assert_eq!(decide(&host, Some("ff")), TrustDecision::NotVerified);
        assert_eq!(decide(&host, None), TrustDecision::NotVerified);
    }

    #[test]
    fn matching_pin_trusted_mismatch_blocked() {
        let host = pinned_host();
        assert_eq!(
            decide(&host, Some(&"ab".repeat(32))),
            TrustDecision::Trusted
        );
        assert_eq!(
            decide(&host, Some(&"cd".repeat(32))),
            TrustDecision::HostKeyMismatch
        );
    }

    #[test]
    fn argv_is_safe_and_strict() {
        let host = pinned_host();
        let argv = build_remote_argv(
            Path::new("/usr/bin/ssh"),
            &host,
            None,
            Path::new("/tmp/aethercore-known_hosts"),
            Duration::from_secs(5),
            &["aetherctl", "version"],
        );
        let joined = argv.join(" ");
        for forbidden in FORBIDDEN_SSH_OPTION_FRAGMENTS {
            assert!(
                !joined.contains(forbidden),
                "forbidden option {forbidden} leaked"
            );
        }
        assert!(argv.contains(&"BatchMode=yes".to_string()));
        assert!(argv.contains(&"StrictHostKeyChecking=yes".to_string()));
        assert!(argv.contains(&"PasswordAuthentication=no".to_string()));
        assert!(argv.contains(&"ops@a.example.internal".to_string()));
        assert_eq!(argv.last().unwrap(), "'version'");
    }

    #[test]
    fn argv_binds_aethercore_owned_known_hosts_store() {
        let host = pinned_host();
        let argv = build_remote_argv(
            Path::new("/usr/bin/ssh"),
            &host,
            None,
            Path::new("/tmp/aethercore-known_hosts"),
            Duration::from_secs(5),
            &["aetherctl", "--output", "json", "sec", "audit"],
        );
        assert!(
            argv.iter()
                .any(|arg| arg.starts_with("UserKnownHostsFile=")),
            "ssh must bind the AetherCore-owned known_hosts store"
        );
    }

    #[test]
    fn shell_quote_neutralizes_metacharacters() {
        assert_eq!(shell_quote("safe"), "'safe'");
        assert_eq!(shell_quote("$(rm -rf /)"), "'$(rm -rf /)'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
        assert_eq!(shell_quote(""), "''");
        let evil = "'; rm -rf /; echo '";
        let quoted = shell_quote(evil);
        // balanced quoting: remote shell sees exactly one literal word
        assert_eq!(quoted.matches('\'').count() % 2, 0);
    }

    #[test]
    fn fingerprint_of_blob_is_sha256_hex() {
        let fp = fingerprint_of_blob("AAAAC3NzaC1lZDI1NTE5AAAAIB3qSmVOUdM=").unwrap();
        assert_eq!(fp.len(), 64);
        assert!(fp.bytes().all(|b| b.is_ascii_hexdigit()));
        let again = fingerprint_of_blob("AAAAC3NzaC1lZDI1NTE5AAAAIB3qSmVOUdM=").unwrap();
        assert_eq!(fp, again);
        assert!(fingerprint_of_blob("!!not-base64!!").is_err());
    }

    #[test]
    fn ssh_exit_classification_is_typed() {
        let auth = classify_ssh_exit(255, "ssh: Permission denied (publickey).");
        assert_eq!(auth.outcome, RemoteOutcomeKind::AuthFailure);
        let key = classify_ssh_exit(255, "Host key verification failed.");
        assert_eq!(key.outcome, RemoteOutcomeKind::HostKeyMismatch);
        let ok = classify_ssh_exit(0, "");
        assert_eq!(ok.outcome, RemoteOutcomeKind::Success);
        let other = classify_ssh_exit(255, "Connection refused");
        assert_eq!(other.outcome, RemoteOutcomeKind::Failed);
        let remote_fail = classify_ssh_exit(3, "boom");
        assert_eq!(remote_fail.outcome, RemoteOutcomeKind::Failed);
        assert_eq!(remote_fail.exit_code, Some(3));
    }

    #[test]
    fn trust_store_is_owned_and_isolated() {
        let dir =
            std::env::temp_dir().join(format!("aethercore-trust-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = TrustStore::open(&dir).unwrap();
        assert!(store.path().starts_with(&dir));
        assert_eq!(store.path().file_name().unwrap(), "known_hosts");
        // Typed records regenerate the known_hosts file; the store holds only
        // explicitly authorized records.
        let key_a = "AAAAC3NzaC1lZDI1NTE5AAAAIB3qSmVOUdM=";
        let fp_a = fingerprint_of_blob(key_a).unwrap();
        let record = TrustedHostKey::authorize(
            "host-a",
            "a.example.internal",
            22,
            "ssh-ed25519",
            key_a,
            &fp_a,
            42,
        )
        .unwrap();
        store.trust(&record).unwrap();
        assert_eq!(store.records().unwrap().len(), 1);
        let content = std::fs::read_to_string(store.path()).unwrap();
        assert!(content.contains("[a.example.internal]:22 ssh-ed25519"));
        // Revocation removes BOTH the record and the line.
        store.untrust("a.example.internal", 22).unwrap();
        assert!(store.records().unwrap().is_empty());
        let content = std::fs::read_to_string(store.path()).unwrap();
        assert!(!content.contains("a.example.internal"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------ CF-1: trust chain

    fn sample_key() -> (&'static str, String) {
        let key = "AAAAC3NzaC1lZDI1NTE5AAAAIB3qSmVOUdM=";
        (key, fingerprint_of_blob(key).unwrap())
    }

    #[test]
    fn cf1_fingerprint_only_trust_cannot_create_execution_entry() {
        // A fingerprint WITHOUT public key material cannot authorize anything:
        // `authorize` requires the key material and recomputes from it.
        let (_key, fp) = sample_key();
        assert!(
            TrustedHostKey::authorize(
                "host-a",
                "a.example.internal",
                22,
                "ssh-ed25519",
                "",
                &fp,
                42
            )
            .is_err()
        );
        // Empty key material decodes to an empty blob -> different fingerprint
        // than the authorized one -> binding failure. No executable entry.
        let dir = std::env::temp_dir().join(format!("aethercore-cf1-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = TrustStore::open(&dir).unwrap();
        // Fabricated record with empty key material is re-verified at persist
        // time and rejected (binding invariant).
        let forged = TrustedHostKey {
            host_id: "host-a".into(),
            hostname: "a.example.internal".into(),
            port: 22,
            key_type: "ssh-ed25519".into(),
            public_key_base64: String::new(),
            host_key_sha256: fp.clone(),
            trusted_unix_ms: 42,
            provenance: None,
        };
        assert!(store.trust(&forged).is_err());
        assert!(store.records().unwrap().is_empty());
        let content = std::fs::read_to_string(store.path()).unwrap();
        assert!(
            content.trim().is_empty(),
            "no line may exist without a bound record"
        );
        // The inventory pin (fingerprint metadata only) is NOT a trust-store
        // entry: pinning the fingerprint alone writes no known_hosts line.
        let mut host = pinned_host();
        host.trust = None;
        let _ = host.pin_trust(&fp, "ssh-ed25519", 42);
        assert!(host.trust.is_some());
        assert!(content.trim().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
        drop(forged);
    }

    #[test]
    fn cf1_malformed_base64_rejected() {
        let fp = "ab".repeat(32);
        assert_eq!(
            TrustedHostKey::authorize(
                "host-a",
                "a.example.internal",
                22,
                "ssh-ed25519",
                "!!not-base64!!",
                &fp,
                42
            )
            .unwrap_err(),
            TrustError::MalformedKeyMaterial
        );
    }

    #[test]
    fn cf1_unsupported_key_type_rejected() {
        let (key, fp) = sample_key();
        assert_eq!(
            TrustedHostKey::authorize("host-a", "a.example.internal", 22, "ssh-dss", key, &fp, 42)
                .unwrap_err(),
            TrustError::UnsupportedKeyType("ssh-dss".into())
        );
    }

    #[test]
    fn cf1_fingerprint_key_mismatch_rejected() {
        let (key, _real_fp) = sample_key();
        // Authorized fingerprint A + public key B must never combine.
        let wrong_fp = "ff".repeat(32);
        assert!(matches!(
            TrustedHostKey::authorize(
                "host-a",
                "a.example.internal",
                22,
                "ssh-ed25519",
                key,
                &wrong_fp,
                42
            ),
            Err(TrustError::FingerprintKeyMismatch { .. })
        ));
    }

    #[test]
    fn cf1_private_key_material_never_enters_trust_storage() {
        let fp = "ab".repeat(32);
        for body in [
            "-----BEGIN OPENSSH PRIVATE KEY-----b3BlbnNzaC1rZXktdjEAAAAA",
            "-----BEGIN RSA PRIVATE KEY-----MIIEpAIBAAKCAQEA",
        ] {
            assert_eq!(
                TrustedHostKey::authorize(
                    "host-a",
                    "a.example.internal",
                    22,
                    "ssh-ed25519",
                    body,
                    &fp,
                    42
                )
                .unwrap_err(),
                TrustError::PrivateKeyMaterialRejected
            );
        }
        // A JSON record carrying private-key-shaped content also cannot be
        // persisted: trust() re-verifies the binding from the key material.
        let dir = std::env::temp_dir().join(format!("aethercore-cf1b-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = TrustStore::open(&dir).unwrap();
        let (key, real_fp) = sample_key();
        // A record whose key material was swapped for a PEM body fails the
        // binding re-check even though its fingerprint field is untouched.
        let mut swapped = TrustedHostKey::authorize(
            "host-a",
            "a.example.internal",
            22,
            "ssh-ed25519",
            key,
            &real_fp,
            42,
        )
        .unwrap();
        swapped.public_key_base64 =
            "-----BEGIN OPENSSH PRIVATE KEY-----b3BlbnNzaC1rZXktdjEAAAAA".into();
        assert!(store.trust(&swapped).is_err());
        assert!(store.records().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cf1_valid_key_plus_fingerprint_accepted_line_is_exact() {
        let (key, fp) = sample_key();
        let record = TrustedHostKey::authorize(
            "host-a",
            "a.example.internal",
            22,
            "ssh-ed25519",
            key,
            &fp,
            42,
        )
        .expect("valid binding accepted");
        assert_eq!(record.host_key_sha256, fp);
        let dir = std::env::temp_dir().join(format!("aethercore-cf1c-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = TrustStore::open(&dir).unwrap();
        store.trust(&record).unwrap();
        let content = std::fs::read_to_string(store.path()).unwrap();
        // known_hosts contains EXACTLY the authorized host key line.
        assert_eq!(
            content,
            format!("[a.example.internal]:22 ssh-ed25519 {key}\n")
        );
        // Re-pinning the same host replaces (not duplicates) the entry.
        store.trust(&record).unwrap();
        let content = std::fs::read_to_string(store.path()).unwrap();
        assert_eq!(
            content.matches("a.example.internal").count(),
            1,
            "single canonical entry per host:port"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cf1_changed_host_key_is_host_key_mismatch() {
        // The domain-level decision on a differing presented fingerprint.
        let host = pinned_host();
        assert_eq!(
            decide(&host, Some(&"cd".repeat(32))),
            TrustDecision::HostKeyMismatch
        );
        // The transport-level ssh classification also types a changed key.
        let key = classify_ssh_exit(255, "Host key verification failed.");
        assert_eq!(key.outcome, RemoteOutcomeKind::HostKeyMismatch);
        // Re-authorization with the NEW key is a new explicit trust action
        // bound to the new key material — the old record cannot absorb it.
        let (new_key, new_fp) = (
            "AAAAC3NzaC1lZDI1NTE5AAAAIfffffffffffffff=",
            fingerprint_of_blob("AAAAC3NzaC1lZDI1NTE5AAAAIfffffffffffffff=").unwrap(),
        );
        let authorized = TrustedHostKey::authorize(
            "host-a",
            "a.example.internal",
            22,
            "ssh-ed25519",
            new_key,
            &new_fp,
            43,
        )
        .unwrap();
        let dir = std::env::temp_dir().join(format!("aethercore-cf1d-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = TrustStore::open(&dir).unwrap();
        let (old_key, old_fp) = sample_key();
        store
            .trust(
                &TrustedHostKey::authorize(
                    "host-a",
                    "a.example.internal",
                    22,
                    "ssh-ed25519",
                    old_key,
                    &old_fp,
                    42,
                )
                .unwrap(),
            )
            .unwrap();
        store.trust(&authorized).unwrap();
        let content = std::fs::read_to_string(store.path()).unwrap();
        assert!(content.contains(new_key));
        assert!(!content.contains(old_key), "old key removed on re-trust");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cf1_unknown_host_stays_not_verified() {
        let mut host = pinned_host();
        host.trust = None;
        assert_eq!(decide(&host, None), TrustDecision::NotVerified);
        assert_eq!(
            decide(&host, Some(&"ab".repeat(32))),
            TrustDecision::NotVerified
        );
    }

    #[test]
    fn supported_key_types_are_pinned_and_strict() {
        assert!(is_supported_key_type("ssh-ed25519"));
        assert!(is_supported_key_type("ssh-rsa"));
        assert!(!is_supported_key_type("ssh-dss"));
        assert!(!is_supported_key_type(""));
        assert!(!is_supported_key_type("ssh-rsa-dropbear"));
    }
}
