#![deny(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PnpError {
    #[error("Windows PnP inventory is only available on Windows")]
    UnsupportedPlatform,
    #[error("Windows API error: {0}")]
    Windows(String),
}

pub type Result<T> = std::result::Result<T, PnpError>;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstalledDriver {
    pub provider: String,
    pub version: String,
    pub inf_path: String,
    pub date: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStatus {
    pub raw_status: u32,
    pub problem_code: u32,
    pub has_problem: bool,
    pub missing_driver: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceRecord {
    pub instance_id: String,
    pub display_name: String,
    pub description: String,
    pub class_name: String,
    pub class_guid: String,
    pub manufacturer: String,
    pub enumerator: String,
    pub location: String,
    pub hardware_ids: Vec<String>,
    pub compatible_ids: Vec<String>,
    pub status: DeviceStatus,
    pub driver: Option<InstalledDriver>,
}


#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceVerification {
    pub instance_id: String,
    pub present: bool,
    pub class_name: String,
    pub hardware_ids: Vec<String>,
    pub compatible_ids: Vec<String>,
    pub status: DeviceStatus,
    pub driver: Option<InstalledDriver>,
}

impl DeviceRecord {
    pub fn all_match_ids(&self) -> impl Iterator<Item = &str> {
        self.hardware_ids
            .iter()
            .chain(self.compatible_ids.iter())
            .map(String::as_str)
    }
}

pub fn normalize_pnp_id(value: &str) -> String {
    value.trim().to_ascii_uppercase()
}

pub fn parse_multi_sz_utf16(units: &[u16]) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0usize;
    while start < units.len() {
        let Some(end_rel) = units[start..].iter().position(|v| *v == 0) else {
            break;
        };
        let end = start + end_rel;
        if end == start {
            break;
        }
        let value = String::from_utf16_lossy(&units[start..end]);
        if !value.trim().is_empty() {
            out.push(value);
        }
        start = end + 1;
    }
    out
}

pub fn is_missing_driver_problem(problem_code: u32) -> bool {
    // Device Manager Code 28 / CM_PROB_FAILED_INSTALL: drivers are not installed.
    problem_code == 28
}

#[cfg(windows)]
mod windows_impl;

#[cfg(windows)]
pub use windows_impl::{scan_present_devices, verify_device_instances};

#[cfg(not(windows))]
pub fn scan_present_devices() -> Result<Vec<DeviceRecord>> {
    Err(PnpError::UnsupportedPlatform)
}

#[cfg(not(windows))]
pub fn verify_device_instances(_instance_ids: &[String]) -> Result<Vec<DeviceVerification>> {
    Err(PnpError::UnsupportedPlatform)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_registry_multi_sz() {
        let data: Vec<u16> = "PCI\\VEN_8086&DEV_1234\0PCI\\VEN_8086&CC_0300\0\0"
            .encode_utf16()
            .collect();
        assert_eq!(
            parse_multi_sz_utf16(&data),
            vec!["PCI\\VEN_8086&DEV_1234", "PCI\\VEN_8086&CC_0300"]
        );
    }

    #[test]
    fn normalization_is_case_insensitive_and_trimmed() {
        assert_eq!(normalize_pnp_id("  pci\\ven_10de&dev_2684 "), "PCI\\VEN_10DE&DEV_2684");
    }

    #[test]
    fn only_code_28_is_classified_as_missing_driver() {
        assert!(is_missing_driver_problem(28));
        assert!(!is_missing_driver_problem(0));
        assert!(!is_missing_driver_problem(10));
    }
}
