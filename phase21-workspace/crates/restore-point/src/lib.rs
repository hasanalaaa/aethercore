#![deny(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RestorePointError {
    #[error("System Restore is only available on Windows desktop editions")]
    UnsupportedPlatform,
    #[error("COM security initialization failed: {0}")]
    ComSecurity(String),
    #[error("System Restore is unavailable or disabled: {0}")]
    Unavailable(String),
    #[error("SRSetRestorePointW failed with status {0}")]
    RestoreStatus(u32),
    #[error("Windows returned a restore point sequence but no fresh AetherCore restore point could be verified")]
    NotFresh,
    #[error("WMI restore point verification failed: {0}")]
    Verification(String),
}

pub type Result<T> = std::result::Result<T, RestorePointError>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RestorePointEvidence {
    pub sequence_number: i64,
    pub description: String,
    pub verified_fresh: bool,
}

#[cfg(windows)]
mod windows_impl;
#[cfg(windows)]
pub use windows_impl::{begin_driver_install, cancel_driver_install, end_driver_install, initialize_process_com_security};

#[cfg(not(windows))]
pub fn initialize_process_com_security() -> Result<()> { Err(RestorePointError::UnsupportedPlatform) }
#[cfg(not(windows))]
pub fn begin_driver_install(_: &str) -> Result<RestorePointEvidence> { Err(RestorePointError::UnsupportedPlatform) }
#[cfg(not(windows))]
pub fn end_driver_install(_: i64, _: &str) -> Result<()> { Err(RestorePointError::UnsupportedPlatform) }
#[cfg(not(windows))]
pub fn cancel_driver_install(_: i64, _: &str) -> Result<()> { Err(RestorePointError::UnsupportedPlatform) }

pub fn description_for_plan(plan_id: &str) -> String {
    let safe = plan_id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(36).collect::<String>();
    format!("Installed AetherCore Drivers {safe}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn description_is_bounded_and_service_generated() {
        let text = description_for_plan("550e8400-e29b-41d4-a716-446655440000/../../bad");
        assert!(text.starts_with("Installed AetherCore Drivers "));
        assert!(!text.contains('/'));
        assert!(text.encode_utf16().count() < 256);
    }
}
