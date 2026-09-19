#![forbid(unsafe_code)]

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use aethercore_gpu_policy::{GpuVendor, policy as gpu_policy};
use aethercore_windows_pnp::{DeviceRecord, InstalledDriver, normalize_pnp_id};
use aethercore_windows_update::{DriverOffer, VersionSource};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

mod provider_registry;
mod truth;
pub use provider_registry::{
    BUILTIN_PROVIDER_COVERAGE_JSON, PROVIDER_COVERAGE_SCHEMA, ProviderAcquisitionCapability,
    ProviderAuthorityClass, ProviderCoverageEntry, ProviderDiscoveryCapability,
    ProviderImplementationStatus, ProviderInstallCapability, ProviderQualificationStatus,
    ProviderRegistry, ProviderUpdateAvailabilityCapability, builtin_provider_registry,
};
pub use truth::{
    AuthorityEvaluation, AuthorityEvaluationState, AuthorityRequirement, DeviceAuthorityCoverage,
    DriverManagementAuthority, ManagementAuthorityAvailability, UpdateAvailabilityEvidence,
    evaluate_required_authorities, management_authority_for_gpu,
    management_authority_for_registry_provider, required_authorities,
};

pub const POLICY_VERSION: &str = "P18.1-AUTHORITY-2";
pub const MAX_PROVIDER_CANDIDATES: usize = 256;
pub const MAX_DOWNLOAD_BYTES: u64 = 8 * 1024 * 1024 * 1024;
pub const MAX_REDIRECTS: u8 = 5;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AuthorityError {
    #[error("candidate is incompatible with the device or machine")]
    Incompatible,
    #[error("candidate trust is unacceptable")]
    Untrusted,
    #[error("candidate source is not an approved official origin")]
    UnofficialOrigin,
    #[error("candidate is firmware and requires protected review")]
    FirmwareProtected,
    #[error("candidate cannot be executed by AetherCore")]
    NotExecutable,
    #[error("candidate changed after consent")]
    PlanInvalidated,
    #[error("driver version ordering is unknown")]
    VersionOrderingUnknown,
    #[error("driver provider registry is invalid: {0}")]
    ProviderRegistryInvalid(String),
}

pub type Result<T> = std::result::Result<T, AuthorityError>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum AuthorityType {
    WindowsUpdate,
    Oem,
    ComponentVendor,
    VendorUtility,
    ManualOfficial,
}

