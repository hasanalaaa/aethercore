//! Read-only Phase 35 release/update verification surface for aetherctl.

use crate::error::CliError;
use aethercore_release_authority::{
    ReleaseManifest, SignatureEnvelope, TargetArchitecture, TargetPlatform, TrustedKeyring,
    UpdateMetadata, parse_strict, verify_release_manifest, verify_update_metadata,
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
    // "Installed" and "now" come from this binary and this clock, never from the file
    // being verified (the workspace version is the single product version source).
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| CliError::local_io_with("cli.update.clockBeforeEpoch", e.to_string()))?
        .as_secs();
    let installed = aethercore_release_authority::ReleaseIdentity {
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform: this_platform()?,
        architecture: this_architecture()?,
        // The installed channel is not recorded anywhere this CLI can read, so it is
        // taken from the metadata and reported under notChecked.
        ..metadata.target_identity.clone()
    };
    verify_update_metadata(&metadata, &signature, &keyring, now, &installed).map_err(|e| {
        CliError::Rejected {
            message_key: "cli.update.verificationFailed".into(),
            detail: Some(e.to_string()),
        }
    })?;
    Ok(serde_json::json!({
        "status": "verified",
        "channel": metadata.channel,
        "targetVersion": metadata.target_identity.version,
        "keyId": signature.key_id,
        "installedVersion": installed.version,
        "checkedAtEpoch": now,
        "notChecked": ["installedChannel"],
    }))
}

fn this_platform() -> Result<TargetPlatform, CliError> {
    if cfg!(windows) {
        Ok(TargetPlatform::Windows)
    } else if cfg!(target_os = "macos") {
        Ok(TargetPlatform::Macos)
    } else if cfg!(target_os = "linux") {
        Ok(TargetPlatform::Linux)
    } else {
        Err(CliError::capability_unavailable(
            "updatePlatformUnsupported",
        ))
    }
}

fn this_architecture() -> Result<TargetArchitecture, CliError> {
    if cfg!(target_arch = "x86_64") {
        Ok(TargetArchitecture::X86_64)
    } else if cfg!(target_arch = "aarch64") {
        Ok(TargetArchitecture::Aarch64)
    } else {
        Err(CliError::capability_unavailable(
            "updateArchitectureUnsupported",
        ))
    }
}

pub fn verify_offline_bundle(path: &str) -> Result<serde_json::Value, CliError> {
    if !Path::new(path).is_file() {
        return Err(CliError::local_io("cli.release.bundleNotFound"));
    }
    Err(CliError::capability_unavailable(
        "offlineBundleVerifierRequiresReleaseTool",
    ))
}
