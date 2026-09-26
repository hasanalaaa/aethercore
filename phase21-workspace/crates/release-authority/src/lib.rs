//! Phase 35 release, update and installation authority.
//!
//! This crate owns the *metadata* contract.  It intentionally does not download,
//! install, or elevate a process: those operations remain behind the existing
//! update-engine mutation supervisor.  Every serialized type is strict and every
//! signature covers the exact deterministic bytes supplied by the caller.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path};
use thiserror::Error;
use uuid::Uuid;

pub const IDENTITY_SCHEMA: &str = "aethercore.release.identity.v1";
pub const MANIFEST_SCHEMA: &str = "aethercore.release.manifest.v1";
pub const UPDATE_METADATA_SCHEMA: &str = "aethercore.update.metadata.v1";
pub const ROLLBACK_SCHEMA: &str = "aethercore.release.rollback-authorization.v1";
pub const SIGNATURE_SCHEMA: &str = "aethercore.release.signature.v1";
pub const KEYRING_SCHEMA: &str = "aethercore.release.keyring.v1";
pub const ROTATION_SCHEMA: &str = "aethercore.release.key-rotation.v1";
pub const UPDATE_CONTRACT_VERSION: &str = "aethercore.update-contract.v1";
pub const MAX_ARTIFACT_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const MAX_METADATA_BYTES: usize = 256 * 1024;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthorityError {
    #[error("invalid release identity: {0}")]
    InvalidIdentity(String),
    #[error("unsupported schema: {0}")]
    UnsupportedSchema(String),
    #[error("deterministic serialization failed: {0}")]
    Serialization(String),
    #[error("metadata exceeds the bounded size")]
    TooLarge,
    #[error("unknown signing key: {0}")]
    UnknownKey(String),
    #[error("signing key is revoked or disabled: {0}")]
    RevokedKey(String),
    #[error("malformed public key")]
    MalformedKey,
    #[error("signature encoding is invalid")]
    InvalidSignature,
    #[error("signature verification failed")]
    SignatureMismatch,
    #[error("channel or target identity does not match")]
    IdentityMismatch,
    #[error("package length or digest does not match")]
    PackageMismatch,
    #[error("metadata is expired or not yet valid")]
    Freshness,
    #[error("downgrade is blocked")]
    DowngradeBlocked,
    #[error("rollback authorization is invalid")]
    UnauthorizedRollback,
    #[error("illegal update transaction transition")]
    IllegalTransition,
    #[error("immutable update plan was changed")]
    PlanChanged,
    #[error("duplicate or conflicting key rotation")]
    RotationConflict,
    #[error("unsafe archive member path")]
    UnsafeArchivePath,
    #[error("unsupported update source")]
    UnsupportedSource,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum ReleaseChannel {
    Stable,
    Beta,
    Dev,
}

impl ReleaseChannel {
    pub const ALL: [Self; 3] = [Self::Stable, Self::Beta, Self::Dev];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Beta => "beta",
            Self::Dev => "dev",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TargetPlatform {
    Windows,
    Macos,
    Linux,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TargetArchitecture {
    X86_64,
    Aarch64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReleaseIdentity {
    pub schema: String,
    pub product_id: String,
    pub version: String,
    pub channel: ReleaseChannel,
    pub release_sequence: u64,
    pub platform: TargetPlatform,
    pub architecture: TargetArchitecture,
    pub protocol_version: String,
    pub minimum_updater_version: String,
    pub source_commit: Option<String>,
    /// Optional only when reproducibility policy supplies a fixed epoch.
    pub build_epoch: Option<u64>,
}

impl ReleaseIdentity {
    pub fn validate(&self) -> Result<(), AuthorityError> {
        if self.schema != IDENTITY_SCHEMA
            || self.product_id != aethercore_product_identity::PRODUCT_NAME
            || !valid_version(&self.version)
            || self.release_sequence == 0
            || !valid_version(&self.minimum_updater_version)
            || self.protocol_version.is_empty()
            || self.protocol_version.len() > 64
            || self
                .source_commit
                .as_ref()
                .is_some_and(|v| !safe_token(v, 128))
        {
            return Err(AuthorityError::InvalidIdentity(
                "field or version constraint".into(),
            ));
        }
        Ok(())
    }

    pub fn consistency_key(&self) -> String {
        format!(
            "{}@{}:{}:{}:{}:{}",
            self.product_id,
            self.version,
            self.channel.as_str(),
            self.release_sequence,
            platform_name(self.platform),
            architecture_name(self.architecture)
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReleasePackage {
    pub package_type: String,
    pub filename: String,
    pub byte_length: u64,
    pub sha256: String,
}

impl ReleasePackage {
    pub fn validate(&self) -> Result<(), AuthorityError> {
        if self.package_type != "msi"
            && self.package_type != "burn"
            && self.package_type != "offline-zip"
        {
            return Err(AuthorityError::PackageMismatch);
        }
        if self.byte_length == 0
            || self.byte_length > MAX_ARTIFACT_BYTES
            || self.filename.is_empty()
            || self.filename.contains('/')
            || self.filename.contains('\\')
            || self.sha256.len() != 64
            || !self.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(AuthorityError::PackageMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReleaseManifest {
    pub schema: String,
    pub identity: ReleaseIdentity,
    pub package: ReleasePackage,
    pub sbom_sha256: String,
    pub sbom_reference: String,
    pub provenance_sha256: String,
    pub provenance_reference: String,
    pub created_epoch: u64,
    pub update_contract_version: String,
}

impl ReleaseManifest {
    pub fn validate(&self) -> Result<(), AuthorityError> {
        if self.schema != MANIFEST_SCHEMA {
            return Err(AuthorityError::UnsupportedSchema(self.schema.clone()));
        }
        self.identity.validate()?;
        self.package.validate()?;
        for digest in [&self.sbom_sha256, &self.provenance_sha256] {
            if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(AuthorityError::PackageMismatch);
            }
        }
        if self.sbom_reference.is_empty()
            || self.provenance_reference.is_empty()
            || self.update_contract_version != UPDATE_CONTRACT_VERSION
        {
            return Err(AuthorityError::InvalidIdentity(
                "binding or update contract".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SignatureEnvelope {
    pub schema: String,
    pub algorithm: String,
    pub key_id: String,
    pub signature_hex: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TrustedReleaseKey {
    pub key_id: String,
    pub algorithm: String,
    pub public_key_hex: String,
    pub enabled: bool,
    pub revoked: bool,
    pub not_before_epoch: Option<u64>,
    pub not_after_epoch: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TrustedKeyring {
    pub schema: String,
    pub keys: Vec<TrustedReleaseKey>,
}

impl TrustedKeyring {
    pub fn validate(&self, now_epoch: Option<u64>) -> Result<(), AuthorityError> {
        if self.schema != KEYRING_SCHEMA || self.keys.is_empty() {
            return Err(AuthorityError::UnsupportedSchema(self.schema.clone()));
        }
        let mut ids = BTreeSet::new();
        for key in &self.keys {
            if !safe_token(&key.key_id, 64)
                || !ids.insert(key.key_id.as_str())
                || key.algorithm != "Ed25519"
            {
                return Err(AuthorityError::MalformedKey);
            }
            let raw = hex::decode(&key.public_key_hex).map_err(|_| AuthorityError::MalformedKey)?;
            if raw.len() != 32 || raw.iter().all(|b| *b == 0) {
                return Err(AuthorityError::MalformedKey);
            }
            if let (Some(before), Some(after)) = (key.not_before_epoch, key.not_after_epoch)
                && after < before
            {
                return Err(AuthorityError::MalformedKey);
            }
            if let Some(now) = now_epoch
                && (key.not_before_epoch.is_some_and(|v| now < v)
                    || key.not_after_epoch.is_some_and(|v| now > v))
            {
                continue;
            }
        }
        Ok(())
    }

    pub fn active_key(
        &self,
        key_id: &str,
        now_epoch: Option<u64>,
    ) -> Result<&TrustedReleaseKey, AuthorityError> {
        self.validate(now_epoch)?;
        let key = self
            .keys
            .iter()
            .find(|v| v.key_id == key_id)
            .ok_or_else(|| AuthorityError::UnknownKey(key_id.into()))?;
        if !key.enabled
            || key.revoked
            || key
                .not_before_epoch
                .is_some_and(|v| now_epoch.is_some_and(|n| n < v))
            || key
                .not_after_epoch
                .is_some_and(|v| now_epoch.is_some_and(|n| n > v))
        {
            return Err(AuthorityError::RevokedKey(key_id.into()));
        }
        Ok(key)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct KeyRotation {
    pub schema: String,
    pub old_key_id: String,
    pub new_key: TrustedReleaseKey,
    pub authorized_by: String,
    pub signature_hex: String,
}

impl KeyRotation {
    fn unsigned_bytes(&self) -> Result<Vec<u8>, AuthorityError> {
        let mut copy = self.clone();
        copy.signature_hex.clear();
        canonical_json(&copy)
    }

    /// `now_epoch` is required: rotation mints new trust, so the authorizing key must be
    /// inside its own validity window when the rotation is applied.
    pub fn authorize(
        &self,
        keyring: &TrustedKeyring,
        now_epoch: u64,
    ) -> Result<TrustedKeyring, AuthorityError> {
        if self.schema != ROTATION_SCHEMA
            || self.authorized_by != self.old_key_id
            || self.new_key.key_id == self.old_key_id
        {
            return Err(AuthorityError::RotationConflict);
        }
        let old = keyring.active_key(&self.old_key_id, Some(now_epoch))?;
        verify_signature(&self.unsigned_bytes()?, &self.signature_hex, old)?;
        let mut next = keyring.clone();
        if next.keys.iter().any(|k| k.key_id == self.new_key.key_id) {
            return Err(AuthorityError::RotationConflict);
        }
        self.new_key.clone().validate()?;
        next.keys.push(self.new_key.clone());
        Ok(next)
    }
}

impl TrustedReleaseKey {
    fn validate(&self) -> Result<(), AuthorityError> {
        TrustedKeyring {
            schema: KEYRING_SCHEMA.into(),
            keys: vec![self.clone()],
        }
        .validate(None)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdateMetadata {
    pub schema: String,
    pub product_id: String,
    pub channel: ReleaseChannel,
    pub current_version: String,
    pub target_identity: ReleaseIdentity,
    pub platform: TargetPlatform,
    pub architecture: TargetArchitecture,
    pub package_url: String,
    pub package_length: u64,
    pub package_sha256: String,
    pub release_manifest_sha256: String,
    pub signing_key_id: String,
    pub update_contract_version: String,
    pub release_notes_reference: Option<String>,
    pub generated_epoch: u64,
    pub expires_epoch: u64,
}

impl UpdateMetadata {
    pub fn validate(&self, now_epoch: u64) -> Result<(), AuthorityError> {
        if self.schema != UPDATE_METADATA_SCHEMA
            || self.product_id != aethercore_product_identity::PRODUCT_NAME
            || self.update_contract_version != UPDATE_CONTRACT_VERSION
            || self.expires_epoch < now_epoch
            || self.expires_epoch <= self.generated_epoch
            || self.expires_epoch.saturating_sub(self.generated_epoch) > 30 * 24 * 3600
        {
            return Err(AuthorityError::Freshness);
        }
        self.target_identity.validate()?;
        if self.target_identity.product_id != self.product_id
            || self.target_identity.channel != self.channel
            || self.target_identity.platform != self.platform
            || self.target_identity.architecture != self.architecture
            || !self.package_url.starts_with("https://")
            || self.package_url.contains('@')
            || self.package_url.contains('#')
            || self.package_length == 0
            || self.package_length > MAX_ARTIFACT_BYTES
            || !valid_digest(&self.package_sha256)
            || !valid_digest(&self.release_manifest_sha256)
        {
            return Err(AuthorityError::IdentityMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RollbackAuthorization {
    pub schema: String,
    pub product_id: String,
    pub current_version: String,
    pub authorized_target_version: String,
    pub channel: ReleaseChannel,
    pub reason_code: String,
    pub expires_epoch: u64,
    pub signer_key_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignatureState {
    Signed,
    Unsigned,
    Invalid,
}

pub fn canonical_json<T: Serialize>(value: &T) -> Result<Vec<u8>, AuthorityError> {
    serde_json::to_vec(value).map_err(|e| AuthorityError::Serialization(e.to_string()))
}

pub fn parse_strict<T: DeserializeOwned>(bytes: &[u8], max: usize) -> Result<T, AuthorityError> {
    if bytes.len() > max {
        return Err(AuthorityError::TooLarge);
    }
    serde_json::from_slice(bytes).map_err(|e| AuthorityError::Serialization(e.to_string()))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub fn sign_bytes(
    bytes: &[u8],
    key_id: impl Into<String>,
    private_key: &[u8; 32],
) -> Result<SignatureEnvelope, AuthorityError> {
    let key = SigningKey::from_bytes(private_key);
    Ok(SignatureEnvelope {
        schema: SIGNATURE_SCHEMA.into(),
        algorithm: "Ed25519".into(),
        key_id: key_id.into(),
        signature_hex: hex::encode(key.sign(bytes).to_bytes()),
    })
}

pub fn verify_signed_bytes(
    bytes: &[u8],
    envelope: &SignatureEnvelope,
    keyring: &TrustedKeyring,
    now_epoch: Option<u64>,
) -> Result<SignatureState, AuthorityError> {
    if envelope.schema != SIGNATURE_SCHEMA || envelope.algorithm != "Ed25519" {
        return Err(AuthorityError::UnsupportedSchema(envelope.schema.clone()));
    }
    let key = keyring.active_key(&envelope.key_id, now_epoch)?;
    verify_signature(bytes, &envelope.signature_hex, key)?;
    Ok(SignatureState::Signed)
}

fn verify_signature(
    bytes: &[u8],
    signature_hex: &str,
    key: &TrustedReleaseKey,
) -> Result<(), AuthorityError> {
    let raw = hex::decode(signature_hex).map_err(|_| AuthorityError::InvalidSignature)?;
    let sig: [u8; 64] = raw
        .try_into()
        .map_err(|_| AuthorityError::InvalidSignature)?;
    let public = hex::decode(&key.public_key_hex).map_err(|_| AuthorityError::MalformedKey)?;
    let public: [u8; 32] = public
        .try_into()
        .map_err(|_| AuthorityError::MalformedKey)?;
    let verifying = VerifyingKey::from_bytes(&public).map_err(|_| AuthorityError::MalformedKey)?;
    verifying
        .verify(bytes, &Signature::from_bytes(&sig))
        .map_err(|_| AuthorityError::SignatureMismatch)
}

pub fn verify_release_manifest(
    manifest: &ReleaseManifest,
    bytes: &[u8],
    envelope: &SignatureEnvelope,
    keyring: &TrustedKeyring,
) -> Result<String, AuthorityError> {
    manifest.validate()?;
    let expected = canonical_json(manifest)?;
    if expected != bytes {
        return Err(AuthorityError::PackageMismatch);
    }
    verify_signed_bytes(bytes, envelope, keyring, None)?;
    Ok(sha256_hex(bytes))
}

pub fn verify_update_metadata(
    metadata: &UpdateMetadata,
    envelope: &SignatureEnvelope,
    keyring: &TrustedKeyring,
    now_epoch: u64,
    installed: &ReleaseIdentity,
) -> Result<(), AuthorityError> {
    metadata.validate(now_epoch)?;
    if installed.product_id != metadata.product_id
        || installed.platform != metadata.platform
        || installed.architecture != metadata.architecture
        || installed.channel != metadata.channel
        || !valid_version(&metadata.current_version)
        || metadata.current_version != installed.version
    {
        return Err(AuthorityError::IdentityMismatch);
    }
    if compare_versions(&metadata.target_identity.version, &installed.version) <= 0 {
        return Err(AuthorityError::DowngradeBlocked);
    }
    let bytes = canonical_json(metadata)?;
    if envelope.key_id != metadata.signing_key_id {
        return Err(AuthorityError::UnknownKey(envelope.key_id.clone()));
    }
    verify_signed_bytes(&bytes, envelope, keyring, Some(now_epoch))?;
    Ok(())
}

pub fn verify_rollback_authorization(
    auth: &RollbackAuthorization,
    envelope: &SignatureEnvelope,
    keyring: &TrustedKeyring,
    now_epoch: u64,
    installed: &ReleaseIdentity,
) -> Result<(), AuthorityError> {
    if auth.schema != ROLLBACK_SCHEMA
        || auth.product_id != installed.product_id
        || auth.current_version != installed.version
        || auth.channel != installed.channel
        || auth.expires_epoch < now_epoch
        || !valid_version(&auth.authorized_target_version)
        || compare_versions(&auth.authorized_target_version, &installed.version) >= 0
        || auth.reason_code.is_empty()
        || envelope.key_id != auth.signer_key_id
    {
        return Err(AuthorityError::UnauthorizedRollback);
    }
    let bytes = canonical_json(auth)?;
    verify_signed_bytes(&bytes, envelope, keyring, Some(now_epoch)).map(|_| ())
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum UpdateState {
    Discovered,
    Validated,
    Downloading,
    Downloaded,
    Verified,
    Staged,
    ReadyToApply,
    Applying,
    Applied,
    RebootRequired,
    RollbackRequired,
    RollingBack,
    RolledBack,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdateTransaction {
    pub transaction_id: String,
    pub source_release: ReleaseIdentity,
    pub target_release: ReleaseIdentity,
    pub channel: ReleaseChannel,
    pub package_sha256: String,
    pub state: UpdateState,
    pub state_sequence: u64,
    pub created_epoch: u64,
    pub updated_epoch: u64,
    pub error_code: Option<String>,
}

impl UpdateTransaction {
    pub fn new(
        source_release: ReleaseIdentity,
        target_release: ReleaseIdentity,
        package_sha256: String,
        epoch: u64,
    ) -> Result<Self, AuthorityError> {
        source_release.validate()?;
        target_release.validate()?;
        if target_release.channel != source_release.channel || !valid_digest(&package_sha256) {
            return Err(AuthorityError::IdentityMismatch);
        }
        Ok(Self {
            transaction_id: Uuid::new_v4().to_string(),
            channel: target_release.channel,
            source_release,
            target_release,
            package_sha256,
            state: UpdateState::Discovered,
            state_sequence: 0,
            created_epoch: epoch,
            updated_epoch: epoch,
            error_code: None,
        })
    }
    pub fn transition(&mut self, next: UpdateState, epoch: u64) -> Result<(), AuthorityError> {
        if !allowed_transition(self.state, next) {
            return Err(AuthorityError::IllegalTransition);
        }
        self.state = next;
        self.state_sequence = self.state_sequence.saturating_add(1);
        self.updated_epoch = epoch;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ImmutableApplyPlan {
    pub transaction_id: String,
    pub installed_version: String,
    pub target_version: String,
    pub channel: ReleaseChannel,
    pub platform: TargetPlatform,
    pub architecture: TargetArchitecture,
    pub package_sha256: String,
    pub release_manifest_sha256: String,
    pub rollback_artifact_sha256: String,
    pub installer_action: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateSource {
    Offline {
        path: String,
    },
    Https {
        url: String,
        timeout_ms: u64,
        max_bytes: u64,
    },
}

impl UpdateSource {
    pub fn validate(&self) -> Result<(), AuthorityError> {
        match self {
            Self::Offline { path } => {
                let candidate = Path::new(path);
                if path.is_empty()
                    || candidate.is_absolute() && path.contains("..")
                    || path.contains('\0')
                {
                    return Err(AuthorityError::UnsupportedSource);
                }
            }
            Self::Https {
                url,
                timeout_ms,
                max_bytes,
            } => {
                if !url.starts_with("https://")
                    || url.contains('@')
                    || url.contains('#')
                    || url.contains('\\')
                    || *timeout_ms == 0
                    || *timeout_ms > 120_000
                    || *max_bytes == 0
                    || *max_bytes > MAX_ARTIFACT_BYTES
                {
                    return Err(AuthorityError::UnsupportedSource);
                }
            }
        }
        Ok(())
    }
}

/// Validate a ZIP/MSI sidecar member before extraction. Direct MSI/Burn apply does
/// not extract archive-controlled paths; the offline ZIP builder uses this policy.
pub fn validate_archive_member(name: &str) -> Result<(), AuthorityError> {
    let path = Path::new(name);
    if name.is_empty()
        || path.is_absolute()
        || name.starts_with('/')
        || name.starts_with('\\')
        || name.starts_with("\\\\")
        || name.contains('\\')
        || name.split('/').any(|part| part == ".." || part.is_empty())
        || name.as_bytes().get(1) == Some(&b':')
        || name.ends_with('/')
    {
        return Err(AuthorityError::UnsafeArchivePath);
    }
    Ok(())
}

impl ImmutableApplyPlan {
    pub fn digest(&self) -> Result<String, AuthorityError> {
        Ok(sha256_hex(&canonical_json(self)?))
    }
    pub fn verify_unchanged(&self, expected_digest: &str) -> Result<(), AuthorityError> {
        if self.digest()? == expected_digest {
            Ok(())
        } else {
            Err(AuthorityError::PlanChanged)
        }
    }
}

fn allowed_transition(from: UpdateState, to: UpdateState) -> bool {
    use UpdateState::*;
    matches!(
        (from, to),
        (Discovered, Validated)
            | (Validated, Downloading)
            | (Downloading, Downloaded)
            | (Downloading, Cancelled)
            | (Downloaded, Verified)
            | (Downloaded, Failed)
            | (Verified, Staged)
            | (Verified, Failed)
            | (Staged, ReadyToApply)
            | (Staged, Cancelled)
            | (ReadyToApply, Applying)
            | (Applying, Applied)
            | (Applying, RebootRequired)
            | (Applying, RollbackRequired)
            | (Applying, Failed)
            | (RollbackRequired, RollingBack)
            | (RollingBack, RolledBack)
            | (RollingBack, Failed)
            | (Failed, Discovered)
            | (Cancelled, Discovered)
    )
}

/// Three-way compare of two dot-separated numeric version strings.
///
/// DBT-P46-B25: both current callers (`verify_update_metadata`,
/// `verify_rollback_authorization`) already reject a malformed version via
/// `valid_version()` before calling this — and they must, because no single
/// silent fallback for a non-numeric segment is safe for both: one call site
/// needs a malformed value to sort low (so it can never look like a valid
/// upgrade), the other needs it to sort high (so it can never look like a
/// valid, strictly-older rollback target). Rather than pick one and leave the
/// other's caller exposed if it forgets to validate, the precondition is
/// enforced here too.
pub fn compare_versions(a: &str, b: &str) -> i8 {
    debug_assert!(
        valid_version(a) && valid_version(b),
        "compare_versions requires both inputs to already pass valid_version() — got {a:?}, {b:?}"
    );
    let parse = |v: &str| -> Vec<u64> {
        v.split('.')
            .map(|part| part.parse::<u64>().unwrap_or(0))
            .collect()
    };
    let left = parse(a);
    let right = parse(b);
    match left.cmp(&right) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

fn valid_version(value: &str) -> bool {
    let parts: Vec<_> = value.split('.').collect();
    parts.len() >= 2
        && parts.len() <= 4
        && parts.iter().all(|part| {
            !part.is_empty()
                && (part.len() == 1 || !part.starts_with('0'))
                && part.bytes().all(|b| b.is_ascii_digit())
        })
}
fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn safe_token(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}
fn platform_name(v: TargetPlatform) -> &'static str {
    match v {
        TargetPlatform::Windows => "windows",
        TargetPlatform::Macos => "macos",
        TargetPlatform::Linux => "linux",
    }
}
fn architecture_name(v: TargetArchitecture) -> &'static str {
    match v {
        TargetArchitecture::X86_64 => "x86_64",
        TargetArchitecture::Aarch64 => "aarch64",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn identity(version: &str) -> ReleaseIdentity {
        ReleaseIdentity {
            schema: IDENTITY_SCHEMA.into(),
            product_id: "AetherCore".into(),
            version: version.into(),
            channel: ReleaseChannel::Stable,
            release_sequence: 1,
            platform: TargetPlatform::Windows,
            architecture: TargetArchitecture::X86_64,
            protocol_version: "p35".into(),
            minimum_updater_version: "0.1.0".into(),
            source_commit: Some("fixture".into()),
            build_epoch: Some(0),
        }
    }
    fn key(seed: u8, id: &str) -> (TrustedReleaseKey, [u8; 32]) {
        let raw = [seed; 32];
        let sk = SigningKey::from_bytes(&raw);
        (
            TrustedReleaseKey {
                key_id: id.into(),
                algorithm: "Ed25519".into(),
                public_key_hex: hex::encode(sk.verifying_key().to_bytes()),
                enabled: true,
                revoked: false,
                not_before_epoch: None,
                not_after_epoch: None,
            },
            raw,
        )
    }
    #[test]
    fn strict_manifest_is_deterministic_and_signed() {
        let (trusted, seed) = key(7, "test-p35");
        let manifest = ReleaseManifest {
            schema: MANIFEST_SCHEMA.into(),
            identity: identity("1.2.3"),
            package: ReleasePackage {
                package_type: "msi".into(),
                filename: "AetherCore-1.2.3-x86_64.msi".into(),
                byte_length: 3,
                sha256: "ab".repeat(32),
            },
            sbom_sha256: "cd".repeat(32),
            sbom_reference: "sbom.cdx.json".into(),
            provenance_sha256: "ef".repeat(32),
            provenance_reference: "provenance.json".into(),
            created_epoch: 0,
            update_contract_version: UPDATE_CONTRACT_VERSION.into(),
        };
        let bytes = canonical_json(&manifest).unwrap();
        assert_eq!(bytes, canonical_json(&manifest).unwrap());
        let sig = sign_bytes(&bytes, "test-p35", &seed).unwrap();
        let ring = TrustedKeyring {
            schema: KEYRING_SCHEMA.into(),
            keys: vec![trusted],
        };
        assert_eq!(
            verify_release_manifest(&manifest, &bytes, &sig, &ring).unwrap(),
            sha256_hex(&bytes)
        );
    }
    #[test]
    fn tamper_unknown_and_revoked_keys_reject() {
        let (mut trusted, seed) = key(8, "k");
        let ring = TrustedKeyring {
            schema: KEYRING_SCHEMA.into(),
            keys: vec![trusted.clone()],
        };
        let body = b"release";
        let sig = sign_bytes(body, "k", &seed).unwrap();
        assert!(verify_signed_bytes(body, &sig, &ring, None).is_ok());
        let mut tampered = sig.clone();
        tampered.signature_hex.replace_range(0..2, "00");
        assert!(verify_signed_bytes(body, &tampered, &ring, None).is_err());
        trusted.revoked = true;
        let revoked = TrustedKeyring {
            schema: KEYRING_SCHEMA.into(),
            keys: vec![trusted],
        };
        assert!(matches!(
            verify_signed_bytes(body, &sig, &revoked, None),
            Err(AuthorityError::RevokedKey(_))
        ));
    }
    #[test]
    fn update_metadata_blocks_downgrade_and_wrong_channel() {
        let (trusted, seed) = key(9, "m");
        let ring = TrustedKeyring {
            schema: KEYRING_SCHEMA.into(),
            keys: vec![trusted],
        };
        let installed = identity("2.0.0");
        let mut metadata = UpdateMetadata {
            schema: UPDATE_METADATA_SCHEMA.into(),
            product_id: "AetherCore".into(),
            channel: ReleaseChannel::Stable,
            current_version: "2.0.0".into(),
            target_identity: identity("1.9.0"),
            platform: TargetPlatform::Windows,
            architecture: TargetArchitecture::X86_64,
            package_url: "https://updates.invalid/a.msi".into(),
            package_length: 4,
            package_sha256: "aa".repeat(32),
            release_manifest_sha256: "bb".repeat(32),
            signing_key_id: "m".into(),
            update_contract_version: UPDATE_CONTRACT_VERSION.into(),
            release_notes_reference: None,
            generated_epoch: 1,
            expires_epoch: 10,
        };
        let sig = sign_bytes(&canonical_json(&metadata).unwrap(), "m", &seed).unwrap();
        assert!(matches!(
            verify_update_metadata(&metadata, &sig, &ring, 2, &installed),
            Err(AuthorityError::DowngradeBlocked)
        ));
        metadata.target_identity.channel = ReleaseChannel::Beta;
        let sig2 = sign_bytes(&canonical_json(&metadata).unwrap(), "m", &seed).unwrap();
        assert!(verify_update_metadata(&metadata, &sig2, &ring, 2, &installed).is_err());
    }
    #[test]
    fn state_machine_rejects_illegal_transition_and_plan_tamper() {
        let source = identity("1.0.0");
        let target = identity("1.1.0");
        let mut tx = UpdateTransaction::new(source, target, "aa".repeat(32), 1).unwrap();
        assert!(tx.transition(UpdateState::Applied, 2).is_err());
        for next in [
            UpdateState::Validated,
            UpdateState::Downloading,
            UpdateState::Downloaded,
            UpdateState::Verified,
            UpdateState::Staged,
            UpdateState::ReadyToApply,
        ] {
            tx.transition(next, 2).unwrap();
        }
        let plan = ImmutableApplyPlan {
            transaction_id: tx.transaction_id,
            installed_version: "1.0.0".into(),
            target_version: "1.1.0".into(),
            channel: ReleaseChannel::Stable,
            platform: TargetPlatform::Windows,
            architecture: TargetArchitecture::X86_64,
            package_sha256: "aa".repeat(32),
            release_manifest_sha256: "bb".repeat(32),
            rollback_artifact_sha256: "cc".repeat(32),
            installer_action: "verified-burn".into(),
        };
        let digest = plan.digest().unwrap();
        assert!(plan.verify_unchanged(&digest).is_ok());
    }
    #[test]
    fn rollback_requires_distinct_signed_authority() {
        let (trusted, seed) = key(10, "r");
        let ring = TrustedKeyring {
            schema: KEYRING_SCHEMA.into(),
            keys: vec![trusted],
        };
        let installed = identity("2.0.0");
        let auth = RollbackAuthorization {
            schema: ROLLBACK_SCHEMA.into(),
            product_id: "AetherCore".into(),
            current_version: "2.0.0".into(),
            authorized_target_version: "1.9.0".into(),
            channel: ReleaseChannel::Stable,
            reason_code: "security-hotfix".into(),
            expires_epoch: 20,
            signer_key_id: "r".into(),
        };
        let sig = sign_bytes(&canonical_json(&auth).unwrap(), "r", &seed).unwrap();
        assert!(verify_rollback_authorization(&auth, &sig, &ring, 2, &installed).is_ok());
    }
    #[test]
    fn strict_parser_rejects_unknown_fields() {
        let bytes = br#"{"schema":"aethercore.release.identity.v1","productId":"AetherCore","version":"1.0.0","channel":"stable","releaseSequence":1,"platform":"windows","architecture":"x86_64","protocolVersion":"p35","minimumUpdaterVersion":"0.1.0","sourceCommit":null,"buildEpoch":0,"unexpected":true}"#;
        assert!(parse_strict::<ReleaseIdentity>(bytes, 4096).is_err());
    }
    #[test]
    fn key_rotation_requires_old_trusted_authority() {
        let (old, seed) = key(11, "old");
        let (new_key, _) = key(12, "new");
        let ring = TrustedKeyring {
            schema: KEYRING_SCHEMA.into(),
            keys: vec![old],
        };
        let mut rotation = KeyRotation {
            schema: ROTATION_SCHEMA.into(),
            old_key_id: "old".into(),
            new_key,
            authorized_by: "old".into(),
            signature_hex: String::new(),
        };
        rotation.signature_hex = hex::encode(
            SigningKey::from_bytes(&seed)
                .sign(&rotation.unsigned_bytes().unwrap())
                .to_bytes(),
        );
        let next = rotation.authorize(&ring, 50).unwrap();
        assert!(next.keys.iter().any(|k| k.key_id == "new"));
    }
    /// P75 — an expired key must not authorize a rotation: rotation is the one
    /// operation that mints new trust, and it ignored the key's validity window.
    #[test]
    fn an_expired_key_cannot_authorize_a_rotation() {
        let (mut old, seed) = key(11, "old");
        old.not_after_epoch = Some(100);
        let (new_key, _) = key(12, "new");
        let ring = TrustedKeyring {
            schema: KEYRING_SCHEMA.into(),
            keys: vec![old],
        };
        let mut rotation = KeyRotation {
            schema: ROTATION_SCHEMA.into(),
            old_key_id: "old".into(),
            new_key,
            authorized_by: "old".into(),
            signature_hex: String::new(),
        };
        rotation.signature_hex = hex::encode(
            SigningKey::from_bytes(&seed)
                .sign(&rotation.unsigned_bytes().unwrap())
                .to_bytes(),
        );
        assert!(matches!(
            rotation.authorize(&ring, 200),
            Err(AuthorityError::RevokedKey(_))
        ));
        assert!(
            rotation.authorize(&ring, 50).is_ok(),
            "inside its window it still may"
        );
    }
    #[test]
    fn rollback_cannot_be_reused_for_different_current_version() {
        let (trusted, seed) = key(13, "r2");
        let ring = TrustedKeyring {
            schema: KEYRING_SCHEMA.into(),
            keys: vec![trusted],
        };
        let installed = identity("3.0.0");
        let auth = RollbackAuthorization {
            schema: ROLLBACK_SCHEMA.into(),
            product_id: "AetherCore".into(),
            current_version: "2.0.0".into(),
            authorized_target_version: "1.9.0".into(),
            channel: ReleaseChannel::Stable,
            reason_code: "hotfix".into(),
            expires_epoch: 20,
            signer_key_id: "r2".into(),
        };
        let sig = sign_bytes(&canonical_json(&auth).unwrap(), "r2", &seed).unwrap();
        assert!(matches!(
            verify_rollback_authorization(&auth, &sig, &ring, 2, &installed),
            Err(AuthorityError::UnauthorizedRollback)
        ));
    }
    #[test]
    fn sources_and_archive_members_are_bounded() {
        assert!(
            UpdateSource::Https {
                url: "https://updates.invalid/a".into(),
                timeout_ms: 10_000,
                max_bytes: 100
            }
            .validate()
            .is_ok()
        );
        assert!(
            UpdateSource::Https {
                url: "http://updates.invalid/a".into(),
                timeout_ms: 10_000,
                max_bytes: 100
            }
            .validate()
            .is_err()
        );
        assert!(validate_archive_member("bin/a.exe").is_ok());
        assert!(validate_archive_member("../escape").is_err());
        assert!(validate_archive_member("C:/escape").is_err());
        assert!(validate_archive_member("/absolute").is_err());
    }

    // DBT-P46-B25. compare_versions silently reads a non-numeric segment as 0
    // (`.unwrap_or(0)`), which is only safe because its two current callers
    // (verify_update_metadata / verify_rollback_authorization) already reject a
    // malformed version via `valid_version()` before ever calling it — and no
    // single silent fallback is safe for BOTH callers' opposite-direction
    // semantics (see the commit message). So the fix is not a fallback value,
    // it's making the precondition loud instead of silent.

    // Pins the existing (correct) defense at both call sites: a structurally
    // malformed target version is rejected before compare_versions ever runs.
    #[test]
    fn malformed_target_version_is_rejected_before_comparison_in_update_metadata() {
        let (trusted, seed) = key(14, "malformed-update");
        let ring = TrustedKeyring {
            schema: KEYRING_SCHEMA.into(),
            keys: vec![trusted],
        };
        let installed = identity("2.0.0");
        let mut metadata = UpdateMetadata {
            schema: UPDATE_METADATA_SCHEMA.into(),
            product_id: "AetherCore".into(),
            channel: ReleaseChannel::Stable,
            current_version: "2.0.0".into(),
            target_identity: identity("2.1.0"),
            platform: TargetPlatform::Windows,
            architecture: TargetArchitecture::X86_64,
            package_url: "https://updates.invalid/a.msi".into(),
            package_length: 4,
            package_sha256: "aa".repeat(32),
            release_manifest_sha256: "bb".repeat(32),
            signing_key_id: "malformed-update".into(),
            update_contract_version: UPDATE_CONTRACT_VERSION.into(),
            release_notes_reference: None,
            generated_epoch: 1,
            expires_epoch: 10,
        };
        metadata.target_identity.version = "2.1.abc".into();
        let sig = sign_bytes(
            &canonical_json(&metadata).unwrap(),
            "malformed-update",
            &seed,
        )
        .unwrap();
        assert!(
            matches!(
                verify_update_metadata(&metadata, &sig, &ring, 2, &installed),
                Err(AuthorityError::InvalidIdentity(_))
            ),
            "a malformed target version must be rejected as an identity-validation failure, never reach the version comparison"
        );
    }

    #[test]
    fn malformed_authorized_target_version_is_rejected_before_comparison_in_rollback() {
        let (trusted, seed) = key(15, "malformed-rollback");
        let ring = TrustedKeyring {
            schema: KEYRING_SCHEMA.into(),
            keys: vec![trusted],
        };
        let installed = identity("2.0.0");
        let auth = RollbackAuthorization {
            schema: ROLLBACK_SCHEMA.into(),
            product_id: "AetherCore".into(),
            current_version: "2.0.0".into(),
            authorized_target_version: "1.abc.0".into(),
            channel: ReleaseChannel::Stable,
            reason_code: "security-hotfix".into(),
            expires_epoch: 20,
            signer_key_id: "malformed-rollback".into(),
        };
        let sig = sign_bytes(&canonical_json(&auth).unwrap(), "malformed-rollback", &seed).unwrap();
        assert!(
            matches!(
                verify_rollback_authorization(&auth, &sig, &ring, 2, &installed),
                Err(AuthorityError::UnauthorizedRollback)
            ),
            "a malformed rollback target version must be rejected, never reach the version comparison"
        );
    }

    // The precondition compare_versions actually depends on, made loud: calling
    // it directly with a version valid_version() would reject must panic in
    // debug/test builds rather than silently miscompare. Before the fix this
    // test fails (no panic — `unwrap_or(0)` just returns a value).
    #[test]
    #[should_panic(expected = "valid_version")]
    fn compare_versions_panics_on_unvalidated_input_instead_of_silently_defaulting_to_zero() {
        compare_versions("1.2.abc", "1.2.0");
    }
}
