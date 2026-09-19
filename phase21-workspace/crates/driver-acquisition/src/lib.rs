#![forbid(unsafe_code)]

use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use aethercore_driver_authority::MAX_DOWNLOAD_BYTES;
use aethercore_update_engine::{
    PlatformVerifier, SignatureVerification, default_platform_verifier,
};
use reqwest::{StatusCode, Url, blocking::Client, header::LOCATION};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const DOWNLOAD_CHUNK_BYTES: usize = 128 * 1024;

#[derive(Debug, Error)]
pub enum AcquisitionError {
    #[error("only HTTPS official sources are permitted")]
    HttpsRequired,
    #[error("download origin is outside the provider allow policy: {0}")]
    OriginRejected(String),
    #[error("redirect limit exceeded")]
    RedirectLimit,
    #[error("redirect response did not contain a valid Location")]
    InvalidRedirect,
    #[error("download is larger than the provider or global bound")]
    TooLarge,
    #[error("download was cancelled before privileged mutation")]
    Cancelled,
    #[error("staging path is not a safe local directory")]
    UnsafeStaging,
    #[error("network request failed: {0}")]
    Network(String),
    #[error("download returned HTTP status {0}")]
    Http(u16),
    #[error("I/O failed: {0}")]
    Io(String),
    #[error("package digest did not match official metadata")]
    DigestMismatch,
    #[error("Authenticode trust verification failed")]
    SignatureRejected,
    #[error("Authenticode signer identity was not available for publisher enforcement")]
    SignerIdentityUnavailable,
    #[error(
        "package was validly signed but signer did not match the provider-approved publisher identity"
    )]
    UnexpectedPublisher,
    #[error("test-signed packages are not accepted as production driver packages")]
    TestSignedRejected,
}

pub type Result<T> = std::result::Result<T, AcquisitionError>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OfficialHostRule {
    pub host: String,
    pub include_subdomains: bool,
}

