#![deny(unsafe_op_in_unsafe_fn)]

mod manifest;
mod coordinator;
mod platform;

pub use coordinator::{verify_file_hash_size,
    UpdateChannel, UpdateCoordinator, UpdateEngineError, UpdateInstallIntent, UpdateReleaseView,
    UpdateSnapshot, UpdateState, UpdateExecutionTicket, UpdateCompletion, UpdateCheckDescriptor,
    UpdateStageUploadDescriptor, MAX_STAGE_CHUNK_BYTES,
};
pub use manifest::{
    ManifestSignature, UpdateManifest, UpdateManifestRelease, UpdatePackage, UpdatePackageKind,
    UpdateTrustChannel, UpdateTrustConfig, manifest_signature_from_hex, sign_manifest_bytes,
    verify_manifest_bytes,
};
pub use platform::{PlatformVerifier, SignatureVerification, default_platform_verifier};
