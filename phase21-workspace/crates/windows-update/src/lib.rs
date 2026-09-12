#![deny(unsafe_op_in_unsafe_fn)]

use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("Windows Update discovery is only available on Windows")]
    UnsupportedPlatform,
    #[error("Windows Update Agent error: {0}")]
    Wua(String),
    #[error("Windows Update is offline: {0}")]
    Offline(String),
    #[error("Windows Update returned an invalid size value")]
    InvalidSize,
    #[error("another update installation is active")]
    Busy,
    #[error("Windows requires a reboot before another driver installation")]
    RebootPending,
    #[error("selected update is no longer applicable: {0}")]
    OfferNoLongerApplicable(String),
    #[error("selected update requires an EULA that has not been accepted: {0}")]
    EulaRequired(String),
    #[error("Windows Update operation timed out during {0}")]
    Timeout(String),
    #[error("pre-install protection failed: {0}")]
    Protection(String),
}

pub type Result<T> = std::result::Result<T, UpdateError>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum VersionSource {
    TitleHeuristic,
    Unavailable,
}

impl Default for VersionSource {
    fn default() -> Self {
        Self::Unavailable
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DriverOffer {
    pub update_id: String,
    pub revision: i32,
    pub title: String,
    pub hardware_id: String,
    pub driver_class: String,
    pub manufacturer: String,
    pub model: String,
    pub provider: String,
    pub driver_date_iso: String,
    pub device_problem_number: i32,
    pub device_status: i32,
    pub min_download_bytes: u64,
    pub max_download_bytes: u64,
    pub target_version: String,
    pub target_version_source: VersionSource,
    pub support_url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryResult {
    pub offers: Vec<DriverOffer>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct UpdateIdentity {
    pub update_id: String,
    pub revision: i32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ExecutionStage {
    Revalidating,
    Downloading,
    Installing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WuaProgress {
    pub stage: ExecutionStage,
    pub percent: u32,
    pub current_update_index: u32,
    pub current_update_percent: u32,
    /// DBT-P46-B16: None when this tick's WUA DECIMAL->u64 conversion failed.
    /// A real 0 ("nothing transferred yet") and a failed read are different
    /// facts; the caller keeps the last known value rather than publishing a
    /// fabricated zero.
    pub bytes_downloaded: Option<u64>,
    /// DBT-P46-B16: None when unknown. An unknown total and a zero-length
    /// download are different facts.
    pub bytes_total: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WuaUpdateResult {
    pub identity: UpdateIdentity,
    pub result_code: String,
    pub hresult: i32,
    pub reboot_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WuaExecutionResult {
    pub result_code: String,
    pub hresult: i32,
    pub reboot_required: bool,
    pub updates: Vec<WuaUpdateResult>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateHealthProbe {
    pub result_code: String,
    pub hresult: i32,
    pub pending_update_count: u32,
    pub detail: String,
}

pub fn extract_version_from_update_title(title: &str) -> Option<String> {
    // WUA does not expose a universal target driver-version property. AetherCore therefore
    // treats a version parsed from the title as display-only metadata, never as a ranking
    // signal. Be deliberately conservative: only accept a dotted numeric token at the very
    // end of the title so a date/version mentioned earlier cannot be mistaken for the target.
    let token = title
        .split_whitespace()
        .last()?
        .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '_' && c != '-')
        .trim_start_matches(['v', 'V']);

    if token.len() < 5 || token.len() > 64 {
        return None;
    }
    let pieces: Vec<&str> = token.split('.').collect();
    if !(3..=6).contains(&pieces.len())
        || !pieces
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
    {
        return None;
    }
    Some(token.to_owned())
}

pub fn ole_automation_date_to_iso(value: f64) -> Option<String> {
    if !value.is_finite() || value < -100_000.0 || value > 1_000_000.0 {
        return None;
    }
    let base = NaiveDate::from_ymd_opt(1899, 12, 30)?.and_hms_opt(0, 0, 0)?;
    let millis = (value * 86_400_000.0).round();
    if millis < i64::MIN as f64 || millis > i64::MAX as f64 {
        return None;
    }
    base.checked_add_signed(Duration::milliseconds(millis as i64))
        .map(|dt| dt.date().format("%Y-%m-%d").to_string())
}

#[cfg(windows)]
mod execution_windows;
#[cfg(windows)]
mod windows_impl;

#[cfg(windows)]
pub use execution_windows::{ensure_servicing_available, execute_driver_updates};
#[cfg(windows)]
pub use windows_impl::{discover_driver_offers, probe_update_health};

#[cfg(not(windows))]
pub fn probe_update_health() -> UpdateHealthProbe {
    UpdateHealthProbe {
        result_code: "UpdateUnknown".into(),
        hresult: 0,
        pending_update_count: 0,
        detail: "Windows Update Agent is only available on Windows".into(),
    }
}

#[cfg(not(windows))]
pub fn discover_driver_offers() -> Result<DiscoveryResult> {
    Err(UpdateError::UnsupportedPlatform)
}

#[cfg(not(windows))]
pub fn ensure_servicing_available() -> Result<()> {
    Err(UpdateError::UnsupportedPlatform)
}

#[cfg(not(windows))]
pub fn execute_driver_updates<P, B>(
    _identities: &[UpdateIdentity],
    _progress: P,
    _before_install: B,
) -> Result<WuaExecutionResult>
where
    P: FnMut(WuaProgress),
    B: FnOnce() -> std::result::Result<(), String>,
{
    Err(UpdateError::UnsupportedPlatform)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conservatively_extracts_dotted_driver_version() {
        assert_eq!(
            extract_version_from_update_title("Intel - Extension - 31.0.101.5590"),
            Some("31.0.101.5590".into())
        );
        assert_eq!(
            extract_version_from_update_title("NVIDIA - Display - (32.0.15.6094)"),
            Some("32.0.15.6094".into())
        );
    }

    #[test]
    fn does_not_invent_target_version() {
        assert_eq!(
            extract_version_from_update_title("Realtek Semiconductor Corp. - MEDIA"),
            None
        );
        assert_eq!(extract_version_from_update_title("Driver 2026.08"), None);
        assert_eq!(
            extract_version_from_update_title("Vendor 31.0.101.5590 - Extension"),
            None
        );
    }

    #[test]
    fn converts_modern_ole_dates_to_iso() {
        assert_eq!(
            ole_automation_date_to_iso(2.0).as_deref(),
            Some("1900-01-01")
        );
    }
}
