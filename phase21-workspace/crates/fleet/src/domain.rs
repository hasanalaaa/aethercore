//! AetherCore Fleet — Phase 34 typed fleet domain model.
//!
//! Strict, versioned schemas (`aethercore.fleet.host.v1` etc.) with
//! `deny_unknown_fields` everywhere. Validation is fail-closed: malformed
//! hostnames, ports, usernames, host IDs, fingerprints, auth methods and
//! duplicate identities are typed rejections. Secrets are structurally
//! impossible: the domain has no field capable of carrying a password,
//! private-key body, passphrase or API key — only a *reference* (path) to an
//! identity file is representable, matching the product's existing policy of
//! storing references, never secret contents.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Stable fleet host identifier type (string; validated by [`FleetHost::new`]).
pub type FleetHostId = str;

pub const FLEET_HOST_SCHEMA: &str = "aethercore.fleet.host.v1";
pub const FLEET_INVENTORY_SCHEMA: &str = "aethercore.fleet.inventory.v1";
pub const FLEET_SCHEDULE_SCHEMA: &str = "aethercore.fleet.schedule.v1";

/// Upper bound on inventory size — conservative by design; a real large-fleet
/// scale qualification is tracked as qualification debt (QD-034-*).
pub const MAX_FLEET_HOSTS: usize = 256;

/// Supported remote authentication references. Passwords are deliberately
/// unrepresentable: `Password` is not a variant. `KeyFile` stores the PATH to
/// an identity file — never its contents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum AuthReference {
    /// Default agent/user key resolution on the controlling host (no stored ref).
    Agent,
    /// Path to a local OpenSSH-compatible private identity file. The file's
    /// contents are never read into the domain nor persisted.
    KeyFile { path: String },
    /// Explicit local certificate identity file path (enterprise CA setups).
    Certificate { path: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FleetHostTrust {
    /// Full SHA-256 fingerprint of the remote host key, hex-encoded lowercase.
    pub host_key_sha256: String,
    /// RFC 4251-ish key type ("ssh-ed25519", "rsa-sha2-256", ...).
    pub key_type: String,
    /// Unix ms when the pin was recorded by an explicit user trust action.
    pub trusted_unix_ms: i64,
    /// Free-form provenance note (e.g. "pinned from first-connect review").
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FleetHost {
    pub schema: String,
    /// Stable host identifier: `[a-z0-9][a-z0-9._-]{1,63}` (lowercase).
    pub host_id: String,
    pub display_name: String,
    /// hostname or IP literal. Validated; no spaces, no scheme, no user@ part.
    pub hostname: String,
    pub port: u16,
    pub username: String,
    pub auth: AuthReference,
    /// Present only after an explicit user trust action (no silent TOFU).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust: Option<FleetHostTrust>,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub tags: BTreeSet<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FleetInventory {
    pub schema: String,
    /// Deterministic ordering (sorted by host_id) is a structural invariant.
    pub hosts: Vec<FleetHost>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FleetDomainError {
    #[error("malformed host id: {0}")]
    MalformedHostId(String),
    #[error("malformed hostname: {0}")]
    MalformedHostname(String),
    #[error("malformed username: {0}")]
    MalformedUsername(String),
    #[error("malformed port: {0}")]
    MalformedPort(String),
    #[error("malformed fingerprint: {0}")]
    MalformedFingerprint(String),
    #[error("malformed display name: {0}")]
    MalformedDisplayName(String),
    #[error("malformed identity path: {0}")]
    MalformedIdentityPath(String),
    #[error("duplicate host id: {0}")]
    DuplicateHostId(String),
    #[error("duplicate identity username@hostname:port: {0}")]
    DuplicateIdentity(String),
    #[error("schema mismatch: expected {expected}, got {got}")]
    SchemaMismatch { expected: String, got: String },
    #[error("fleet inventory full: {0} hosts is the configured maximum")]
    InventoryFull(usize),
    #[error("malformed inventory: {0}")]
    MalformedInventory(String),
    #[error("malformed schedule: {0}")]
    MalformedSchedule(String),
    #[error("duplicate schedule id: {0}")]
    DuplicateScheduleId(String),
}

fn valid_host_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.len() < 2 || bytes.len() > 64 {
        return false;
    }
    let first = bytes[0];
    if !(first.is_ascii_digit() || first.is_ascii_lowercase()) {
        return false;
    }
    bytes[1..].iter().all(|b| {
        b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'.' || *b == b'_' || *b == b'-'
    })
}

fn valid_hostname(host: &str) -> bool {
    // Reject the dangerous shapes first: no scheme, no user@, no spaces,
    // no shell metacharacters, no option-leading value.
    if host.is_empty()
        || host.len() > 253
        || host.contains("://")
        || host.contains('@')
        || host.contains(' ')
        || host.contains('\t')
        || host.contains('\n')
        || host.contains(';')
        || host.contains('&')
        || host.contains('|')
        || host.contains('$')
        || host.contains('`')
        || host.contains('"')
        || host.contains('\'')
        || host.contains('\\')
        || host.starts_with('-')
        || host.starts_with('.')
    {
        return false;
    }
    // Accept IP literals and DNS labels; each label must be non-empty and
    // alphanumeric/hyphen, not starting/ending with a hyphen.
    host.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    })
}

