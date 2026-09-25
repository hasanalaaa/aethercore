#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BackupError {
    #[error("driver backup is only available on Windows")]
    UnsupportedPlatform,
    #[error("refusing to export an untrusted driver INF name")]
    InvalidInf,
    #[error("backup root is invalid")]
    InvalidRoot,
    #[error("PnPUtil failed: {0}")]
    PnpUtil(String),
    #[error("filesystem: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, BackupError>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupEvidence {
    pub source_inf: String,
    pub backup_directory: String,
    pub manifest_path: String,
    pub file_count: u32,
    pub total_bytes: u64,
    pub not_applicable: bool,
}

// Only windows_impl builds a manifest; off Windows these would be dead code.
#[cfg(windows)]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupManifest {
    source_inf: String,
    files: Vec<BackupFile>,
}

#[cfg(windows)]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupFile {
    relative_path: String,
    bytes: u64,
    sha256: String,
}

pub fn validate_oem_inf_name(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    let Some(number) = lower
        .strip_prefix("oem")
        .and_then(|v| v.strip_suffix(".inf"))
    else {
        return false;
    };
    !number.is_empty() && number.len() <= 8 && number.chars().all(|c| c.is_ascii_digit())
}

fn validate_storage_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

pub fn plan_backup_root(product_data_root: &Path, plan_id: &str) -> Result<PathBuf> {
    if !validate_storage_component(plan_id) {
        return Err(BackupError::InvalidRoot);
    }
    Ok(product_data_root
        .join("recovery")
        .join("driver-backups")
        .join(plan_id))
}

pub fn candidate_backup_root(plan_root: &Path, candidate_id: &str) -> Result<PathBuf> {
    if !validate_storage_component(candidate_id) {
        return Err(BackupError::InvalidRoot);
    }
    Ok(plan_root.join(candidate_id))
}

#[cfg(windows)]
mod windows_impl;
#[cfg(windows)]
pub use windows_impl::export_driver_package;

#[cfg(not(windows))]
pub fn export_driver_package(_: &str, _: &Path) -> Result<BackupEvidence> {
    Err(BackupError::UnsupportedPlatform)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_service_observed_oem_inf_names_are_accepted() {
        assert!(validate_oem_inf_name("oem42.inf"));
        assert!(validate_oem_inf_name("OEM123.INF"));
        for bad in [
            "netrtwlane.inf",
            "oem.inf",
            "oem1.inf & whoami",
            "..\\oem1.inf",
            "C:\\Windows\\INF\\oem1.inf",
        ] {
            assert!(!validate_oem_inf_name(bad), "{bad}");
        }
    }
    #[test]
    fn backup_root_cannot_escape_product_data() {
        let root = Path::new(r"C:\\ProgramData\\AetherCore");
        assert!(
            plan_backup_root(root, "550e8400-e29b-41d4-a716-446655440000")
                .unwrap()
                .starts_with(root)
        );
        assert!(plan_backup_root(root, "..\\escape").is_err());
    }
}