impl AuthorityType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WindowsUpdate => "WindowsUpdate",
            Self::Oem => "Oem",
            Self::ComponentVendor => "ComponentVendor",
            Self::VendorUtility => "VendorUtility",
            Self::ManualOfficial => "ManualOfficial",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum TrustLevel {
    TrustedPlatform,
    TrustedVendor,
    ReviewRequired,
    Rejected,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum AcquisitionCapability {
    WindowsManaged,
    DirectTrusted,
    OfficialUtility,
    ManualOfficial,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum InstallationCapability {
    WindowsManaged,
    DirectTrusted,
    OfficialUtility,
    ManualOfficial,
    Unsupported,
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord, Default,
)]
#[serde(rename_all = "camelCase")]
pub enum MachineKind {
    Oem,
    SelfBuilt,
    #[default]
    Unknown,
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord, Default,
)]
#[serde(rename_all = "camelCase")]
pub enum CoverageState {
    CompleteForRequiredAuthorities,
    Partial,
    Offline,
    ProviderUnavailable,
    ManualAuthorityRequired,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum MatchKind {
    HardwareId,
    CompatibleId,
    DeviceClass,
    VendorFamily,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum ApplicabilityState {
    Exact,
    Compatible,
    Contextual,
    Incompatible,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum PackageTrustState {
    WindowsManaged,
    TrustedSignature,
    TrustedSignatureAndDigest,
    UnexpectedPublisher,
    InvalidSignature,
    Unsigned,
    Unverifiable,
    TestSigned,
    NotApplicable,
}

impl PackageTrustState {
    pub fn acceptable_for_recommendation(self) -> bool {
        matches!(
            self,
            Self::WindowsManaged | Self::TrustedSignature | Self::TrustedSignatureAndDigest
        )
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum RecommendationState {
    Recommended,
    Alternative,
    Optional,
    NotRecommended,
    ManualOfficial,
    FirmwareProtected,
    CurrentDriverPreferred,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecommendationReason {
    OemMachineSpecific,
    WindowsApplicable,
    ComponentReferenceDriver,
    InstalledDriverAlreadyPreferred,
    NewerButLowerAuthority,
    FirmwareRequiresManualReview,
    ExactHardwareIdMatch,
    CompatibleIdMatch,
    MissingDriverPriority,
    DeviceProblemPriority,
    TrustRejected,
    Incompatible,
    VersionOrderingUnknown,
    AuthorityCoverageIncomplete,
    UserIgnoredExactVersion,
    UserIgnoredOptional,
    UserDeferred,
}

impl RecommendationReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OemMachineSpecific => "OEM_MACHINE_SPECIFIC",
            Self::WindowsApplicable => "WINDOWS_APPLICABLE",
            Self::ComponentReferenceDriver => "COMPONENT_REFERENCE_DRIVER",
            Self::InstalledDriverAlreadyPreferred => "INSTALLED_DRIVER_ALREADY_PREFERRED",
            Self::NewerButLowerAuthority => "NEWER_BUT_LOWER_AUTHORITY",
            Self::FirmwareRequiresManualReview => "FIRMWARE_REQUIRES_MANUAL_REVIEW",
            Self::ExactHardwareIdMatch => "EXACT_HARDWARE_ID_MATCH",
            Self::CompatibleIdMatch => "COMPATIBLE_ID_MATCH",
            Self::MissingDriverPriority => "MISSING_DRIVER_PRIORITY",
            Self::DeviceProblemPriority => "DEVICE_PROBLEM_PRIORITY",
            Self::TrustRejected => "TRUST_REJECTED",
            Self::Incompatible => "INCOMPATIBLE",
            Self::VersionOrderingUnknown => "VERSION_ORDERING_UNKNOWN",
            Self::AuthorityCoverageIncomplete => "AUTHORITY_COVERAGE_INCOMPLETE",
            Self::UserIgnoredExactVersion => "USER_IGNORED_EXACT_VERSION",
            Self::UserIgnoredOptional => "USER_IGNORED_OPTIONAL",
            Self::UserDeferred => "USER_DEFERRED",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeviceIdentity {
    pub privacy_key: String,
    pub instance_id: String,
    pub hardware_ids: Vec<String>,
    pub compatible_ids: Vec<String>,
    pub class_name: String,
    pub class_guid: String,
    pub vendor_id: String,
    pub device_id: String,
    pub subsystem_vendor_id: String,
    pub subsystem_device_id: String,
    pub revision: String,
    pub enumerator: String,
    pub manufacturer: String,
    pub current_inf: String,
    pub driver_service: String,
    pub current_provider: String,
    pub current_version: String,
    pub current_date: String,
    pub signer: String,
    pub problem_code: u32,
    pub device_state: String,
}

impl DeviceIdentity {
    pub fn from_pnp(device: &DeviceRecord) -> Self {
        let privacy_key = privacy_device_key(&device.instance_id);
        let parts = parse_best_hardware_id(&device.hardware_ids);
        Self {
            privacy_key,
            instance_id: device.instance_id.clone(),
            hardware_ids: device
                .hardware_ids
                .iter()
                .map(|v| normalize_pnp_id(v))
                .collect(),
            compatible_ids: device
                .compatible_ids
                .iter()
                .map(|v| normalize_pnp_id(v))
                .collect(),
            class_name: device.class_name.clone(),
            class_guid: device.class_guid.clone(),
            vendor_id: parts.get("VEN").cloned().unwrap_or_default(),
            device_id: parts.get("DEV").cloned().unwrap_or_default(),
            subsystem_vendor_id: parts.get("SUBSYS_VENDOR").cloned().unwrap_or_default(),
            subsystem_device_id: parts.get("SUBSYS_DEVICE").cloned().unwrap_or_default(),
            revision: parts.get("REV").cloned().unwrap_or_default(),
            enumerator: device.enumerator.clone(),
            manufacturer: device.manufacturer.clone(),
            current_inf: device
                .driver
                .as_ref()
                .map(|d| d.inf_path.clone())
                .unwrap_or_default(),
            driver_service: String::new(),
            current_provider: device
                .driver
                .as_ref()
                .map(|d| d.provider.clone())
                .unwrap_or_default(),
            current_version: device
                .driver
                .as_ref()
                .map(|d| d.version.clone())
                .unwrap_or_default(),
            current_date: device
                .driver
                .as_ref()
                .map(|d| d.date.clone())
                .unwrap_or_default(),
            signer: String::new(),
            problem_code: device.status.problem_code,
            device_state: if device.status.missing_driver {
                "MissingDriver"
            } else if device.status.has_problem {
                "Problem"
            } else {
                "Present"
            }
            .into(),
        }
    }
}

pub fn privacy_device_key(instance_id: &str) -> String {
    let mut seed = Sha256::new();
    seed.update(normalize_pnp_id(instance_id).as_bytes());
    hex::encode(seed.finalize())[..24].to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct MachineProfile {
    pub manufacturer: String,
    pub product_model: String,
    pub product_family: String,
    pub board_product: String,
    pub architecture: String,
    pub windows_edition: String,
    pub windows_build: String,
    pub os_channel: String,
    pub form_factor: String,
    pub machine_kind: MachineKind,
}

impl MachineProfile {
    pub fn privacy_minimized(
        architecture: impl Into<String>,
        windows_build: impl Into<String>,
    ) -> Self {
        Self {
            architecture: architecture.into(),
            windows_build: windows_build.into(),
            machine_kind: MachineKind::Unknown,
            ..Self::default()
        }
    }

    /// Collects only machine-level fields required for driver authority policy. The underlying
    /// Windows collector deliberately excludes serial numbers, UUIDs and asset tags. Phase 18.1
    /// deliberately returns Unknown rather than inferring OEM from an arbitrary non-generic board
    /// manufacturer; retail motherboard systems must not silently acquire OEM ranking semantics.
    pub fn collect_local() -> Self {
        let identity = aethercore_windows_foundation::machine_identity().unwrap_or_default();
        let manufacturer = identity.manufacturer.trim().to_owned();
        let product_model = identity.product_model.trim().to_owned();
        let product_family = identity.product_family.trim().to_owned();
        let board_product = identity.board_product.trim().to_owned();
        let machine_kind = classify_machine_kind(
            &manufacturer,
            &product_model,
            &product_family,
            &board_product,
        );
        Self {
            manufacturer,
            product_model,
            product_family,
            board_product,
            architecture: std::env::consts::ARCH.to_owned(),
            windows_edition: identity.windows_edition,
            windows_build: identity.windows_build,
            os_channel: identity.os_display_version,
            form_factor: String::new(),
            machine_kind,
        }
    }
}

fn classify_machine_kind(
    manufacturer: &str,
    product_model: &str,
    product_family: &str,
    board_product: &str,
) -> MachineKind {
    let m = manufacturer.trim().to_ascii_lowercase();
    let model = product_model.trim().to_ascii_lowercase();
    let family = product_family.trim().to_ascii_lowercase();
    let board = board_product.trim().to_ascii_lowercase();
    if m.is_empty()
        || m.contains("system manufacturer")
        || m.contains("to be filled")
        || m.contains("default string")
    {
        return MachineKind::Unknown;
    }
    if ["dell", "lenovo", "hewlett", " hp", "acer", "microsoft"]
        .iter()
        .any(|needle| m.contains(needle.trim()))
    {
        return MachineKind::Oem;
    }
    // ASUS/MSI can describe either a complete OEM machine or a retail motherboard. Require
    // independent product-family/model evidence and reject board-only identity as insufficient.
    if (m.contains("asus") || m.contains("asustek") || m.contains("micro-star") || m == "msi")
        && !family.is_empty()
        && !model.is_empty()
        && model != board
    {
        return MachineKind::Oem;
    }
    MachineKind::Unknown
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DriverAuthority {
    pub authority_type: AuthorityType,
    pub provider_id: String,
    pub display_name: String,
    pub trust_level: TrustLevel,
    pub machine_specificity: u8,
    pub device_specificity: u8,
    pub source_provenance: String,
    pub acquisition_capability: AcquisitionCapability,
    pub installation_capability: InstallationCapability,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProvenance {
    pub provider_id: String,
    pub provider_name: String,
    pub authority_type: AuthorityType,
    pub official_source: String,
    pub discovery_method: String,
    pub package_publisher: String,
    pub package_origin: String,
    pub catalog_identity: String,
    pub offer_identity: String,
    pub retrieved_unix_ms: i64,
    pub applicability_evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DriverCandidateV2 {
    pub candidate_id: String,
    pub authority: DriverAuthority,
    pub provenance: ProviderProvenance,
    pub device_privacy_key: String,
    pub matched_id: String,
    pub match_kind: MatchKind,
    pub title: String,
    pub target_version: String,
    pub driver_date_iso: String,
    pub driver_class: String,
    pub package_identity: String,
    pub architecture: String,
    pub supported_windows_build: String,
    pub applicability: ApplicabilityState,
    pub trust_state: PackageTrustState,
    pub recommendation_state: RecommendationState,
    pub recommendation_reasons: Vec<RecommendationReason>,
    pub acquisition_mode: AcquisitionCapability,
    pub installation_mode: InstallationCapability,
    pub reboot_expected: bool,
    pub firmware: bool,
    pub vendor_managed: bool,
    pub release_notes_url: String,
    pub official_support_url: String,
    pub update_id: String,
    pub revision: i32,
    pub min_download_bytes: u64,
    pub max_download_bytes: u64,
}

impl DriverCandidateV2 {
    pub fn executable(&self) -> bool {
        !self.firmware
            && self.trust_state.acceptable_for_recommendation()
            && !matches!(
                self.applicability,
                ApplicabilityState::Incompatible | ApplicabilityState::Unknown
            )
            && matches!(
                self.installation_mode,
                InstallationCapability::WindowsManaged | InstallationCapability::DirectTrusted
            )
            && matches!(
                self.recommendation_state,
                RecommendationState::Recommended | RecommendationState::Optional
            )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub provider_id: String,
    pub authority_type: AuthorityType,
    pub state: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthorityDecision {
    pub device_privacy_key: String,
    pub recommended_candidate_id: String,
    pub coverage: CoverageState,
    pub candidates: Vec<DriverCandidateV2>,
    pub reasons: Vec<RecommendationReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderQuery {
    pub device: DeviceIdentity,
    pub machine: MachineProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderResult {
    pub provider_id: String,
    pub coverage: CoverageState,
    pub candidates: Vec<DriverCandidateV2>,
    pub warnings: Vec<String>,
}

pub trait DriverProvider: Send + Sync {
    fn provider_id(&self) -> &'static str;
    fn authority_type(&self) -> AuthorityType;
    fn supports(&self, query: &ProviderQuery) -> bool;
    fn discover(&self, query: &ProviderQuery) -> std::result::Result<ProviderResult, String>;
}

#[derive(Debug, Clone)]
pub struct WindowsUpdateProvider;

impl WindowsUpdateProvider {
    pub fn from_offer(
        device: &DeviceIdentity,
        offer: &DriverOffer,
        match_kind: MatchKind,
    ) -> DriverCandidateV2 {
        let applicability = match match_kind {
            MatchKind::HardwareId => ApplicabilityState::Exact,
            MatchKind::CompatibleId => ApplicabilityState::Compatible,
            _ => ApplicabilityState::Unknown,
        };
        let matched = normalize_pnp_id(&offer.hardware_id);
        let mut hasher = Sha256::new();
        hasher.update(b"wua\0");
        hasher.update(offer.update_id.as_bytes());
        hasher.update(offer.revision.to_le_bytes());
        hasher.update(device.privacy_key.as_bytes());
        let candidate_id = hex::encode(hasher.finalize())[..24].to_string();
        let mut reasons = vec![RecommendationReason::WindowsApplicable];
        reasons.push(if match_kind == MatchKind::HardwareId {
            RecommendationReason::ExactHardwareIdMatch
        } else {
            RecommendationReason::CompatibleIdMatch
        });
        if device.problem_code == 28 {
            reasons.push(RecommendationReason::MissingDriverPriority);
        } else if device.problem_code != 0 {
            reasons.push(RecommendationReason::DeviceProblemPriority);
        }
        let firmware = offer.driver_class.eq_ignore_ascii_case("Firmware")
            || device.class_name.eq_ignore_ascii_case("Firmware");
        if firmware {
            reasons.push(RecommendationReason::FirmwareRequiresManualReview);
        }
        Self::normalize_version_reason(device, offer, &mut reasons);
        DriverCandidateV2 {
            candidate_id,
            authority: DriverAuthority {
                authority_type: AuthorityType::WindowsUpdate,
                provider_id: "microsoft.windows-update".into(),
                display_name: "Windows Update".into(),
                trust_level: TrustLevel::TrustedPlatform,
                machine_specificity: 70,
                device_specificity: if match_kind == MatchKind::HardwareId {
                    100
                } else {
                    80
                },
                source_provenance: "Windows Update Agent applicability result".into(),
                acquisition_capability: AcquisitionCapability::WindowsManaged,
                installation_capability: InstallationCapability::WindowsManaged,
            },
            provenance: ProviderProvenance {
                provider_id: "microsoft.windows-update".into(),
                provider_name: "Windows Update".into(),
                authority_type: AuthorityType::WindowsUpdate,
                official_source: "Microsoft Windows Update Agent".into(),
                discovery_method: "WUA Search IsInstalled=0 and Type=Driver".into(),
                package_publisher: offer.provider.clone(),
                package_origin: "Windows Update content service".into(),
                catalog_identity: format!("{}:{}", offer.update_id, offer.revision),
                offer_identity: format!("{}:{}:{}", offer.update_id, offer.revision, matched),
                retrieved_unix_ms: Utc::now().timestamp_millis(),
                applicability_evidence: vec![format!("{:?}:{}", match_kind, matched)],
            },
            device_privacy_key: device.privacy_key.clone(),
            matched_id: matched,
            match_kind,
            title: offer.title.clone(),
            target_version: offer.target_version.clone(),
            driver_date_iso: offer.driver_date_iso.clone(),
            driver_class: offer.driver_class.clone(),
            package_identity: format!("wua:{}:{}", offer.update_id, offer.revision),
            architecture: String::new(),
            supported_windows_build: String::new(),
            applicability,
            trust_state: PackageTrustState::WindowsManaged,
            recommendation_state: if firmware {
                RecommendationState::FirmwareProtected
            } else {
                RecommendationState::Recommended
            },
            recommendation_reasons: reasons,
            acquisition_mode: AcquisitionCapability::WindowsManaged,
            installation_mode: InstallationCapability::WindowsManaged,
            reboot_expected: false,
            firmware,
            vendor_managed: false,
            release_notes_url: offer.support_url.clone(),
            official_support_url: offer.support_url.clone(),
            update_id: offer.update_id.clone(),
            revision: offer.revision,
            min_download_bytes: offer.min_download_bytes,
            max_download_bytes: offer.max_download_bytes,
        }
    }

    fn normalize_version_reason(
        device: &DeviceIdentity,
        offer: &DriverOffer,
        reasons: &mut Vec<RecommendationReason>,
    ) {
        if device.current_version.is_empty() || offer.target_version.is_empty() {
            return;
        }
        match compare_driver_versions(&device.current_version, &offer.target_version) {
            VersionOrdering::Unknown => reasons.push(RecommendationReason::VersionOrderingUnknown),
            VersionOrdering::CandidateOlder | VersionOrdering::Equal => {
                if offer.target_version_source == VersionSource::TitleHeuristic {
                    reasons.push(RecommendationReason::InstalledDriverAlreadyPreferred);
                }
            }
            VersionOrdering::CandidateNewer => {}
        }
    }
}

impl DriverProvider for WindowsUpdateProvider {
    fn provider_id(&self) -> &'static str {
        "microsoft.windows-update"
    }
    fn authority_type(&self) -> AuthorityType {
        AuthorityType::WindowsUpdate
    }
    fn supports(&self, _query: &ProviderQuery) -> bool {
        true
    }
    fn discover(&self, _query: &ProviderQuery) -> std::result::Result<ProviderResult, String> {
        Err("WindowsUpdateProvider discovery is executed by the existing WUA collector and normalized through from_offer".into())
    }
}

pub trait DriverManagementProvider: Send + Sync {
    fn provider_id(&self) -> &'static str;
    fn supports(&self, query: &ProviderQuery) -> bool;
    fn guidance(&self, query: &ProviderQuery, installed: bool) -> DriverManagementAuthority;
}

#[derive(Debug, Clone)]
pub struct OfficialUtilityProvider {
    pub vendor: GpuVendor,
}

impl OfficialUtilityProvider {
    /// Phase 18.1 truth boundary: an official utility is management/discovery guidance, not a
    /// versioned DriverCandidateV2. It cannot enter candidate ranking or create an update count
    /// unless a separate provider later supplies concrete update evidence.
    pub fn management_authority(
        &self,
        device: &DeviceIdentity,
        installed: bool,
    ) -> DriverManagementAuthority {
        let p = gpu_policy(self.vendor);
        let _ = device; // device identity establishes applicability at the caller boundary.
        management_authority_for_gpu(self.vendor, installed, p.official_url, p.app_name)
    }
}

impl DriverManagementProvider for OfficialUtilityProvider {
    fn provider_id(&self) -> &'static str {
        "component.vendor-utility"
    }
    fn supports(&self, query: &ProviderQuery) -> bool {
        detect_gpu_vendor(&query.device) == Some(self.vendor)
    }
    fn guidance(&self, query: &ProviderQuery, installed: bool) -> DriverManagementAuthority {
        self.management_authority(&query.device, installed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionOrdering {
    CandidateNewer,
    CandidateOlder,
    Equal,
    Unknown,
}

pub fn compare_driver_versions(installed: &str, candidate: &str) -> VersionOrdering {
    let Some(a) = parse_numeric_version(installed) else {
        return VersionOrdering::Unknown;
    };
    let Some(b) = parse_numeric_version(candidate) else {
        return VersionOrdering::Unknown;
    };
    let width = a.len().max(b.len());
    let ordering = (0..width)
        .map(|i| (*a.get(i).unwrap_or(&0), *b.get(i).unwrap_or(&0)))
        .find_map(|(left, right)| match left.cmp(&right) {
            Ordering::Equal => None,
            other => Some(other),
        });
    match ordering.unwrap_or(Ordering::Equal) {
        Ordering::Less => VersionOrdering::CandidateNewer,
        Ordering::Greater => VersionOrdering::CandidateOlder,
        Ordering::Equal => VersionOrdering::Equal,
    }
}

fn parse_numeric_version(value: &str) -> Option<Vec<u64>> {
    let value = value.trim().trim_start_matches(['v', 'V']);
    if value.is_empty() || value.len() > 96 {
        return None;
    }
    let parts: Vec<&str> = value.split('.').collect();
    if !(2..=8).contains(&parts.len()) {
        return None;
    }
    let mut out = Vec::with_capacity(parts.len());
    for part in parts {
        if part.is_empty() || part.len() > 12 || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        out.push(part.parse().ok()?);
    }
    Some(out)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DriverOverrideKind {
    IgnoreExactVersion,
    RemindLater,
    IgnoreOptional,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DriverOverride {
    pub device_privacy_key: String,
    pub kind: DriverOverrideKind,
    pub candidate_version: String,
    pub authority_provider_id: String,
    pub expires_unix_ms: Option<i64>,
}

impl DriverOverride {
    pub fn suppresses(&self, candidate: &DriverCandidateV2, now_ms: i64) -> bool {
        if self.device_privacy_key != candidate.device_privacy_key {
            return false;
        }
        if self.expires_unix_ms.is_some_and(|v| v <= now_ms) {
            return false;
        }
        if self.authority_provider_id.trim().is_empty()
            || self.authority_provider_id != candidate.authority.provider_id
        {
            return false;
        }
        match self.kind {
            DriverOverrideKind::IgnoreExactVersion => {
                !self.candidate_version.is_empty()
                    && self.candidate_version == candidate.target_version
            }
            DriverOverrideKind::RemindLater => true,
            DriverOverrideKind::IgnoreOptional => {
                candidate.recommendation_state == RecommendationState::Optional
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AcquisitionPolicy {
    pub provider_id: String,
    pub official_https_hosts: BTreeSet<String>,
    pub content_https_hosts: BTreeSet<String>,
    pub max_redirects: u8,
    pub max_download_bytes: u64,
}

impl AcquisitionPolicy {
    pub fn validate_chain(&self, urls: &[String], content_length: Option<u64>) -> Result<()> {
        if urls.is_empty() || urls.len().saturating_sub(1) > self.max_redirects as usize {
            return Err(AuthorityError::UnofficialOrigin);
        }
        if content_length.is_some_and(|v| v > self.max_download_bytes) {
            return Err(AuthorityError::UnofficialOrigin);
        }
        for (index, url) in urls.iter().enumerate() {
            let host = https_host(url).ok_or(AuthorityError::UnofficialOrigin)?;
            let allowed = if index == 0 {
                self.official_https_hosts.contains(&host)
            } else {
                self.official_https_hosts.contains(&host)
                    || self.content_https_hosts.contains(&host)
            };
            if !allowed {
                return Err(AuthorityError::UnofficialOrigin);
            }
        }
        Ok(())
    }
}

fn https_host(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    let authority = rest.split('/').next()?;
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let host = authority
        .split(':')
        .next()?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host.is_empty() || host.contains('\\') || host.contains('%') {
        return None;
    }
    Some(host)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackageTrustEvidence {
    pub expected_sha256: String,
    pub observed_sha256: String,
    pub signer_subject: String,
    pub signer_thumbprint_or_identity: String,
    pub expected_publishers: Vec<String>,
    pub expected_signer_identities: Vec<String>,
    pub test_signed: bool,
    pub signature_state: PackageTrustState,
    pub inf_identity: String,
    pub device_applicable: bool,
}

impl PackageTrustEvidence {
    pub fn accepted(&self) -> bool {
        let digest_ok = self.expected_sha256.is_empty()
            || self
                .expected_sha256
                .eq_ignore_ascii_case(&self.observed_sha256);
        let signer_identity = self
            .signer_thumbprint_or_identity
            .chars()
            .filter(|c| !c.is_ascii_whitespace() && *c != ':')
            .flat_map(char::to_uppercase)
            .collect::<String>();
        let identity_ok = if self.expected_signer_identities.is_empty() {
            true
        } else {
            !signer_identity.is_empty()
                && self.expected_signer_identities.iter().any(|expected| {
                    expected
                        .chars()
                        .filter(|c| !c.is_ascii_whitespace() && *c != ':')
                        .flat_map(char::to_uppercase)
                        .collect::<String>()
                        == signer_identity
                })
        };
        let publisher_ok = if self.expected_publishers.is_empty() {
            true
        } else {
            self.expected_publishers
                .iter()
                .any(|v| self.signer_subject.trim().eq_ignore_ascii_case(v.trim()))
        };
        digest_ok
            && identity_ok
            && publisher_ok
            && !self.test_signed
            && self.device_applicable
            && self.signature_state.acceptable_for_recommendation()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SealedDriverAction {
    pub device_privacy_key: String,
    pub candidate_id: String,
    pub provider_id: String,
    pub authority_type: AuthorityType,
    pub update_id: String,
    pub revision: i32,
    pub target_version: String,
    pub package_identity: String,
    pub source_provenance: String,
    pub expected_trust_state: PackageTrustState,
    pub installation_mode: InstallationCapability,
    pub reboot_expected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImmutableDriverPlan {
    pub plan_id: String,
    pub scan_id: String,
    pub inventory_epoch: u64,
    pub machine_state_fingerprint: String,
    pub created_unix_ms: i64,
    pub actions: Vec<SealedDriverAction>,
    pub digest: String,
}

impl ImmutableDriverPlan {
    pub fn seal(
        plan_id: impl Into<String>,
        scan_id: impl Into<String>,
        inventory_epoch: u64,
        machine_state_fingerprint: impl Into<String>,
        candidates: &[DriverCandidateV2],
    ) -> Result<Self> {
        if candidates.is_empty() || candidates.iter().any(|c| !c.executable()) {
            return Err(AuthorityError::NotExecutable);
        }
        let mut actions: Vec<SealedDriverAction> = candidates
            .iter()
            .map(|c| SealedDriverAction {
                device_privacy_key: c.device_privacy_key.clone(),
                candidate_id: c.candidate_id.clone(),
                provider_id: c.authority.provider_id.clone(),
                authority_type: c.authority.authority_type,
                update_id: c.update_id.clone(),
                revision: c.revision,
                target_version: c.target_version.clone(),
                package_identity: c.package_identity.clone(),
                source_provenance: c.provenance.offer_identity.clone(),
                expected_trust_state: c.trust_state,
                installation_mode: c.installation_mode,
                reboot_expected: c.reboot_expected,
            })
            .collect();
        actions.sort_by(|a, b| {
            a.device_privacy_key
                .cmp(&b.device_privacy_key)
                .then_with(|| a.candidate_id.cmp(&b.candidate_id))
        });
        let plan_id = plan_id.into();
        let scan_id = scan_id.into();
        let machine_state_fingerprint = machine_state_fingerprint.into();
        let created_unix_ms = Utc::now().timestamp_millis();
        let digest = plan_digest(
            &plan_id,
            &scan_id,
            inventory_epoch,
            &machine_state_fingerprint,
            &actions,
        );
        Ok(Self {
            plan_id,
            scan_id,
            inventory_epoch,
            machine_state_fingerprint,
            created_unix_ms,
            actions,
            digest,
        })
    }

    pub fn revalidate(&self, candidates: &[DriverCandidateV2]) -> Result<()> {
        let current: BTreeMap<&str, &DriverCandidateV2> = candidates
            .iter()
            .map(|c| (c.candidate_id.as_str(), c))
            .collect();
        for action in &self.actions {
            let Some(candidate) = current.get(action.candidate_id.as_str()) else {
                return Err(AuthorityError::PlanInvalidated);
            };
            if candidate.device_privacy_key != action.device_privacy_key
                || candidate.authority.provider_id != action.provider_id
                || candidate.update_id != action.update_id
                || candidate.revision != action.revision
                || candidate.target_version != action.target_version
                || candidate.package_identity != action.package_identity
                || candidate.provenance.offer_identity != action.source_provenance
                || candidate.trust_state != action.expected_trust_state
                || candidate.installation_mode != action.installation_mode
            {
                return Err(AuthorityError::PlanInvalidated);
            }
        }
        Ok(())
    }
}

fn plan_digest(
    plan_id: &str,
    scan_id: &str,
    inventory_epoch: u64,
    machine_state_fingerprint: &str,
    actions: &[SealedDriverAction],
) -> String {
    let mut h = Sha256::new();
    for part in [POLICY_VERSION, plan_id, scan_id, machine_state_fingerprint] {
        h.update(part.as_bytes());
        h.update([0]);
    }
    h.update(inventory_epoch.to_le_bytes());
    for action in actions {
        for part in [
            &action.device_privacy_key,
            &action.candidate_id,
            &action.provider_id,
            &action.update_id,
            &action.target_version,
            &action.package_identity,
            &action.source_provenance,
        ] {
            h.update(part.as_bytes());
            h.update([0]);
        }
        h.update(action.revision.to_le_bytes());
    }
    hex::encode(h.finalize())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstallOrderItem {
    pub candidate_id: String,
    pub order_group: u8,
    pub requires_reboot_boundary: bool,
}

pub fn conservative_install_order(candidates: &[DriverCandidateV2]) -> Vec<InstallOrderItem> {
    let mut items: Vec<InstallOrderItem> = candidates
        .iter()
        .filter(|c| c.executable())
        .map(|c| {
            let class = c.driver_class.to_ascii_lowercase();
            let group = if class.contains("system") || class.contains("chipset") {
                10
            } else if class.contains("display") {
                30
            } else if class.contains("net") || class.contains("network") {
                40
            } else {
                20
            };
            InstallOrderItem {
                candidate_id: c.candidate_id.clone(),
                order_group: group,
                requires_reboot_boundary: c.reboot_expected,
            }
        })
        .collect();
    items.sort_by(|a, b| {
        a.order_group
            .cmp(&b.order_group)
            .then_with(|| a.candidate_id.cmp(&b.candidate_id))
    });
    items
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BatchItemResult {
    Installed,
    InstalledRebootPending,
    FailedBeforeMutation,
    FailedAfterMutation,
    SkippedDependency,
    CancelledBeforeMutation,
    RecoveryRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BatchContinuationDecision {
    pub continue_batch: bool,
    pub aggregate_reboot: bool,
    pub reason: String,
}

pub fn continuation_policy(
    result: BatchItemResult,
    next_depends_on_current: bool,
    reboot_boundary_required: bool,
) -> BatchContinuationDecision {
    match result {
        BatchItemResult::Installed => BatchContinuationDecision {
            continue_batch: !reboot_boundary_required,
            aggregate_reboot: reboot_boundary_required,
            reason: if reboot_boundary_required {
                "provider requires reboot boundary before further mutation"
            } else {
                "independent item verified"
            }
            .into(),
        },
        BatchItemResult::InstalledRebootPending => BatchContinuationDecision {
            continue_batch: !reboot_boundary_required && !next_depends_on_current,
            aggregate_reboot: true,
            reason: "reboot aggregated unless dependency/provider semantics require a boundary"
                .into(),
        },
        BatchItemResult::FailedBeforeMutation => BatchContinuationDecision {
            continue_batch: !next_depends_on_current,
            aggregate_reboot: false,
            reason: "no mutation occurred; independent items may continue".into(),
        },
        BatchItemResult::FailedAfterMutation | BatchItemResult::RecoveryRequired => {
            BatchContinuationDecision {
                continue_batch: false,
                aggregate_reboot: false,
                reason: "mutation outcome requires containment/recovery review".into(),
            }
        }
        BatchItemResult::SkippedDependency | BatchItemResult::CancelledBeforeMutation => {
            BatchContinuationDecision {
                continue_batch: false,
                aggregate_reboot: false,
                reason: "batch stopped by dependency/cancellation policy".into(),
            }
        }
    }
}

pub struct DriverAuthorityEngine;

impl DriverAuthorityEngine {
    pub fn evaluate(
        device: &DeviceIdentity,
        machine: &MachineProfile,
        mut candidates: Vec<DriverCandidateV2>,
        overrides: &[DriverOverride],
        coverage: CoverageState,
        now_ms: i64,
    ) -> AuthorityDecision {
        if candidates.len() > MAX_PROVIDER_CANDIDATES {
            candidates.truncate(MAX_PROVIDER_CANDIDATES);
        }
        for candidate in &mut candidates {
            if candidate.firmware {
                candidate.recommendation_state = RecommendationState::FirmwareProtected;
                push_reason(
                    &mut candidate.recommendation_reasons,
                    RecommendationReason::FirmwareRequiresManualReview,
                );
                continue;
            }
            if !candidate_has_authoritative_trust(candidate) {
                candidate.recommendation_state = RecommendationState::NotRecommended;
                push_reason(
                    &mut candidate.recommendation_reasons,
                    RecommendationReason::TrustRejected,
                );
                continue;
            }
            if matches!(
                candidate.applicability,
                ApplicabilityState::Incompatible | ApplicabilityState::Unknown
            ) {
                candidate.recommendation_state = RecommendationState::NotRecommended;
                push_reason(
                    &mut candidate.recommendation_reasons,
                    RecommendationReason::Incompatible,
                );
                continue;
            }
            if let Some(rule) = overrides
                .iter()
                .find(|rule| rule.suppresses(candidate, now_ms))
            {
                candidate.recommendation_state = RecommendationState::NotRecommended;
                push_reason(
                    &mut candidate.recommendation_reasons,
                    match rule.kind {
                        DriverOverrideKind::IgnoreExactVersion => {
                            RecommendationReason::UserIgnoredExactVersion
                        }
                        DriverOverrideKind::IgnoreOptional => {
                            RecommendationReason::UserIgnoredOptional
                        }
                        DriverOverrideKind::RemindLater => RecommendationReason::UserDeferred,
                    },
                );
            }
        }

        candidates.sort_by(|a, b| compare_candidate_rank(device, machine, b, a));
        let recommended_idx = candidates.iter().position(candidate_can_recommend);
        if let Some(index) = recommended_idx {
            let recommended_snapshot = candidates[index].clone();
            for (i, candidate) in candidates.iter_mut().enumerate() {
                if !candidate_can_recommend(candidate) {
                    continue;
                }
                candidate.recommendation_state = if i == index {
                    RecommendationState::Recommended
                } else {
                    RecommendationState::Alternative
                };
                if i != index && is_newer_than(candidate, &recommended_snapshot) {
                    push_reason(
                        &mut candidate.recommendation_reasons,
                        RecommendationReason::NewerButLowerAuthority,
                    );
                }
            }
        }
        if coverage != CoverageState::CompleteForRequiredAuthorities {
            for c in &mut candidates {
                push_reason(
                    &mut c.recommendation_reasons,
                    RecommendationReason::AuthorityCoverageIncomplete,
                );
            }
        }
        let recommended_candidate_id = recommended_idx
            .and_then(|i| candidates.get(i))
            .map(|c| c.candidate_id.clone())
            .unwrap_or_default();
        let mut reasons = recommended_idx
            .and_then(|i| candidates.get(i))
            .map(|c| c.recommendation_reasons.clone())
            .unwrap_or_default();
        reasons.sort();
        reasons.dedup();
        AuthorityDecision {
            device_privacy_key: device.privacy_key.clone(),
            recommended_candidate_id,
            coverage,
            candidates,
            reasons,
        }
    }
}

fn candidate_can_recommend(c: &DriverCandidateV2) -> bool {
    candidate_has_authoritative_trust(c)
        && !c.firmware
        && !matches!(
            c.applicability,
            ApplicabilityState::Incompatible | ApplicabilityState::Unknown
        )
        && !matches!(c.recommendation_state, RecommendationState::NotRecommended)
}

fn candidate_has_authoritative_trust(c: &DriverCandidateV2) -> bool {
    // Management guidance never reaches this path. Concrete driver candidates require package
    // trust evidence; a trusted vendor name or an HTTPS support page is not package trust.
    c.trust_state.acceptable_for_recommendation()
}

fn compare_candidate_rank(
    device: &DeviceIdentity,
    machine: &MachineProfile,
    a: &DriverCandidateV2,
    b: &DriverCandidateV2,
) -> Ordering {
    trust_score(a)
        .cmp(&trust_score(b))
        .then_with(|| applicability_score(a).cmp(&applicability_score(b)))
        .then_with(|| {
            authority_context_score(device, machine, a)
                .cmp(&authority_context_score(device, machine, b))
        })
        .then_with(|| {
            a.authority
                .device_specificity
                .cmp(&b.authority.device_specificity)
        })
        .then_with(|| {
            a.authority
                .machine_specificity
                .cmp(&b.authority.machine_specificity)
        })
        .then_with(|| version_score(device, a).cmp(&version_score(device, b)))
        .then_with(|| b.candidate_id.cmp(&a.candidate_id))
}

fn trust_score(c: &DriverCandidateV2) -> u16 {
    match c.trust_state {
        PackageTrustState::TrustedSignatureAndDigest => 500,
        PackageTrustState::WindowsManaged => 490,
        PackageTrustState::TrustedSignature => 480,
        PackageTrustState::NotApplicable if candidate_has_authoritative_trust(c) => 490,
        _ => 0,
    }
}
fn applicability_score(c: &DriverCandidateV2) -> u16 {
    match c.applicability {
        ApplicabilityState::Exact => 400,
        ApplicabilityState::Compatible => 320,
        ApplicabilityState::Contextual => 240,
        _ => 0,
    }
}
fn authority_context_score(
    _device: &DeviceIdentity,
    machine: &MachineProfile,
    c: &DriverCandidateV2,
) -> u16 {
    match c.authority.authority_type {
        AuthorityType::Oem
            if machine.machine_kind == MachineKind::Oem
                && c.authority.machine_specificity >= 80 =>
        {
            390
        }
        AuthorityType::WindowsUpdate => 360,
        AuthorityType::ComponentVendor if machine.machine_kind == MachineKind::SelfBuilt => 375,
        AuthorityType::ComponentVendor if machine.machine_kind == MachineKind::Unknown => 335,
        AuthorityType::ComponentVendor => 300,
        AuthorityType::Oem => 320,
        // Vendor utilities are management authorities, not concrete package candidates. Keep these
        // values fail-low only for legacy/deserialized data; new source never creates such candidates.
        AuthorityType::VendorUtility => 100,
        AuthorityType::ManualOfficial => 180,
    }
}
fn version_score(device: &DeviceIdentity, c: &DriverCandidateV2) -> u16 {
    match compare_driver_versions(&device.current_version, &c.target_version) {
        VersionOrdering::CandidateNewer => 40,
        VersionOrdering::Equal => 30,
        VersionOrdering::Unknown => 20,
        VersionOrdering::CandidateOlder => 10,
    }
}
fn is_newer_than(a: &DriverCandidateV2, b: &DriverCandidateV2) -> bool {
    compare_driver_versions(&b.target_version, &a.target_version) == VersionOrdering::CandidateNewer
}
fn push_reason(values: &mut Vec<RecommendationReason>, reason: RecommendationReason) {
    if !values.contains(&reason) {
        values.push(reason);
    }
}

pub fn detect_gpu_vendor(device: &DeviceIdentity) -> Option<GpuVendor> {
    let ids = device
        .hardware_ids
        .iter()
        .chain(device.compatible_ids.iter());
    for id in ids {
        let upper = normalize_pnp_id(id);
        if upper.contains("VEN_10DE") {
            return Some(GpuVendor::Nvidia);
        }
        if upper.contains("VEN_1002") {
            return Some(GpuVendor::Amd);
        }
        if upper.contains("VEN_8086") {
            return Some(GpuVendor::Intel);
        }
    }
    None
}

fn parse_best_hardware_id(values: &[String]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Some(raw) = values
        .iter()
        .map(|v| normalize_pnp_id(v))
        .find(|v| !v.is_empty())
    else {
        return out;
    };
    for token in raw.split(['\\', '&']) {
        if let Some(v) = token.strip_prefix("VEN_") {
            out.entry("VEN".into()).or_insert_with(|| v.into());
        } else if let Some(v) = token.strip_prefix("DEV_") {
            out.entry("DEV".into()).or_insert_with(|| v.into());
        } else if let Some(v) = token.strip_prefix("REV_") {
            out.entry("REV".into()).or_insert_with(|| v.into());
        } else if let Some(v) = token.strip_prefix("SUBSYS_")
            && v.len() >= 8
        {
            out.entry("SUBSYS_DEVICE".into())
                .or_insert_with(|| v[..4].into());
            out.entry("SUBSYS_VENDOR".into())
                .or_insert_with(|| v[4..8].into());
        }
    }
    out
}

pub fn recovery_available(
    current_driver: Option<&InstalledDriver>,
    backup_available: bool,
    restore_point_available: bool,
) -> bool {
    current_driver.is_some() && (backup_available || restore_point_available)
}

pub fn recommended_update_all(decisions: &[AuthorityDecision]) -> Vec<String> {
    let mut ids: Vec<String> = decisions
        .iter()
        .flat_map(|d| d.candidates.iter())
        .filter(|c| {
            c.recommendation_state == RecommendationState::Recommended
                && c.executable()
                && !c.firmware
        })
        .map(|c| c.candidate_id.clone())
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

pub fn default_defer_until(now_ms: i64) -> i64 {
    now_ms.saturating_add(Duration::from_secs(7 * 24 * 60 * 60).as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device() -> DeviceIdentity {
        DeviceIdentity {
            privacy_key: "dev".into(),
            hardware_ids: vec!["PCI\\VEN_1234&DEV_5678".into()],
            class_name: "Net".into(),
            current_version: "1.0.0.0".into(),
            ..Default::default()
        }
    }
    fn candidate(id: &str, authority: AuthorityType, version: &str) -> DriverCandidateV2 {
        DriverCandidateV2 {
            candidate_id: id.into(),
            authority: DriverAuthority {
                authority_type: authority,
                provider_id: format!("p-{id}"),
                display_name: id.into(),
                trust_level: TrustLevel::TrustedVendor,
                machine_specificity: if authority == AuthorityType::Oem {
                    100
                } else {
                    40
                },
                device_specificity: 100,
                source_provenance: "official".into(),
                acquisition_capability: AcquisitionCapability::DirectTrusted,
                installation_capability: InstallationCapability::DirectTrusted,
            },
            provenance: ProviderProvenance {
                provider_id: format!("p-{id}"),
                provider_name: id.into(),
                authority_type: authority,
                official_source: "https://vendor.example".into(),
                discovery_method: "fixture".into(),
                package_publisher: "Vendor".into(),
                package_origin: "https://vendor.example".into(),
                catalog_identity: id.into(),
                offer_identity: id.into(),
                retrieved_unix_ms: 1,
                applicability_evidence: vec!["exact".into()],
            },
            device_privacy_key: "dev".into(),
            matched_id: "PCI\\VEN_1234&DEV_5678".into(),
            match_kind: MatchKind::HardwareId,
            title: id.into(),
            target_version: version.into(),
            driver_date_iso: "2026-01-01".into(),
            driver_class: "Net".into(),
            package_identity: id.into(),
            architecture: "x64".into(),
            supported_windows_build: "".into(),
            applicability: ApplicabilityState::Exact,
            trust_state: PackageTrustState::TrustedSignatureAndDigest,
            recommendation_state: RecommendationState::Optional,
            recommendation_reasons: vec![],
            acquisition_mode: AcquisitionCapability::DirectTrusted,
            installation_mode: InstallationCapability::DirectTrusted,
            reboot_expected: false,
            firmware: false,
            vendor_managed: false,
            release_notes_url: "".into(),
            official_support_url: "https://vendor.example".into(),
            update_id: "".into(),
            revision: 0,
            min_download_bytes: 0,
            max_download_bytes: 100,
        }
    }

    #[test]
    fn dotted_versions_are_provider_agnostic_not_semver() {
        assert_eq!(
            compare_driver_versions("31.0.101.5590", "32.0.1.7"),
            VersionOrdering::CandidateNewer
        );
        assert_eq!(
            compare_driver_versions("31.0.rc1", "32.0.1"),
            VersionOrdering::Unknown
        );
    }
    #[test]
    fn exact_ignore_does_not_suppress_future_version() {
        let mut c = candidate("x", AuthorityType::WindowsUpdate, "2.0.0.0");
        let rule = DriverOverride {
            device_privacy_key: "dev".into(),
            kind: DriverOverrideKind::IgnoreExactVersion,
            candidate_version: "1.5.0.0".into(),
            authority_provider_id: "p-x".into(),
            expires_unix_ms: None,
        };
        assert!(!rule.suppresses(&c, 10));
        c.target_version = "1.5.0.0".into();
        assert!(rule.suppresses(&c, 10));
    }
    #[test]
    fn firmware_never_enters_update_all() {
        let mut c = candidate("fw", AuthorityType::WindowsUpdate, "2.0.0.0");
        c.firmware = true;
        let d = DriverAuthorityEngine::evaluate(
            &device(),
            &MachineProfile::default(),
            vec![c],
            &[],
            CoverageState::CompleteForRequiredAuthorities,
            1,
        );
        assert!(recommended_update_all(&[d]).is_empty());
    }
    #[test]
    fn unsigned_cannot_outrank_trusted() {
        let mut unsigned = candidate("unsigned", AuthorityType::Oem, "9.0.0.0");
        unsigned.trust_state = PackageTrustState::Unsigned;
        let trusted = candidate("trusted", AuthorityType::WindowsUpdate, "2.0.0.0");
        let d = DriverAuthorityEngine::evaluate(
            &device(),
            &MachineProfile {
                machine_kind: MachineKind::Oem,
                ..Default::default()
            },
            vec![unsigned, trusted],
            &[],
            CoverageState::CompleteForRequiredAuthorities,
            1,
        );
        assert_eq!(d.recommended_candidate_id, "trusted");
    }
    #[test]
    fn d18_07_unsigned_candidate_is_rejected_with_trust_reason() {
        let mut unsigned = candidate("unsigned", AuthorityType::Oem, "9.0.0.0");
        unsigned.trust_state = PackageTrustState::Unsigned;
        let d = DriverAuthorityEngine::evaluate(
            &device(),
            &MachineProfile {
                machine_kind: MachineKind::Oem,
                ..Default::default()
            },
            vec![unsigned],
            &[],
            CoverageState::CompleteForRequiredAuthorities,
            1,
        );
        assert!(d.recommended_candidate_id.is_empty());
        assert_eq!(
            d.candidates[0].recommendation_state,
            RecommendationState::NotRecommended
        );
        assert!(
            d.candidates[0]
                .recommendation_reasons
                .contains(&RecommendationReason::TrustRejected)
        );
    }
    #[test]
    fn machine_specific_oem_can_beat_newer_generic() {
        let oem = candidate("oem", AuthorityType::Oem, "2.0.0.0");
        let generic = candidate("generic", AuthorityType::ComponentVendor, "9.0.0.0");
        let d = DriverAuthorityEngine::evaluate(
            &device(),
            &MachineProfile {
                machine_kind: MachineKind::Oem,
                ..Default::default()
            },
            vec![generic, oem],
            &[],
            CoverageState::CompleteForRequiredAuthorities,
            1,
        );
        assert_eq!(d.recommended_candidate_id, "oem");
    }
    #[test]
    fn component_vendor_is_contextually_strong_on_self_built() {
        let component = candidate("component", AuthorityType::ComponentVendor, "2.0.0.0");
        let oem = candidate("oem", AuthorityType::Oem, "2.1.0.0");
        let d = DriverAuthorityEngine::evaluate(
            &device(),
            &MachineProfile {
                machine_kind: MachineKind::SelfBuilt,
                ..Default::default()
            },
            vec![oem, component],
            &[],
            CoverageState::CompleteForRequiredAuthorities,
            1,
        );
        assert_eq!(d.recommended_candidate_id, "component");
    }
    #[test]
    fn provider_failure_prevents_complete_up_to_date_claim() {
        let d = DriverAuthorityEngine::evaluate(
            &device(),
            &MachineProfile::default(),
            vec![],
            &[],
            CoverageState::Partial,
            1,
        );
        assert_eq!(d.coverage, CoverageState::Partial);
        assert!(d.recommended_candidate_id.is_empty());
    }
    #[test]
    fn hostile_redirect_fails_closed() {
        let p = AcquisitionPolicy {
            provider_id: "x".into(),
            official_https_hosts: ["vendor.example".into()].into_iter().collect(),
            content_https_hosts: ["cdn.vendor.example".into()].into_iter().collect(),
            max_redirects: 2,
            max_download_bytes: 1000,
        };
        assert!(
            p.validate_chain(
                &[
                    "https://vendor.example/a".into(),
                    "https://attacker.example/b".into()
                ],
                Some(10)
            )
            .is_err()
        );
    }
    #[test]
    fn sealed_plan_detects_candidate_substitution() {
        let c = candidate("a", AuthorityType::WindowsUpdate, "2.0.0.0");
        let p = ImmutableDriverPlan::seal("p", "s", 1, "m", std::slice::from_ref(&c)).unwrap();
        let mut changed = c.clone();
        changed.package_identity = "evil".into();
        assert_eq!(
            p.revalidate(&[changed]),
            Err(AuthorityError::PlanInvalidated)
        );
    }
    #[test]
    fn ignore_exact_version_is_provider_scoped() {
        let a = candidate("a", AuthorityType::WindowsUpdate, "2.0.0.0");
        let b = candidate("b", AuthorityType::ComponentVendor, "2.0.0.0");
        let rule = DriverOverride {
            device_privacy_key: "dev".into(),
            kind: DriverOverrideKind::IgnoreExactVersion,
            candidate_version: "2.0.0.0".into(),
            authority_provider_id: "p-a".into(),
            expires_unix_ms: None,
        };
        assert!(rule.suppresses(&a, 10));
        assert!(!rule.suppresses(&b, 10));
    }
    #[test]
    fn empty_provider_override_fails_closed() {
        let c = candidate("a", AuthorityType::WindowsUpdate, "2.0.0.0");
        let rule = DriverOverride {
            device_privacy_key: "dev".into(),
            kind: DriverOverrideKind::IgnoreExactVersion,
            candidate_version: "2.0.0.0".into(),
            authority_provider_id: String::new(),
            expires_unix_ms: None,
        };
        assert!(!rule.suppresses(&c, 10));
    }
    #[test]
    fn ranking_policy_is_deterministic_for_machine_context() {
        let cases = [
            (
                MachineKind::Oem,
                AuthorityType::Oem,
                AuthorityType::ComponentVendor,
                "preferred",
            ),
            (
                MachineKind::SelfBuilt,
                AuthorityType::ComponentVendor,
                AuthorityType::Oem,
                "preferred",
            ),
            (
                MachineKind::Unknown,
                AuthorityType::WindowsUpdate,
                AuthorityType::ComponentVendor,
                "preferred",
            ),
        ];
        for (machine_kind, preferred_authority, other_authority, expected) in cases {
            let preferred = candidate(expected, preferred_authority, "2.0.0.0");
            let other = candidate("other", other_authority, "2.0.0.0");
            let decision = DriverAuthorityEngine::evaluate(
                &device(),
                &MachineProfile {
                    machine_kind,
                    ..Default::default()
                },
                vec![other, preferred],
                &[],
                CoverageState::CompleteForRequiredAuthorities,
                1,
            );
            assert_eq!(decision.recommended_candidate_id, expected);
        }
    }
    #[test]
    fn trust_precedes_context_for_actual_packages() {
        let mut bad = candidate("bad", AuthorityType::Oem, "9.0.0.0");
        bad.trust_state = PackageTrustState::UnexpectedPublisher;
        let good = candidate("good", AuthorityType::WindowsUpdate, "2.0.0.0");
        let decision = DriverAuthorityEngine::evaluate(
            &device(),
            &MachineProfile {
                machine_kind: MachineKind::Oem,
                ..Default::default()
            },
            vec![bad, good],
            &[],
            CoverageState::CompleteForRequiredAuthorities,
            1,
        );
        assert_eq!(decision.recommended_candidate_id, "good");
    }
    #[test]
    fn d18_06_multiple_official_candidates_keep_one_recommended_and_alternatives() {
        let a = candidate("a", AuthorityType::WindowsUpdate, "2.0.0.0");
        let b = candidate("b", AuthorityType::ComponentVendor, "2.1.0.0");
        let decision = DriverAuthorityEngine::evaluate(
            &device(),
            &MachineProfile {
                machine_kind: MachineKind::SelfBuilt,
                ..Default::default()
            },
            vec![a, b],
            &[],
            CoverageState::CompleteForRequiredAuthorities,
            1,
        );
        assert_eq!(
            decision
                .candidates
                .iter()
                .filter(|c| c.recommendation_state == RecommendationState::Recommended)
                .count(),
            1
        );
        assert_eq!(
            decision
                .candidates
                .iter()
                .filter(|c| c.recommendation_state == RecommendationState::Alternative)
                .count(),
            1
        );
    }
    #[test]
    fn d18_15_recovery_is_available_only_with_installed_driver_and_recovery_evidence() {
        let installed = InstalledDriver {
            provider: "Vendor".into(),
            version: "1".into(),
            inf_path: "oem1.inf".into(),
            date: "".into(),
        };
        assert!(recovery_available(Some(&installed), true, false));
        assert!(recovery_available(Some(&installed), false, true));
        assert!(!recovery_available(Some(&installed), false, false));
        assert!(!recovery_available(None, true, true));
    }
    #[test]
    fn d18_16_reboot_aggregation_preserves_independent_batch_progress() {
        let d = continuation_policy(BatchItemResult::InstalledRebootPending, false, false);
        assert!(d.continue_batch);
        assert!(d.aggregate_reboot);
        let boundary = continuation_policy(BatchItemResult::InstalledRebootPending, false, true);
        assert!(!boundary.continue_batch);
        assert!(boundary.aggregate_reboot);
    }
    #[test]
    fn batch_failure_after_mutation_stops_following_items() {
        assert!(
            !continuation_policy(BatchItemResult::FailedAfterMutation, false, false).continue_batch
        );
    }
}
