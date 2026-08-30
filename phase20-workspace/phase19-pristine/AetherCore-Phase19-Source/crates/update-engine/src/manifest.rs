use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use thiserror::Error;

pub const MANIFEST_SCHEMA: &str = "aethercore.update-manifest.v1";
pub const SIGNATURE_SCHEMA: &str = "aethercore.update-signature.v1";
pub const TRUST_SCHEMA: &str = "aethercore.update-trust.v1";
pub const MAX_MANIFEST_BYTES: usize = 256 * 1024;
pub const MAX_SIGNATURE_BYTES: usize = 8 * 1024;
pub const MAX_RELEASES: usize = 64;
pub const MAX_PACKAGE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const MAX_MANIFEST_LIFETIME_MS: i64 = 30 * 24 * 60 * 60 * 1000;

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("update manifest is too large")]
    ManifestTooLarge,
    #[error("update signature envelope is too large")]
    SignatureTooLarge,
    #[error("update manifest JSON is invalid: {0}")]
    InvalidJson(String),
    #[error("update manifest schema is not supported")]
    UnsupportedSchema,
    #[error("update manifest contains too many releases")]
    TooManyReleases,
    #[error("update manifest channel does not match the selected trust channel")]
    ChannelMismatch,
    #[error("update manifest timestamp is invalid")]
    InvalidTimestamp,
    #[error("update manifest expired")]
    Expired,
    #[error("update manifest was generated too far in the future")]
    FutureDated,
    #[error("update package metadata is invalid")]
    InvalidPackage,
    #[error("update URL must be absolute HTTPS without credentials or fragment")]
    InvalidHttpsUrl,
    #[error("update manifest signing key is unknown")]
    UnknownKey,
    #[error("update signature encoding is invalid")]
    InvalidSignatureEncoding,
    #[error("update signing public key is invalid")]
    InvalidPublicKey,
    #[error("update signature verification failed")]
    SignatureVerification,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTrustConfig {
    pub schema: String,
    pub enabled: bool,
    pub channels: Vec<UpdateTrustChannel>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTrustChannel {
    pub channel: String,
    pub manifest_url: String,
    pub signature_url: String,
    pub key_id: String,
    pub public_key_hex: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateManifest {
    pub schema: String,
    pub channel: String,
    pub sequence: u64,
    pub generated_unix_ms: i64,
    pub expires_unix_ms: i64,
    pub releases: Vec<UpdateManifestRelease>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateManifestRelease {
    pub release_id: String,
    pub version: String,
    pub published_unix_ms: i64,
    pub notes_message_key: String,
    pub minimum_windows_build: u32,
    pub package: UpdatePackage,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePackage {
    pub kind: UpdatePackageKind,
    pub url: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum UpdatePackageKind {
    Burn,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestSignature {
    pub schema: String,
    pub key_id: String,
    pub signature_hex: String,
}

pub fn manifest_signature_from_hex(key_id: impl Into<String>, signature: &[u8; 64]) -> ManifestSignature {
    ManifestSignature { schema: SIGNATURE_SCHEMA.into(), key_id: key_id.into(), signature_hex: hex::encode(signature) }
}

pub fn sign_manifest_bytes(bytes: &[u8], private_key: &[u8; 32]) -> Result<[u8; 64], ManifestError> {
    if bytes.len() > MAX_MANIFEST_BYTES { return Err(ManifestError::ManifestTooLarge); }
    let signing = SigningKey::from_bytes(private_key);
    Ok(signing.sign(bytes).to_bytes())
}

pub fn verify_manifest_bytes(
    manifest_bytes: &[u8],
    signature_bytes: &[u8],
    trust: &UpdateTrustChannel,
    expected_channel: &str,
    now_unix_ms: i64,
) -> Result<UpdateManifest, ManifestError> {
    if manifest_bytes.len() > MAX_MANIFEST_BYTES { return Err(ManifestError::ManifestTooLarge); }
    if signature_bytes.len() > MAX_SIGNATURE_BYTES { return Err(ManifestError::SignatureTooLarge); }
    let signature: ManifestSignature = serde_json::from_slice(signature_bytes)
        .map_err(|e| ManifestError::InvalidJson(e.to_string()))?;
    if signature.schema != SIGNATURE_SCHEMA { return Err(ManifestError::UnsupportedSchema); }
    if trust.channel != expected_channel { return Err(ManifestError::ChannelMismatch); }
    if signature.key_id != trust.key_id { return Err(ManifestError::UnknownKey); }
    let public = hex::decode(&trust.public_key_hex).map_err(|_| ManifestError::InvalidPublicKey)?;
    let public: [u8; 32] = public.try_into().map_err(|_| ManifestError::InvalidPublicKey)?;
    let verifying = VerifyingKey::from_bytes(&public).map_err(|_| ManifestError::InvalidPublicKey)?;
    let signature_raw = hex::decode(&signature.signature_hex).map_err(|_| ManifestError::InvalidSignatureEncoding)?;
    let signature_raw: [u8; 64] = signature_raw.try_into().map_err(|_| ManifestError::InvalidSignatureEncoding)?;
    verifying.verify_strict(manifest_bytes, &Signature::from_bytes(&signature_raw))
        .map_err(|_| ManifestError::SignatureVerification)?;

    // Parse only after authenticating the exact bytes that were received.
    let manifest: UpdateManifest = serde_json::from_slice(manifest_bytes)
        .map_err(|e| ManifestError::InvalidJson(e.to_string()))?;
    validate_manifest(&manifest, expected_channel, now_unix_ms)?;
    Ok(manifest)
}

#[cfg(feature="fuzzing")]
pub fn fuzz_parse_and_validate_manifest_bytes(
    manifest_bytes: &[u8],
    expected_channel: &str,
    now_unix_ms: i64,
) -> Result<UpdateManifest, ManifestError> {
    if manifest_bytes.len() > MAX_MANIFEST_BYTES { return Err(ManifestError::ManifestTooLarge); }
    let manifest: UpdateManifest = serde_json::from_slice(manifest_bytes)
        .map_err(|e| ManifestError::InvalidJson(e.to_string()))?;
    validate_manifest(&manifest, expected_channel, now_unix_ms)?;
    Ok(manifest)
}

pub fn validate_trust_config(value: &UpdateTrustConfig) -> Result<(), ManifestError> {
    if value.schema != TRUST_SCHEMA { return Err(ManifestError::UnsupportedSchema); }
    if !value.enabled { return Ok(()); }
    if value.channels.is_empty() || value.channels.len() > 4 { return Err(ManifestError::InvalidPackage); }
    let mut seen_channels=BTreeSet::new();
    for channel in &value.channels {
        if channel.channel != "stable" && channel.channel != "beta" { return Err(ManifestError::ChannelMismatch); }
        if !seen_channels.insert(channel.channel.as_str()) { return Err(ManifestError::ChannelMismatch); }
        validate_https_url(&channel.manifest_url)?;
        validate_https_url(&channel.signature_url)?;
        if !safe_key_id(&channel.key_id) { return Err(ManifestError::UnknownKey); }
        let key = hex::decode(&channel.public_key_hex).map_err(|_| ManifestError::InvalidPublicKey)?;
        if key.len() != 32 || key.iter().all(|v| *v == 0) { return Err(ManifestError::InvalidPublicKey); }
    }
    Ok(())
}

fn validate_manifest(value: &UpdateManifest, expected_channel: &str, now: i64) -> Result<(), ManifestError> {
    if value.schema != MANIFEST_SCHEMA { return Err(ManifestError::UnsupportedSchema); }
    if value.channel != expected_channel { return Err(ManifestError::ChannelMismatch); }
    if value.releases.len() > MAX_RELEASES { return Err(ManifestError::TooManyReleases); }
    if value.sequence==0 || value.generated_unix_ms <= 0 || value.expires_unix_ms <= value.generated_unix_ms { return Err(ManifestError::InvalidTimestamp); }
    if value.expires_unix_ms.saturating_sub(value.generated_unix_ms)>MAX_MANIFEST_LIFETIME_MS { return Err(ManifestError::InvalidTimestamp); }
    if value.expires_unix_ms < now { return Err(ManifestError::Expired); }
    if value.generated_unix_ms > now.saturating_add(10 * 60 * 1000) { return Err(ManifestError::FutureDated); }
    let mut release_ids=BTreeSet::new();
    for release in &value.releases {
        if !release_ids.insert(release.release_id.as_str()) || !safe_id(&release.release_id) || parse_version(&release.version).is_none() || !safe_message_key(&release.notes_message_key) {
            return Err(ManifestError::InvalidPackage);
        }
        if release.package.kind != UpdatePackageKind::Burn || release.package.size_bytes == 0 || release.package.size_bytes > MAX_PACKAGE_BYTES {
            return Err(ManifestError::InvalidPackage);
        }
        if release.package.sha256.len() != 64 || !release.package.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(ManifestError::InvalidPackage);
        }
        validate_https_url(&release.package.url)?;
    }
    Ok(())
}

pub fn safe_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 80 && value.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}
fn safe_key_id(value:&str)->bool{!value.is_empty()&&value.len()<=64&&value.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'.'|b'-'|b'_'))}
fn safe_message_key(value:&str)->bool{!value.is_empty()&&value.len()<=128&&value.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'.'|b'-'|b'_'))}