fn valid_username(user: &str) -> bool {
    !user.is_empty()
        && user.len() <= 64
        && user
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
        && !user.starts_with('-')
}

fn valid_fingerprint(fp: &str) -> bool {
    fp.len() == 64
        && fp
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn valid_identity_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\0')
        && !path.contains('\n')
        && path != "/dev/null"
        // A path is a REFERENCE, never key material: reject PEM/secret bodies
        // that do not name a file.
        && !path.to_ascii_lowercase().contains("-----begin")
}

impl FleetHost {
    /// Validates and constructs a host. Strict field validation; trust must
    /// itself be well-formed if present.
    pub fn new(
        host_id: &str,
        display_name: &str,
        hostname: &str,
        port: u16,
        username: &str,
        auth: AuthReference,
        tags: BTreeSet<String>,
    ) -> Result<Self, FleetDomainError> {
        if !valid_host_id(host_id) {
            return Err(FleetDomainError::MalformedHostId(host_id.to_string()));
        }
        if display_name.trim().is_empty() || display_name.len() > 128 {
            return Err(FleetDomainError::MalformedDisplayName(
                display_name.to_string(),
            ));
        }
        if !valid_hostname(hostname) {
            return Err(FleetDomainError::MalformedHostname(hostname.to_string()));
        }
        if port == 0 {
            return Err(FleetDomainError::MalformedPort(port.to_string()));
        }
        if !valid_username(username) {
            return Err(FleetDomainError::MalformedUsername(username.to_string()));
        }
        if let AuthReference::KeyFile { path } | AuthReference::Certificate { path } = &auth
            && !valid_identity_path(path)
        {
            return Err(FleetDomainError::MalformedIdentityPath(path.clone()));
        }
        for tag in &tags {
            if tag.trim().is_empty() || tag.len() > 32 {
                return Err(FleetDomainError::MalformedDisplayName(format!(
                    "tag {tag:?}"
                )));
            }
        }
        Ok(Self {
            schema: FLEET_HOST_SCHEMA.to_string(),
            host_id: host_id.to_string(),
            display_name: display_name.trim().to_string(),
            hostname: hostname.to_string(),
            port,
            username: username.to_string(),
            auth,
            trust: None,
            enabled: true,
            tags,
        })
    }

    pub fn identity_key(&self) -> String {
        format!("{}@{}:{}", self.username, self.hostname, self.port)
    }

    /// Attach a trust pin — the ONLY path to a trusted state, and it must be
    /// an explicit, well-formed fingerprint. This is the "user-authorized
    /// action" gate: nothing in the codebase may call this from discovery.
    pub fn pin_trust(
        &mut self,
        host_key_sha256: &str,
        key_type: &str,
        trusted_unix_ms: i64,
    ) -> Result<(), FleetDomainError> {
        if !valid_fingerprint(host_key_sha256) {
            return Err(FleetDomainError::MalformedFingerprint(
                host_key_sha256.to_string(),
            ));
        }
        if key_type.trim().is_empty() || key_type.len() > 64 {
            return Err(FleetDomainError::MalformedFingerprint(format!(
                "key type {key_type:?}"
            )));
        }
        self.trust = Some(FleetHostTrust {
            host_key_sha256: host_key_sha256.to_string(),
            key_type: key_type.trim().to_string(),
            trusted_unix_ms,
            note: None,
        });
        Ok(())
    }

