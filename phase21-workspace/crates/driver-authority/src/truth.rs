use serde::{Deserialize, Serialize};

use super::{
    AcquisitionCapability, AuthorityType, CoverageState, DeviceIdentity, GpuVendor,
    InstallationCapability, MachineProfile, ProviderAuthorityClass, ProviderImplementationStatus,
    ProviderRegistry,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum ManagementAuthorityAvailability {
    Available,
    Installed,
    NotInstalled,
    ProviderUnavailable,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum UpdateAvailabilityEvidence {
    UnknownUntilVendorCheck,
    UpdateAvailable,
    NoUpdateReported,
    ProviderUnavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DriverManagementAuthority {
    pub provider_id: String,
    pub authority_type: AuthorityType,
    pub display_name: String,
    pub official_authority: String,
    pub official_url: String,
    pub availability: ManagementAuthorityAvailability,
    pub update_availability: UpdateAvailabilityEvidence,
    pub acquisition_capability: AcquisitionCapability,
    pub installation_capability: InstallationCapability,
    pub update_evidence_candidate_id: String,
}

impl DriverManagementAuthority {
    pub fn proves_update_available(&self) -> bool {
        self.update_availability == UpdateAvailabilityEvidence::UpdateAvailable
            && !self.update_evidence_candidate_id.trim().is_empty()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum AuthorityEvaluationState {
    Evaluated,
    PartiallyEvaluated,
    Offline,
    Unavailable,
    Unsupported,
    ManualRequired,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthorityRequirement {
    pub provider_id: String,
    pub authority_type: AuthorityType,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthorityEvaluation {
    pub provider_id: String,
    pub authority_type: AuthorityType,
    pub state: AuthorityEvaluationState,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeviceAuthorityCoverage {
    pub required_authorities: Vec<String>,
    pub evaluated_authorities: Vec<String>,
    pub unavailable_authorities: Vec<String>,
    pub unsupported_authorities: Vec<String>,
    pub manual_authorities: Vec<String>,
    pub completeness: CoverageState,
}

impl DeviceAuthorityCoverage {
    pub fn from_evaluations(
        required: &[AuthorityRequirement],
        evaluations: &[AuthorityEvaluation],
    ) -> Self {
        let mut out = Self {
            required_authorities: required.iter().map(|r| r.provider_id.clone()).collect(),
            completeness: CoverageState::Unknown,
            ..Self::default()
        };
        let mut saw_partial = false;
        let mut saw_offline = false;
        let mut saw_unavailable = false;
        let mut saw_manual = false;
        let mut saw_unknown = false;
        for req in required {
            let eval = evaluations
                .iter()
                .find(|e| e.provider_id == req.provider_id);
            match eval
                .map(|e| e.state)
                .unwrap_or(AuthorityEvaluationState::Unknown)
            {
                AuthorityEvaluationState::Evaluated => {
                    out.evaluated_authorities.push(req.provider_id.clone())
                }
                AuthorityEvaluationState::PartiallyEvaluated => {
                    out.evaluated_authorities.push(req.provider_id.clone());
                    saw_partial = true;
                }
                AuthorityEvaluationState::Offline => {
                    out.unavailable_authorities.push(req.provider_id.clone());
                    saw_offline = true;
                }
                AuthorityEvaluationState::Unavailable => {
                    out.unavailable_authorities.push(req.provider_id.clone());
                    saw_unavailable = true;
                }
                AuthorityEvaluationState::Unsupported => {
                    out.unsupported_authorities.push(req.provider_id.clone());
                    saw_partial = true;
                }
                AuthorityEvaluationState::ManualRequired => {
                    out.manual_authorities.push(req.provider_id.clone());
                    saw_manual = true;
                }
                AuthorityEvaluationState::Unknown => {
                    out.unavailable_authorities.push(req.provider_id.clone());
                    saw_unknown = true;
                }
            }
        }
        out.completeness = if required.is_empty() || saw_unknown {
            CoverageState::Unknown
        } else if saw_offline {
            CoverageState::Offline
        } else if saw_unavailable {
            CoverageState::ProviderUnavailable
        } else if saw_partial {
            CoverageState::Partial
        } else if saw_manual {
            CoverageState::ManualAuthorityRequired
        } else {
            CoverageState::CompleteForRequiredAuthorities
        };
        out
    }

    pub fn permits_strong_up_to_date(&self) -> bool {
        self.completeness == CoverageState::CompleteForRequiredAuthorities
            && !self.required_authorities.is_empty()
            && self
                .required_authorities
                .iter()
                .all(|id| self.evaluated_authorities.contains(id))
    }
}

pub fn required_authorities(
    device: &DeviceIdentity,
    machine: &MachineProfile,
    registry: &ProviderRegistry,
) -> Vec<AuthorityRequirement> {
    let mut required = Vec::new();
    if registry.windows_update().is_some() {
        required.push(AuthorityRequirement {
            provider_id: "microsoft.windows-update".into(),
            authority_type: AuthorityType::WindowsUpdate,
            reason: "platform driver authority".into(),
        });
    }
    if let Some(oem) = registry.oem_provider_for_machine(machine) {
        required.push(AuthorityRequirement {
            provider_id: oem.provider_id.clone(),
            authority_type: AuthorityType::Oem,
            reason: "machine OEM authority applies".into(),
        });
    }
    for component in registry.component_providers_for_device(device) {
        required.push(AuthorityRequirement {
            provider_id: component.provider_id.clone(),
            authority_type: AuthorityType::ComponentVendor,
            reason: "component vendor authority applies".into(),
        });
    }
    required.sort_by(|a, b| a.provider_id.cmp(&b.provider_id));
    required.dedup_by(|a, b| a.provider_id == b.provider_id);
    required
}

pub fn evaluate_required_authorities(
    required: &[AuthorityRequirement],
    registry: &ProviderRegistry,
    windows_update_state: CoverageState,
) -> Vec<AuthorityEvaluation> {
    required.iter().map(|req| {
        if req.provider_id == "microsoft.windows-update" {
            let (state, detail) = match windows_update_state {
                CoverageState::CompleteForRequiredAuthorities => (AuthorityEvaluationState::Evaluated, "Windows Update query completed"),
                CoverageState::Partial => (AuthorityEvaluationState::PartiallyEvaluated, "Windows Update returned partial/rejected metadata"),
                CoverageState::Offline => (AuthorityEvaluationState::Offline, "Windows Update could not be evaluated while offline"),
                CoverageState::ProviderUnavailable => (AuthorityEvaluationState::Unavailable, "Windows Update provider unavailable"),
                CoverageState::ManualAuthorityRequired => (AuthorityEvaluationState::Unknown, "invalid Windows Update state"),
                CoverageState::Unknown => (AuthorityEvaluationState::Unknown, "Windows Update evaluation state unknown"),
            };
            return AuthorityEvaluation { provider_id:req.provider_id.clone(), authority_type:req.authority_type, state, detail:detail.into() };
        }
        let Some(provider) = registry.by_id(&req.provider_id) else {
            return AuthorityEvaluation { provider_id:req.provider_id.clone(), authority_type:req.authority_type, state:AuthorityEvaluationState::Unsupported, detail:"required authority missing from provider registry".into() };
        };
        let state = match provider.implementation_status {
            ProviderImplementationStatus::Implemented if provider.automatically_evaluates_update_availability() => AuthorityEvaluationState::Unknown,
            ProviderImplementationStatus::OfficialUtility | ProviderImplementationStatus::ManualOfficial => AuthorityEvaluationState::ManualRequired,
            ProviderImplementationStatus::UnsupportedAutomation => AuthorityEvaluationState::Unsupported,
            ProviderImplementationStatus::Implemented => AuthorityEvaluationState::Unknown,
        };
        let detail = match state {
            AuthorityEvaluationState::ManualRequired => "official authority exists but update availability requires an explicit vendor/OEM check",
            AuthorityEvaluationState::Unsupported => "automatic provider adapter is not implemented; manual official fallback only",
            _ => "provider capability exists but was not evaluated by this scan",
        };
        AuthorityEvaluation { provider_id:req.provider_id.clone(), authority_type:req.authority_type, state, detail:detail.into() }
    }).collect()
}

pub fn management_authority_for_gpu(
    vendor: GpuVendor,
    app_installed: bool,
    official_url: &str,
    app_name: &str,
) -> DriverManagementAuthority {
    let provider_id = match vendor {
        GpuVendor::Nvidia => "component.nvidia",
        GpuVendor::Amd => "component.amd",
        GpuVendor::Intel => "component.intel",
    };
    DriverManagementAuthority {
        provider_id: provider_id.into(),
        authority_type: AuthorityType::ComponentVendor,
        display_name: app_name.into(),
        official_authority: format!("{} official update authority", vendor.as_str()),
        official_url: official_url.into(),
        availability: if app_installed {
            ManagementAuthorityAvailability::Installed
        } else {
            ManagementAuthorityAvailability::NotInstalled
        },
        update_availability: UpdateAvailabilityEvidence::UnknownUntilVendorCheck,
        acquisition_capability: AcquisitionCapability::OfficialUtility,
        installation_capability: InstallationCapability::OfficialUtility,
        update_evidence_candidate_id: String::new(),
    }
}

pub fn management_authority_for_registry_provider(
    provider_id: &str,
    registry: &ProviderRegistry,
) -> Option<DriverManagementAuthority> {
    let provider = registry.by_id(provider_id)?;
    if provider.authority_class == ProviderAuthorityClass::WindowsUpdate {
        return None;
    }
    let authority_type = match provider.authority_class {
        ProviderAuthorityClass::Oem => AuthorityType::Oem,
        ProviderAuthorityClass::ComponentVendor => AuthorityType::ComponentVendor,
        ProviderAuthorityClass::VendorUtility => AuthorityType::VendorUtility,
        ProviderAuthorityClass::ManualOfficial => AuthorityType::ManualOfficial,
        ProviderAuthorityClass::WindowsUpdate => AuthorityType::WindowsUpdate,
    };
    let (acquisition, installation) = match provider.implementation_status {
        ProviderImplementationStatus::OfficialUtility => (
            AcquisitionCapability::OfficialUtility,
            InstallationCapability::OfficialUtility,
        ),
        ProviderImplementationStatus::ManualOfficial => (
            AcquisitionCapability::ManualOfficial,
            InstallationCapability::ManualOfficial,
        ),
        ProviderImplementationStatus::Implemented => (
            AcquisitionCapability::Unsupported,
            InstallationCapability::Unsupported,
        ),
        ProviderImplementationStatus::UnsupportedAutomation => (
            AcquisitionCapability::Unsupported,
            InstallationCapability::Unsupported,
        ),
    };
    Some(DriverManagementAuthority {
        provider_id: provider.provider_id.clone(),
        authority_type,
        display_name: provider.provider_family.clone(),
        official_authority: provider.official_authority.clone(),
        official_url: String::new(),
        availability: ManagementAuthorityAvailability::Available,
        update_availability: UpdateAvailabilityEvidence::UnknownUntilVendorCheck,
        acquisition_capability: acquisition,
        installation_capability: installation,
        update_evidence_candidate_id: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MachineKind, builtin_provider_registry};

    #[test]
    fn utility_presence_never_proves_update_availability() {
        let authority = management_authority_for_gpu(
            GpuVendor::Nvidia,
            true,
            "https://www.nvidia.com/",
            "NVIDIA App",
        );
        assert_eq!(
            authority.availability,
            ManagementAuthorityAvailability::Installed
        );
        assert_eq!(
            authority.update_availability,
            UpdateAvailabilityEvidence::UnknownUntilVendorCheck
        );
        assert!(!authority.proves_update_available());
    }

    #[test]
    fn oem_and_gpu_require_device_aware_authority_coverage() {
        let registry = builtin_provider_registry().unwrap();
        let device = DeviceIdentity {
            class_name: "Display".into(),
            vendor_id: "10DE".into(),
            ..Default::default()
        };
        let machine = MachineProfile {
            manufacturer: "Dell Inc.".into(),
            machine_kind: MachineKind::Oem,
            ..Default::default()
        };
        let required = required_authorities(&device, &machine, &registry);
        assert!(
            required
                .iter()
                .any(|r| r.provider_id == "microsoft.windows-update")
        );
        assert!(required.iter().any(|r| r.provider_id == "oem.dell"));
        assert!(required.iter().any(|r| r.provider_id == "component.nvidia"));
        let eval = evaluate_required_authorities(
            &required,
            &registry,
            CoverageState::CompleteForRequiredAuthorities,
        );
        let coverage = DeviceAuthorityCoverage::from_evaluations(&required, &eval);
        assert_eq!(
            coverage.completeness,
            CoverageState::ManualAuthorityRequired
        );
        assert!(!coverage.permits_strong_up_to_date());
    }
}