pub fn parse_version(value: &str) -> Option<[u64; 4]> {
    let mut out = [0u64; 4];
    let parts: Vec<_> = value.split('.').collect();
    if parts.len() < 2 || parts.len() > 4 { return None; }
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() || (part.len() > 1 && part.starts_with('0')) { return None; }
        out[i] = part.parse().ok()?;
    }
    Some(out)
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) { (Some(a), Some(b)) => a > b, _ => false }
}

pub fn validate_https_url(url: &str) -> Result<(), ManifestError> {
    if url.len() > 2048 || !url.starts_with("https://") || url.bytes().any(|b|b.is_ascii_control()||b==b' '||b==b'\\') { return Err(ManifestError::InvalidHttpsUrl); }
    let rest = &url[8..];
    let host_end = rest.find('/').unwrap_or(rest.len());
    let authority = &rest[..host_end];
    if authority.is_empty() || authority.contains('@') || authority.contains('#') || authority.contains('?') || url.contains('#') {
        return Err(ManifestError::InvalidHttpsUrl);
    }
    Ok(())
}

pub fn sha256_hex(bytes: &[u8]) -> String { hex::encode(Sha256::digest(bytes)) }

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(now: i64) -> UpdateManifest {
        UpdateManifest {
            schema: MANIFEST_SCHEMA.into(), channel: "stable".into(), sequence: 7,
            generated_unix_ms: now - 1_000, expires_unix_ms: now + 60_000,
            releases: vec![UpdateManifestRelease { release_id:"1.2.3-x64".into(), version:"1.2.3".into(), published_unix_ms:now-1_000,
                notes_message_key:"update.notes.1_2_3".into(), minimum_windows_build:22621,
                package:UpdatePackage{kind:UpdatePackageKind::Burn,url:"https://example.invalid/AetherCore.exe".into(),size_bytes:123,sha256:"11".repeat(32)} }]
        }
    }

    #[test]
    fn signature_authenticates_exact_manifest_bytes_before_parse() {
        let now=1_800_000_000_000i64;
        let key=SigningKey::from_bytes(&[7u8;32]);
        let manifest=serde_json::to_vec(&sample(now)).unwrap();
        let signature=manifest_signature_from_hex("k1",&key.sign(&manifest).to_bytes());
        let sig=serde_json::to_vec(&signature).unwrap();
        let trust=UpdateTrustChannel{channel:"stable".into(),manifest_url:"https://example.invalid/m.json".into(),signature_url:"https://example.invalid/m.sig".into(),key_id:"k1".into(),public_key_hex:hex::encode(key.verifying_key().to_bytes())};
        assert!(verify_manifest_bytes(&manifest,&sig,&trust,"stable",now).is_ok());
        let mut tampered=manifest.clone(); tampered[10]^=1;
        assert!(matches!(verify_manifest_bytes(&tampered,&sig,&trust,"stable",now),Err(ManifestError::SignatureVerification)));
    }

    #[test]
    fn rollback_and_url_inputs_are_bounded() {
        assert!(validate_https_url("http://example.invalid/a").is_err());
        assert!(validate_https_url("https://u:p@example.invalid/a").is_err());
        assert!(validate_https_url("https://example.invalid/a#frag").is_err());
        assert!(validate_https_url("https://example.invalid/a path").is_err());
        assert!(safe_id("1.2.3-x64")); assert!(!safe_id("../escape"));
        assert!(is_newer("1.2.0","1.1.9")); assert!(!is_newer("1.2.0","1.2.0"));
    }

    #[test]
    fn trust_and_manifest_reject_ambiguity_and_excessive_lifetime(){
        let key=SigningKey::from_bytes(&[3u8;32]);
        let channel=UpdateTrustChannel{channel:"stable".into(),manifest_url:"https://example.invalid/m.json".into(),signature_url:"https://example.invalid/m.sig".into(),key_id:"release-2026".into(),public_key_hex:hex::encode(key.verifying_key().to_bytes())};
        let duplicate=UpdateTrustConfig{schema:TRUST_SCHEMA.into(),enabled:true,channels:vec![channel.clone(),channel.clone()]};
        assert!(validate_trust_config(&duplicate).is_err());
        let now=1_800_000_000_000i64;let mut manifest=sample(now);manifest.expires_unix_ms=manifest.generated_unix_ms+MAX_MANIFEST_LIFETIME_MS+1;
        assert!(validate_manifest(&manifest,"stable",now).is_err());
        let mut manifest=sample(now);manifest.releases.push(manifest.releases[0].clone());
        assert!(validate_manifest(&manifest,"stable",now).is_err());
    }
}