    /// Strict deserialization from JSON bytes (unknown fields rejected).
    pub fn from_bytes(raw: &[u8]) -> Result<Self, FleetDomainError> {
        let host: Self = serde_json::from_slice(raw)
            .map_err(|error| FleetDomainError::MalformedInventory(error.to_string()))?;
        if host.schema != FLEET_HOST_SCHEMA {
            return Err(FleetDomainError::SchemaMismatch {
                expected: FLEET_HOST_SCHEMA.to_string(),
                got: host.schema,
            });
        }
        // Re-validate every field after deserialization — a hand-edited JSON
        // file cannot smuggle a malformed value past the constructor.
        let validated = Self::new(
            &host.host_id,
            &host.display_name,
            &host.hostname,
            host.port,
            &host.username,
            host.auth.clone(),
            host.tags.clone(),
        )?;
        let mut out = validated;
        out.enabled = host.enabled;
        out.trust = host.trust;
        if let Some(trust) = &out.trust
            && !valid_fingerprint(&trust.host_key_sha256)
        {
            return Err(FleetDomainError::MalformedFingerprint(
                trust.host_key_sha256.clone(),
            ));
        }
        Ok(out)
    }
}

impl FleetInventory {
    pub fn new() -> Self {
        Self {
            schema: FLEET_INVENTORY_SCHEMA.to_string(),
            hosts: Vec::new(),
        }
    }

    /// Adds a host; rejects duplicate host IDs and duplicate identities.
    /// Keeps hosts deterministically sorted by host_id.
    pub fn add(&mut self, host: FleetHost) -> Result<(), FleetDomainError> {
        if self.hosts.len() >= MAX_FLEET_HOSTS {
            return Err(FleetDomainError::InventoryFull(MAX_FLEET_HOSTS));
        }
        if self.hosts.iter().any(|h| h.host_id == host.host_id) {
            return Err(FleetDomainError::DuplicateHostId(host.host_id));
        }
        let identity = host.identity_key();
        if self.hosts.iter().any(|h| h.identity_key() == identity) {
            return Err(FleetDomainError::DuplicateIdentity(identity));
        }
        self.hosts.push(host);
        self.hosts.sort_by(|a, b| a.host_id.cmp(&b.host_id));
        Ok(())
    }

    pub fn remove(&mut self, host_id: &str) -> Result<(), FleetDomainError> {
        let before = self.hosts.len();
        self.hosts.retain(|h| h.host_id != host_id);
        if self.hosts.len() == before {
            return Err(FleetDomainError::MalformedInventory(format!(
                "unknown host id {host_id:?}"
            )));
        }
        Ok(())
    }

    pub fn get(&self, host_id: &str) -> Option<&FleetHost> {
        self.hosts.iter().find(|h| h.host_id == host_id)
    }

    pub fn get_mut(&mut self, host_id: &str) -> Option<&mut FleetHost> {
        self.hosts.iter_mut().find(|h| h.host_id == host_id)
    }

    pub fn hosts_matching(&self, tag: &str) -> Vec<&FleetHost> {
        self.hosts.iter().filter(|h| h.tags.contains(tag)).collect()
    }

