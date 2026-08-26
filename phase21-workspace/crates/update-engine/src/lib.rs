#![deny(unsafe_op_in_unsafe_fn)]

mod coordinator;
mod manifest;
mod platform;

pub use coordinator::{
    MAX_STAGE_CHUNK_BYTES, UpdateChannel, UpdateCheckDescriptor, UpdateCompletion,
    UpdateCoordinator, UpdateEngineError, UpdateExecutionTicket, UpdateInstallIntent,
    UpdateReleaseView, UpdateSnapshot, UpdateStageUploadDescriptor, UpdateState,
    verify_file_hash_size,
};
pub use manifest::{
    ManifestSignature, UpdateManifest, UpdateManifestRelease, UpdatePackage, UpdatePackageKind,
    UpdateTrustChannel, UpdateTrustConfig, manifest_signature_from_hex, sign_manifest_bytes,
    verify_manifest_bytes,
};
pub use platform::{PlatformVerifier, SignatureVerification, default_platform_verifier};
// Phase 27 (unix composition): exposed so the service composition can resolve the build
// number where the API exists and degrade to 0 (updates ineligible) elsewhere.
pub use platform::current_windows_build;
