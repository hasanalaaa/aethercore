use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{AuthorityError, DeviceIdentity, MachineKind, MachineProfile, Result};

pub const PROVIDER_COVERAGE_SCHEMA: &str = "aethercore.driver-provider-coverage.v1";
pub const BUILTIN_PROVIDER_COVERAGE_JSON: &str = include_str!("../../../DRIVER_PROVIDER_COVERAGE.json");

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "PascalCase")]
pub enum ProviderAuthorityClass { WindowsUpdate, Oem, ComponentVendor, VendorUtility, ManualOfficial }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "PascalCase")]
pub enum ProviderDiscoveryCapability { MachineReadable, OfficialUtility, ManualOfficial }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "PascalCase")]
pub enum ProviderUpdateAvailabilityCapability { MachineReadable, ManualOfficialUtilityCheck, ManualOfficial }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "PascalCase")]
pub enum ProviderAcquisitionCapability { WindowsManaged, UnsupportedAutomation }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "PascalCase")]
pub enum ProviderInstallCapability { WindowsManaged, OfficialUtility, ManualOfficial }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "PascalCase")]
pub enum ProviderImplementationStatus { Implemented, OfficialUtility, ManualOfficial, UnsupportedAutomation }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "PascalCase")]
pub enum ProviderQualificationStatus { SourceCompleteNativePending, NativePending }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCoverageEntry {
    pub provider_id: String,
    pub provider_family: String,
    pub authority_class: ProviderAuthorityClass,
    pub supported_vendors: Vec<String>,
    pub supported_classes: Vec<String>,
    pub discovery_capability: ProviderDiscoveryCapability,
    pub update_availability_capability: ProviderUpdateAvailabilityCapability,
    pub direct_acquisition_capability: ProviderAcquisitionCapability,
    pub install_capability: ProviderInstallCapability,
    pub manual_fallback: bool,
    pub implementation_status: ProviderImplementationStatus,
    pub qualification_status: ProviderQualificationStatus,
    pub official_authority: String,
}

impl ProviderCoverageEntry {
    pub fn automatically_evaluates_update_availability(&self) -> bool {
        self.update_availability_capability == ProviderUpdateAvailabilityCapability::MachineReadable
            && self.implementation_status == ProviderImplementationStatus::Implemented
    }

    pub fn requires_manual_check(&self) -> bool {
        !self.automatically_evaluates_update_availability()
            && matches!(self.implementation_status, ProviderImplementationStatus::OfficialUtility | ProviderImplementationStatus::ManualOfficial)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRegistry {
    pub schema: String,
    pub policy_version: String,
    pub providers: Vec<ProviderCoverageEntry>,
}

impl ProviderRegistry {
    pub fn parse(input: &str) -> Result<Self> {
        let registry: Self = serde_json::from_str(input)
            .map_err(|e| AuthorityError::ProviderRegistryInvalid(e.to_string()))?;
        registry.validate()?;
        Ok(registry)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != PROVIDER_COVERAGE_SCHEMA {
            return Err(AuthorityError::ProviderRegistryInvalid(format!("unexpected schema {}", self.schema)));
        }
        if self.policy_version.trim().is_empty() || self.providers.is_empty() {
            return Err(AuthorityError::ProviderRegistryInvalid("missing policy version or providers".into()));
        }
        let mut ids = BTreeSet::new();
        for p in &self.providers {
            if p.provider_id.trim().is_empty() || p.provider_family.trim().is_empty() || p.official_authority.trim().is_empty() {
                return Err(AuthorityError::ProviderRegistryInvalid("provider identity/authority is empty".into()));
            }
            if !ids.insert(p.provider_id.as_str()) {
                return Err(AuthorityError::ProviderRegistryInvalid(format!("duplicate provider id {}", p.provider_id)));
            }
            if p.supported_vendors.is_empty() || p.supported_classes.is_empty() {
                return Err(AuthorityError::ProviderRegistryInvalid(format!("provider {} has empty applicability metadata", p.provider_id)));
            }
        }
        Ok(())
    }

    pub fn by_id(&self, id: &str) -> Option<&ProviderCoverageEntry> {
        self.providers.iter().find(|p| p.provider_id == id)
    }

    pub fn windows_update(&self) -> Option<&ProviderCoverageEntry> { self.by_id("microsoft.windows-update") }

    pub fn oem_provider_for_machine(&self, machine: &MachineProfile) -> Option<&ProviderCoverageEntry> {
        if machine.machine_kind != MachineKind::Oem { return None; }
        self.providers.iter().find(|p| {
            p.authority_class == ProviderAuthorityClass::Oem
                && p.supported_vendors.iter().any(|vendor| text_matches(&machine.manufacturer, vendor))
        })
    }

    pub fn component_providers_for_device(&self, device: &DeviceIdentity) -> Vec<&ProviderCoverageEntry> {
        self.providers.iter().filter(|p| {
            p.authority_class == ProviderAuthorityClass::ComponentVendor
                && class_matches(&device.class_name, &p.supported_classes)
                && p.supported_vendors.iter().any(|vendor| vendor_matches(device, vendor))
        }).collect()
    }
}

pub fn builtin_provider_registry() -> Result<ProviderRegistry> {
    ProviderRegistry::parse(BUILTIN_PROVIDER_COVERAGE_JSON)
}

fn class_matches(class_name: &str, classes: &[String]) -> bool {
    classes.iter().any(|c| c.eq_ignore_ascii_case("Any") || c.eq_ignore_ascii_case(class_name))
}

fn vendor_matches(device: &DeviceIdentity, vendor: &str) -> bool {
    if vendor.eq_ignore_ascii_case("Any") { return true; }
    let normalized = vendor.trim().to_ascii_uppercase();
    if let Some(id) = normalized.strip_prefix("VEN_") {
        return device.vendor_id.eq_ignore_ascii_case(id)
            || device.hardware_ids.iter().chain(device.compatible_ids.iter()).any(|v| v.to_ascii_uppercase().contains(&normalized));
    }
    text_matches(&device.manufacturer, vendor) || text_matches(&device.current_provider, vendor)
}

fn text_matches(value: &str, expected: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    let expected = expected.trim().to_ascii_lowercase();
    !value.is_empty() && !expected.is_empty() && (value == expected || value.contains(&expected))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_registry_is_machine_readable_and_unique() {
        let registry = builtin_provider_registry().expect("valid built-in registry");
        assert!(registry.providers.len() >= 15);
        assert!(registry.windows_update().unwrap().automatically_evaluates_update_availability());
    }

    #[test]
    fn malformed_provider_registry_fails_closed() {
        let malformed = r#"{"schema":"wrong","policyVersion":"x","providers":[]}"#;
        assert!(matches!(ProviderRegistry::parse(malformed), Err(AuthorityError::ProviderRegistryInvalid(_))));
    }
}