    /// Strict deserialization (unknown fields rejected, deterministic order
    /// re-asserted, duplicates rejected).
    pub fn from_bytes(raw: &[u8]) -> Result<Self, FleetDomainError> {
        let mut inventory: Self = serde_json::from_slice(raw)
            .map_err(|error| FleetDomainError::MalformedInventory(error.to_string()))?;
        if inventory.schema != FLEET_INVENTORY_SCHEMA {
            return Err(FleetDomainError::SchemaMismatch {
                expected: FLEET_INVENTORY_SCHEMA.to_string(),
                got: inventory.schema,
            });
        }
        if inventory.hosts.len() > MAX_FLEET_HOSTS {
            return Err(FleetDomainError::InventoryFull(inventory.hosts.len()));
        }
        let mut seen_ids = BTreeSet::new();
        let mut seen_identities = BTreeSet::new();
        for host in &inventory.hosts {
            if !seen_ids.insert(host.host_id.clone()) {
                return Err(FleetDomainError::DuplicateHostId(host.host_id.clone()));
            }
            if !seen_identities.insert(host.identity_key()) {
                return Err(FleetDomainError::DuplicateIdentity(host.identity_key()));
            }
        }
        inventory.hosts.sort_by(|a, b| a.host_id.cmp(&b.host_id));
        Ok(inventory)
    }

    /// Parses a single host's strict JSON into a one-host inventory
    /// (round-trip helper used by persistence read-back).
    pub fn from_bytes_raw(raw: &[u8]) -> Result<Self, FleetDomainError> {
        let trimmed = raw;
        if let Ok(host) = FleetHost::from_bytes(trimmed) {
            let mut inventory = Self::new();
            inventory.add(host)?;
            Ok(inventory)
        } else {
            Self::from_bytes(trimmed)
        }
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, FleetDomainError> {
        serde_json::to_vec_pretty(self)
            .map_err(|error| FleetDomainError::MalformedInventory(error.to_string()))
    }
}

/// Compliance cadence. Enum variants are conservative — there is no
/// "as-fast-as-possible" mode and the minimum period is bounded well above
/// tight-loop territory (see MIN_CADENCE_SECS).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub enum FleetCadence {
    /// Every N hours. Minimum enforced at construction.
    EveryHours(u32),
    /// Daily at a fixed UTC hour (0..=23).
    DailyAtUtcHour(u8),
}

/// Minimum allowed cadence period in seconds (1 hour) — a structural
/// no-tight-loop bound.
pub const MIN_CADENCE_SECS: u64 = 3600;