impl OfficialHostRule {
    fn permits(&self, host: &str) -> bool {
        let rule = self.host.trim().trim_end_matches('.').to_ascii_lowercase();
        let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
        host == rule || (self.include_subdomains && host.ends_with(&format!(".{rule}")))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderNetworkPolicy {
    pub provider_id: String,
    pub authority_hosts: Vec<OfficialHostRule>,
    pub distribution_hosts: Vec<OfficialHostRule>,
    pub max_redirects: u8,
    pub max_download_bytes: u64,
}

impl ProviderNetworkPolicy {
    pub fn validate_url(&self, url: &Url) -> Result<()> {
        if url.scheme() != "https" {
            return Err(AcquisitionError::HttpsRequired);
        }
        let host = url
            .host_str()
            .ok_or_else(|| AcquisitionError::OriginRejected("missing host".into()))?;
        if self
            .authority_hosts
            .iter()
            .chain(self.distribution_hosts.iter())
            .any(|rule| rule.permits(host))
        {
            Ok(())
        } else {
            Err(AcquisitionError::OriginRejected(host.into()))
        }
    }

    fn byte_limit(&self) -> u64 {
        self.max_download_bytes.clamp(1, MAX_DOWNLOAD_BYTES)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrustedDownloadRequest {
    pub candidate_id: String,
    pub provider_id: String,
    pub package_url: String,
    pub expected_sha256: String,
    pub expected_bytes: Option<u64>,
    pub expected_publisher: String,
    #[serde(default)]
    pub expected_signer_identities: Vec<String>,
    pub network_policy: ProviderNetworkPolicy,
}

impl TrustedDownloadRequest {
    pub fn validate(&self) -> Result<Url> {
        if self.provider_id != self.network_policy.provider_id {
            return Err(AcquisitionError::OriginRejected(
                "provider policy mismatch".into(),
            ));
        }
        if self.expected_sha256.len() != 64
            || !self.expected_sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(AcquisitionError::DigestMismatch);
        }
        if self.expected_publisher.trim().is_empty() && self.expected_signer_identities.is_empty() {
            return Err(AcquisitionError::SignerIdentityUnavailable);
        }
        let url =
            Url::parse(&self.package_url).map_err(|e| AcquisitionError::Network(e.to_string()))?;
        self.network_policy.validate_url(&url)?;
        if self
            .expected_bytes
            .is_some_and(|bytes| bytes > self.network_policy.byte_limit())
        {
            return Err(AcquisitionError::TooLarge);
        }
        Ok(url)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StagedPackage {
    pub candidate_id: String,
    pub provider_id: String,
    pub staged_path: PathBuf,
    pub source_url: String,
    pub effective_url: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub expected_publisher: String,
    pub actual_signer_subject: String,
    pub actual_signer_identity: String,
    pub chain_status: String,
    pub test_signed: bool,
    pub authenticode_valid: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DownloadProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub stage: &'static str,
}

pub trait Cancellation: Send + Sync {
    fn cancelled(&self) -> bool;
}

impl<F: Fn() -> bool + Send + Sync> Cancellation for F {
    fn cancelled(&self) -> bool {
        self()
    }
}

pub struct DriverAcquisitionEngine {
    client: Client,
    verifier: Arc<dyn PlatformVerifier>,
}

impl DriverAcquisitionEngine {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(std::time::Duration::from_secs(20))
            .timeout(std::time::Duration::from_secs(30 * 60))
            .build()
            .map_err(|e| AcquisitionError::Network(e.to_string()))?;
        Ok(Self {
            client,
            verifier: default_platform_verifier(),
        })
    }

    pub fn with_verifier(verifier: Arc<dyn PlatformVerifier>) -> Result<Self> {
        let mut engine = Self::new()?;
        engine.verifier = verifier;
        Ok(engine)
    }

    /// Downloads only into a non-privileged caller-provided staging root. The privileged service
    /// must consume the returned frozen identity and independently revalidate file identity/hash
    /// immediately before any supported driver servicing API is called.
    pub fn acquire(
        &self,
        request: &TrustedDownloadRequest,
        staging_root: &Path,
        cancellation: &dyn Cancellation,
        mut progress: impl FnMut(DownloadProgress),
    ) -> Result<StagedPackage> {
        let source = request.validate()?;
        prepare_staging_root(staging_root)?;
        let (mut response, effective) =
            self.follow_official_redirects(source.clone(), &request.network_policy, cancellation)?;
        if !response.status().is_success() {
            return Err(AcquisitionError::Http(response.status().as_u16()));
        }
        let limit = request.network_policy.byte_limit();
        let declared = response.content_length();
        if declared.is_some_and(|bytes| bytes > limit) {
            return Err(AcquisitionError::TooLarge);
        }
        if let (Some(expected), Some(actual)) = (request.expected_bytes, declared)
            && expected != actual
        {
            return Err(AcquisitionError::DigestMismatch);
        }

        let partial = staging_root.join(format!("{}.partial", safe_token(&request.candidate_id)));
        let final_path =
            staging_root.join(format!("{}.driverpkg", safe_token(&request.candidate_id)));
        let _ = fs::remove_file(&partial);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)
            .map_err(|e| AcquisitionError::Io(e.to_string()))?;
        let mut hasher = Sha256::new();
        let mut downloaded = 0u64;
        let mut buffer = vec![0u8; DOWNLOAD_CHUNK_BYTES];
        progress(DownloadProgress {
            downloaded_bytes: 0,
            total_bytes: declared,
            stage: "Downloading",
        });
        loop {
            if cancellation.cancelled() {
                let _ = fs::remove_file(&partial);
                return Err(AcquisitionError::Cancelled);
            }
            let count = response
                .read(&mut buffer)
                .map_err(|e| AcquisitionError::Network(e.to_string()))?;
            if count == 0 {
                break;
            }
            downloaded = downloaded.saturating_add(count as u64);
            if downloaded > limit {
                let _ = fs::remove_file(&partial);
                return Err(AcquisitionError::TooLarge);
            }
            hasher.update(&buffer[..count]);
            file.write_all(&buffer[..count])
                .map_err(|e| AcquisitionError::Io(e.to_string()))?;
            progress(DownloadProgress {
                downloaded_bytes: downloaded,
                total_bytes: declared,
                stage: "Downloading",
            });
        }
        file.flush()
            .map_err(|e| AcquisitionError::Io(e.to_string()))?;
        file.sync_all()
            .map_err(|e| AcquisitionError::Io(e.to_string()))?;
        drop(file);
        if request
            .expected_bytes
            .is_some_and(|expected| expected != downloaded)
        {
            let _ = fs::remove_file(&partial);
            return Err(AcquisitionError::DigestMismatch);
        }
        let digest = hex::encode(hasher.finalize());
        if !digest.eq_ignore_ascii_case(&request.expected_sha256) {
            let _ = fs::remove_file(&partial);
            return Err(AcquisitionError::DigestMismatch);
        }
        progress(DownloadProgress {
            downloaded_bytes: downloaded,
            total_bytes: Some(downloaded),
            stage: "VerifyingSignature",
        });
        let signature = self.verifier.verify_authenticode(&partial).map_err(|_| {
            let _ = fs::remove_file(&partial);
            AcquisitionError::SignatureRejected
        })?;
        if let Err(error) = validate_expected_publisher(request, &signature) {
            let _ = fs::remove_file(&partial);
            return Err(error);
        }
        let _ = fs::remove_file(&final_path);
        fs::rename(&partial, &final_path).map_err(|e| AcquisitionError::Io(e.to_string()))?;
        progress(DownloadProgress {
            downloaded_bytes: downloaded,
            total_bytes: Some(downloaded),
            stage: "Ready",
        });
        Ok(StagedPackage {
            candidate_id: request.candidate_id.clone(),
            provider_id: request.provider_id.clone(),
            staged_path: final_path,
            source_url: source.to_string(),
            effective_url: effective.to_string(),
            sha256: digest,
            size_bytes: downloaded,
            expected_publisher: request.expected_publisher.clone(),
            actual_signer_subject: signature.signer_subject,
            actual_signer_identity: signature.signer_thumbprint_or_identity,
            chain_status: signature.chain_status,
            test_signed: signature.test_signed,
            authenticode_valid: signature.valid,
        })
    }

    fn follow_official_redirects(
        &self,
        mut url: Url,
        policy: &ProviderNetworkPolicy,
        cancellation: &dyn Cancellation,
    ) -> Result<(reqwest::blocking::Response, Url)> {
        policy.validate_url(&url)?;
        for redirects in 0..=policy.max_redirects {
            if cancellation.cancelled() {
                return Err(AcquisitionError::Cancelled);
            }
            let response = self
                .client
                .get(url.clone())
                .send()
                .map_err(|e| AcquisitionError::Network(e.to_string()))?;
            if !is_redirect(response.status()) {
                return Ok((response, url));
            }
            if redirects >= policy.max_redirects {
                return Err(AcquisitionError::RedirectLimit);
            }
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or(AcquisitionError::InvalidRedirect)?;
            let next = url
                .join(location)
                .map_err(|_| AcquisitionError::InvalidRedirect)?;
            policy.validate_url(&next)?;
            url = next;
        }
        Err(AcquisitionError::RedirectLimit)
    }
}

fn normalized_signer_identity(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_ascii_whitespace() && *c != ':')
        .flat_map(char::to_uppercase)
        .collect()
}

fn normalized_subject(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

pub fn validate_expected_publisher(
    request: &TrustedDownloadRequest,
    signature: &SignatureVerification,
) -> Result<()> {
    if !signature.valid {
        return Err(AcquisitionError::SignatureRejected);
    }
    if signature.test_signed {
        return Err(AcquisitionError::TestSignedRejected);
    }
    if !request.expected_signer_identities.is_empty() {
        let actual = normalized_signer_identity(&signature.signer_thumbprint_or_identity);
        if actual.is_empty() {
            return Err(AcquisitionError::SignerIdentityUnavailable);
        }
        let matches = request.expected_signer_identities.iter().any(|expected| {
            let expected = normalized_signer_identity(expected);
            !expected.is_empty() && expected == actual
        });
        return if matches {
            Ok(())
        } else {
            Err(AcquisitionError::UnexpectedPublisher)
        };
    }
    let expected = normalized_subject(&request.expected_publisher);
    let actual = normalized_subject(&signature.signer_subject);
    if expected.is_empty() || actual.is_empty() {
        return Err(AcquisitionError::SignerIdentityUnavailable);
    }
    if expected == actual {
        Ok(())
    } else {
        Err(AcquisitionError::UnexpectedPublisher)
    }
}

fn is_redirect(status: StatusCode) -> bool {
    status.is_redirection()
}

fn safe_token(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    hex::encode(digest)[..32].to_owned()
}

fn prepare_staging_root(root: &Path) -> Result<()> {
    fs::create_dir_all(root).map_err(|e| AcquisitionError::Io(e.to_string()))?;
    let meta = fs::symlink_metadata(root).map_err(|e| AcquisitionError::Io(e.to_string()))?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err(AcquisitionError::UnsafeStaging);
    }
    if !root.is_absolute() {
        return Err(AcquisitionError::UnsafeStaging);
    }
    let canonical = root
        .canonicalize()
        .map_err(|e| AcquisitionError::Io(e.to_string()))?;
    let canonical_meta =
        fs::symlink_metadata(&canonical).map_err(|e| AcquisitionError::Io(e.to_string()))?;
    if !canonical_meta.is_dir() || canonical_meta.file_type().is_symlink() {
        return Err(AcquisitionError::UnsafeStaging);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> ProviderNetworkPolicy {
        ProviderNetworkPolicy {
            provider_id: "oem.test".into(),
            authority_hosts: vec![OfficialHostRule {
                host: "vendor.example".into(),
                include_subdomains: false,
            }],
            distribution_hosts: vec![OfficialHostRule {
                host: "cdn.vendor.example".into(),
                include_subdomains: true,
            }],
            max_redirects: 3,
            max_download_bytes: 1024,
        }
    }

    #[test]
    fn rejects_http_and_unrelated_redirect_hosts() {
        assert!(matches!(
            policy().validate_url(&Url::parse("http://vendor.example/a").unwrap()),
            Err(AcquisitionError::HttpsRequired)
        ));
        assert!(matches!(
            policy().validate_url(&Url::parse("https://attacker.example/a").unwrap()),
            Err(AcquisitionError::OriginRejected(_))
        ));
        assert!(
            policy()
                .validate_url(&Url::parse("https://edge.cdn.vendor.example/a").unwrap())
                .is_ok()
        );
    }

    #[test]
    fn request_requires_provider_bound_digest_metadata() {
        let mut request = TrustedDownloadRequest {
            candidate_id: "c".into(),
            provider_id: "different".into(),
            package_url: "https://vendor.example/a".into(),
            expected_sha256: "a".repeat(64),
            expected_bytes: Some(10),
            expected_publisher: "Vendor".into(),
            expected_signer_identities: vec![],
            network_policy: policy(),
        };
        assert!(request.validate().is_err());
        request.provider_id = "oem.test".into();
        assert!(request.validate().is_ok());
        request.expected_sha256 = "not-a-digest".into();
        assert!(matches!(
            request.validate(),
            Err(AcquisitionError::DigestMismatch)
        ));
    }

    #[test]
    fn d18_08_valid_signature_wrong_publisher_fails_closed() {
        let request = TrustedDownloadRequest {
            candidate_id: "c".into(),
            provider_id: "oem.test".into(),
            package_url: "https://vendor.example/a".into(),
            expected_sha256: "a".repeat(64),
            expected_bytes: Some(10),
            expected_publisher: "Expected Vendor LLC".into(),
            expected_signer_identities: vec![],
            network_policy: policy(),
        };
        let evidence = SignatureVerification {
            valid: true,
            signer_subject: "Different Vendor LLC".into(),
            signer_thumbprint_or_identity: "AA11".into(),
            chain_status: "Trusted".into(),
            test_signed: false,
        };
        assert!(matches!(
            validate_expected_publisher(&request, &evidence),
            Err(AcquisitionError::UnexpectedPublisher)
        ));
    }

    #[test]
    fn signer_identity_set_is_stronger_than_display_subject() {
        let request = TrustedDownloadRequest {
            candidate_id: "c".into(),
            provider_id: "oem.test".into(),
            package_url: "https://vendor.example/a".into(),
            expected_sha256: "a".repeat(64),
            expected_bytes: Some(10),
            expected_publisher: "Display Name May Change".into(),
            expected_signer_identities: vec!["AA:BB:CC".into()],
            network_policy: policy(),
        };
        let evidence = SignatureVerification {
            valid: true,
            signer_subject: "Another Display Name".into(),
            signer_thumbprint_or_identity: "aabbcc".into(),
            chain_status: "Trusted".into(),
            test_signed: false,
        };
        assert!(validate_expected_publisher(&request, &evidence).is_ok());
    }

    #[test]
    fn missing_signer_evidence_cannot_be_promoted_to_trusted() {
        let request = TrustedDownloadRequest {
            candidate_id: "c".into(),
            provider_id: "oem.test".into(),
            package_url: "https://vendor.example/a".into(),
            expected_sha256: "a".repeat(64),
            expected_bytes: Some(10),
            expected_publisher: "Vendor".into(),
            expected_signer_identities: vec![],
            network_policy: policy(),
        };
        let evidence = SignatureVerification::validity_only();
        assert!(matches!(
            validate_expected_publisher(&request, &evidence),
            Err(AcquisitionError::SignerIdentityUnavailable)
        ));
    }
}
