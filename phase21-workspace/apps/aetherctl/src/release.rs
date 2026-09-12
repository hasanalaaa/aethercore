//! Read-only Phase 35 release/update verification surface for aetherctl.

use crate::error::CliError;
use aethercore_release_authority::{
    ReleaseManifest, SignatureEnvelope, TrustedKeyring, UpdateMetadata, parse_strict,
    verify_release_manifest, verify_update_metadata,
};
use std::{fs, path::Path};

fn read<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, CliError> {
    let bytes = fs::read(path)
        .map_err(|e| CliError::local_io_with("cli.release.readFailed", e.to_string()))?;
    parse_strict(&bytes, 256 * 1024).map_err(|e| CliError::Rejected {
        message_key: "cli.release.invalidArtifact".into(),
        detail: Some(e.to_string()),
    })
}

pub fn inspect(path: Option<&str>) -> Result<serde_json::Value, CliError> {
    let manifest: ReleaseManifest = read(path.unwrap_or("release/release-manifest.json"))?;
    manifest.validate().map_err(|e| CliError::Rejected {
        message_key: "cli.release.invalidManifest".into(),
        detail: Some(e.to_string()),
    })?;
    Ok(
        serde_json::json!({ "schema": manifest.schema, "identity": manifest.identity, "package": manifest.package, "sbomSha256": manifest.sbom_sha256, "provenanceSha256": manifest.provenance_sha256 }),
    )
}

pub fn verify_manifest(
    manifest_path: &str,
    signature_path: &str,
    keyring_path: &str,
) -> Result<serde_json::Value, CliError> {
    let manifest: ReleaseManifest = read(manifest_path)?;
    let bytes = fs::read(manifest_path)
        .map_err(|e| CliError::local_io_with("cli.release.readFailed", e.to_string()))?;
    let signature: SignatureEnvelope = read(signature_path)?;
    let keyring: TrustedKeyring = read(keyring_path)?;
    let digest = verify_release_manifest(&manifest, &bytes, &signature, &keyring).map_err(|e| {
        CliError::Rejected {
            message_key: "cli.release.verificationFailed".into(),
            detail: Some(e.to_string()),
        }
    })?;
    Ok(
        serde_json::json!({ "status": "verified", "manifestSha256": digest, "keyId": signature.key_id, "signatureState": "signed" }),
    )
}

pub fn verify_update(
    metadata_path: &str,
    signature_path: &str,
    keyring_path: &str,
) -> Result<serde_json::Value, CliError> {
    let metadata: UpdateMetadata = read(metadata_path)?;
    let signature: SignatureEnvelope = read(signature_path)?;
    let keyring: TrustedKeyring = read(keyring_path)?;
    let installed = aethercore_release_authority::ReleaseIdentity {
        version: metadata.current_version.clone(),
        ..metadata.target_identity.clone()
    };
    verify_update_metadata(
        &metadata,
        &signature,
        &keyring,
        metadata.generated_epoch,
        &installed,
    )
    .map_err(|e| CliError::Rejected {
        message_key: "cli.update.verificationFailed".into(),
        detail: Some(e.to_string()),
    })?;
    Ok(
        serde_json::json!({ "status": "verified", "channel": metadata.channel, "targetVersion": metadata.target_identity.version, "keyId": signature.key_id }),
    )
}

pub fn verify_offline_bundle(path: &str) -> Result<serde_json::Value, CliError> {
    if !Path::new(path).is_file() {
        return Err(CliError::local_io("cli.release.bundleNotFound"));
    }
    Err(CliError::capability_unavailable(
        "offlineBundleVerifierRequiresReleaseTool",
    ))
}