impl FleetCadence {
    pub fn period_secs(&self) -> Result<u64, FleetDomainError> {
        match self {
            FleetCadence::EveryHours(hours) => {
                let secs = u64::from(*hours) * 3600;
                if secs < MIN_CADENCE_SECS {
                    return Err(FleetDomainError::MalformedSchedule(format!(
                        "cadence {hours}h is below the 1-hour floor"
                    )));
                }
                Ok(secs)
            }
            FleetCadence::DailyAtUtcHour(hour) => {
                if *hour > 23 {
                    return Err(FleetDomainError::MalformedSchedule(format!(
                        "daily hour {hour} out of range 0..=23"
                    )));
                }
                Ok(24 * 3600)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FleetSchedule {
    pub schema: String,
    pub schedule_id: String,
    /// Host IDs or `group:<tag>` selectors, deterministic order.
    pub scope: Vec<String>,
    pub profile_id: String,
    pub enabled: bool,
    pub cadence: FleetCadence,
    /// Unix ms of the next scheduled run (maintained by the scheduler).
    pub next_run_unix_ms: i64,
    /// Metadata from the most recent completed run (never erased by failure).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_result: Option<FleetScheduleLastResult>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FleetScheduleLastResult {
    pub finished_unix_ms: i64,
    pub hosts_attempted: u32,
    pub hosts_ok: u32,
    pub hosts_failed: u32,
    pub outcome_summary: String,
}

impl FleetSchedule {
    pub fn new(
        schedule_id: &str,
        scope: Vec<String>,
        profile_id: &str,
        cadence: FleetCadence,
        now_unix_ms: i64,
    ) -> Result<Self, FleetDomainError> {
        if !valid_host_id(schedule_id) {
            return Err(FleetDomainError::MalformedSchedule(format!(
                "schedule id {schedule_id:?}"
            )));
        }
        if scope.is_empty() {
            return Err(FleetDomainError::MalformedSchedule(
                "schedule scope must be non-empty".to_string(),
            ));
        }
        if !matches!(profile_id, "cis-l1" | "cis-l2") {
            return Err(FleetDomainError::MalformedSchedule(format!(
                "unknown profile {profile_id:?}"
            )));
        }
        cadence.period_secs()?;
        let period_ms = (cadence.period_secs()? * 1000) as i64;
        Ok(Self {
            schema: FLEET_SCHEDULE_SCHEMA.to_string(),
            schedule_id: schedule_id.to_string(),
            scope,
            profile_id: profile_id.to_string(),
            enabled: true,
            cadence,
            next_run_unix_ms: now_unix_ms + period_ms,
            last_result: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_host(id: &str) -> FleetHost {
        let hostname = format!("{id}.example.internal");
        FleetHost::new(
            id,
            "Build host",
            &hostname,
            22,
            "ops",
            AuthReference::Agent,
            BTreeSet::new(),
        )
        .expect("valid host")
    }

    #[test]
    fn host_validation_accepts_well_formed() {
        let host = base_host("host-build-01");
        assert_eq!(host.schema, FLEET_HOST_SCHEMA);
        assert!(host.trust.is_none());
        assert!(host.enabled);
    }

    #[test]
    fn host_validation_rejects_malformed() {
        assert!(
            FleetHost::new(
                "Bad ID!",
                "n",
                "h.example",
                22,
                "u",
                AuthReference::Agent,
                BTreeSet::new()
            )
            .is_err()
        );
        assert!(
            FleetHost::new(
                "ok-id",
                "n",
                "bad host",
                22,
                "u",
                AuthReference::Agent,
                BTreeSet::new()
            )
            .is_err()
        );
        assert!(
            FleetHost::new(
                "ok-id",
                "n",
                "host",
                22,
                "-bad user",
                AuthReference::Agent,
                BTreeSet::new()
            )
            .is_err()
        );
        assert!(
            FleetHost::new(
                "ok-id",
                "n",
                "host",
                0,
                "u",
                AuthReference::Agent,
                BTreeSet::new()
            )
            .is_err()
        );
        assert!(
            FleetHost::new(
                "ok-id",
                "  ",
                "host",
                22,
                "u",
                AuthReference::Agent,
                BTreeSet::new()
            )
            .is_err()
        );
        assert!(
            FleetHost::new(
                "ok-id",
                "n",
                "host",
                22,
                "u",
                AuthReference::KeyFile {
                    path: "/dev/null".into()
                },
                BTreeSet::new()
            )
            .is_err()
        );
        // shell metacharacter in hostname is rejected
        assert!(
            FleetHost::new(
                "ok-id",
                "n",
                "host;rm -rf",
                22,
                "u",
                AuthReference::Agent,
                BTreeSet::new()
            )
            .is_err()
        );
        assert!(
            FleetHost::new(
                "ok-id",
                "n",
                "$(echo pwn)",
                22,
                "u",
                AuthReference::Agent,
                BTreeSet::new()
            )
            .is_err()
        );
        assert!(
            FleetHost::new(
                "ok-id",
                "n",
                "user@host",
                22,
                "u",
                AuthReference::Agent,
                BTreeSet::new()
            )
            .is_err()
        );
        assert!(
            FleetHost::new(
                "ok-id",
                "n",
                "-oProxyCommand=evil",
                22,
                "u",
                AuthReference::Agent,
                BTreeSet::new()
            )
            .is_err()
        );
    }

    #[test]
    fn strict_deserialization_rejects_unknown_fields() {
        let host = base_host("host-a");
        let mut json = serde_json::to_value(&host).unwrap();
        json["sneaky_extra"] = serde_json::json!("boom");
        assert!(FleetHost::from_bytes(serde_json::to_vec(&json).unwrap().as_slice()).is_err());
        // malformed hostname in hand-edited file rejected on re-validation
        let mut json2 = serde_json::to_value(base_host("host-a")).unwrap();
        json2["hostname"] = serde_json::json!("bad host;rm");
        assert!(FleetHost::from_bytes(serde_json::to_vec(&json2).unwrap().as_slice()).is_err());
        // schema mismatch rejected
        let mut json3 = serde_json::to_value(base_host("host-a")).unwrap();
        json3["schema"] = serde_json::json!("aethercore.fleet.host.v9");
        assert!(FleetHost::from_bytes(serde_json::to_vec(&json3).unwrap().as_slice()).is_err());
    }

    #[test]
    fn inventory_rejects_duplicates_and_orders_deterministically() {
        let mut inv = FleetInventory::new();
        inv.add(base_host("zebra")).unwrap();
        inv.add(base_host("alpha")).unwrap();
        assert_eq!(
            inv.hosts
                .iter()
                .map(|h| h.host_id.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha", "zebra"]
        );
        // duplicate host id
        assert_eq!(
            inv.add(base_host("alpha")),
            Err(FleetDomainError::DuplicateHostId("alpha".into()))
        );
        // duplicate identity, different id
        let mut twin = base_host("bravo");
        twin.hostname = "alpha.example.internal".into();
        twin.username = "ops".into();
        twin.port = 22;
        assert!(matches!(
            inv.add(twin),
            Err(FleetDomainError::DuplicateIdentity(_))
        ));
        assert_eq!(
            FleetInventory::from_bytes(&inv.to_bytes().unwrap()).unwrap(),
            inv
        );
    }

    #[test]
    fn trust_pinning_is_explicit_and_strict() {
        let mut host = base_host("host-a");
        assert!(host.trust.is_none());
        assert!(
            host.pin_trust("not-a-fingerprint", "ssh-ed25519", 0)
                .is_err()
        );
        host.pin_trust(&"a".repeat(64), "ssh-ed25519", 1_700_000_000_000)
            .unwrap();
        assert!(host.trust.is_some());
        assert!(host.pin_trust("zz", "ssh-ed25519", 0).is_err());
    }

    #[test]
    fn schedule_json_uses_desktop_camel_case_contract() {
        let value = serde_json::json!({
            "schema": FLEET_SCHEDULE_SCHEMA,
            "scheduleId": "sched-json",
            "scope": ["host-a"],
            "profileId": "cis-l2",
            "enabled": true,
            "cadence": {"everyHours": 4},
            "nextRunUnixMs": 0
        });
        let schedule: FleetSchedule = serde_json::from_value(value).expect("typed schedule");
        assert_eq!(schedule.schedule_id, "sched-json");
        assert_eq!(schedule.profile_id, "cis-l2");
        assert_eq!(schedule.next_run_unix_ms, 0);
    }

    #[test]
    fn no_password_variant_exists() {
        // Structural secret-ban proof: serde of an auth containing a password
        // variant fails, and the enum literally has no password shape.
        let raw = br#"{"agent":{"password":"hunter2"}}"#;
        assert!(serde_json::from_slice::<AuthReference>(raw).is_err());
        let raw2 = br#"{"password":"hunter2"}"#;
        assert!(serde_json::from_slice::<AuthReference>(raw2).is_err());
    }

    #[test]
    fn cadence_bounds_are_enforced() {
        assert!(FleetCadence::EveryHours(0).period_secs().is_err());
        assert_eq!(FleetCadence::EveryHours(2).period_secs().unwrap(), 7200);
        assert!(FleetCadence::DailyAtUtcHour(24).period_secs().is_err());
        assert_eq!(
            FleetCadence::DailyAtUtcHour(3).period_secs().unwrap(),
            86400
        );
        assert!(
            FleetSchedule::new(
                "sched-1",
                vec!["group:edge".into()],
                "cis-l1",
                FleetCadence::EveryHours(0),
                0
            )
            .is_err()
        );
        assert!(
            FleetSchedule::new("sched-1", vec![], "cis-l1", FleetCadence::EveryHours(2), 0)
                .is_err()
        );
        assert!(
            FleetSchedule::new(
                "sched-1",
                vec!["group:x".into()],
                "cis-l3",
                FleetCadence::EveryHours(2),
                0
            )
            .is_err()
        );
        let sched = FleetSchedule::new(
            "sched-1",
            vec!["group:x".into()],
            "cis-l1",
            FleetCadence::EveryHours(2),
            1000,
        )
        .unwrap();
        assert_eq!(sched.next_run_unix_ms, 1000 + 7200 * 1000);
    }
}
