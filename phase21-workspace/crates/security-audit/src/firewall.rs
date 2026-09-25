//! Phase 32 — read-only firewall configuration evidence.
//!
//! Unix inspects pf/ufw/nftables config-file presence. Windows reads the three
//! profile registry values, preferring policy over local configuration. Neither
//! path claims the effective filtering state or inspects live rules.

use crate::model::{Confidence, EvidenceRef, SecFinding, Severity};
use serde::{Deserialize, Serialize};
#[cfg(not(windows))]
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum FirewallStatus {
    /// A firewall config was found (state inferred from config presence only).
    ConfigPresent { product: String, path: String },
    /// Windows registry configuration; `None` means that profile could not be read.
    WindowsProfiles { profiles: Vec<WindowsProfile> },
    /// No firewall config found on this host.
    NotAvailable { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsProfile {
    pub name: String,
    pub configured_enabled: Option<bool>,
    pub source: String,
}

/// Checks every supported firewall's config file locations.
pub fn audit_firewall_state() -> FirewallStatus {
    #[cfg(windows)]
    return audit_windows_profiles();

    #[cfg(not(windows))]
    audit_unix_config()
}

#[cfg(not(windows))]
fn audit_unix_config() -> FirewallStatus {
    let candidates: [(&str, &[&str]); 3] = [
        ("pf", &["/etc/pf.conf", "/private/etc/pf.conf"]),
        ("ufw", &["/etc/ufw/ufw.conf", "/lib/ufw/ufw.conf"]),
        (
            "nftables",
            &["/etc/nftables.conf", "/etc/sysconfig/nftables.conf"],
        ),
    ];
    for (product, paths) in candidates {
        for p in paths {
            if Path::new(p).is_file() {
                return FirewallStatus::ConfigPresent {
                    product: product.to_string(),
                    path: p.to_string(),
                };
            }
        }
    }
    FirewallStatus::NotAvailable {
        reason: "no pf/ufw/nftables configuration file found; live state probing requires \
             elevation and stays out of scope (QD-032-001)"
            .to_string(),
    }
}

#[cfg(windows)]
fn audit_windows_profiles() -> FirewallStatus {
    let profiles: Vec<WindowsProfile> = ["DomainProfile", "PrivateProfile", "PublicProfile"]
        .into_iter()
        .map(read_windows_profile)
        .collect();
    if profiles.iter().all(|p| p.configured_enabled.is_none()) {
        return FirewallStatus::NotAvailable {
            reason: format!(
                "Windows EnableFirewall registry configuration unavailable for DomainProfile, \
                 PrivateProfile and PublicProfile: {}; effective state requires a live firewall probe",
                profiles
                    .iter()
                    .map(|p| format!("{}: {}", p.name, p.source))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        };
    }
    FirewallStatus::WindowsProfiles { profiles }
}

#[cfg(windows)]
fn read_windows_profile(name: &str) -> WindowsProfile {
    let policy = format!("SOFTWARE\\Policies\\Microsoft\\WindowsFirewall\\{name}");
    let local = format!(
        "SYSTEM\\CurrentControlSet\\Services\\SharedAccess\\Parameters\\FirewallPolicy\\{name}"
    );
    let selected = match registry_dword(&policy, "EnableFirewall") {
        Ok(Some(value)) => (Some(value), format!("HKLM\\{policy}\\EnableFirewall")),
        Ok(None) => match registry_dword(&local, "EnableFirewall") {
            Ok(value) => (value, format!("HKLM\\{local}\\EnableFirewall")),
            Err(reason) => (None, format!("HKLM\\{local}\\EnableFirewall: {reason}")),
        },
        Err(reason) => (None, format!("HKLM\\{policy}\\EnableFirewall: {reason}")),
    };
    let (value, source) = selected;
    let source = match value {
        Some(value) if value > 1 => format!("{source}: unsupported DWORD {value}"),
        _ => source,
    };
    WindowsProfile {
        name: name.to_string(),
        configured_enabled: value.and_then(|value| match value {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }),
        source,
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;

    #[test]
    fn firewall_evidence_names_windows_registry_profile_values() {
        let status = audit_firewall_state();
        eprintln!("Windows firewall registry evidence: {status:?}");
        match status {
            FirewallStatus::WindowsProfiles { profiles } => {
                assert_eq!(profiles.len(), 3);
                for name in ["DomainProfile", "PrivateProfile", "PublicProfile"] {
                    let profile = profiles
                        .iter()
                        .find(|profile| profile.name == name)
                        .unwrap();
                    assert!(profile.source.contains("EnableFirewall"), "{profile:?}");
                }
                let finding = firewall_findings(&FirewallStatus::WindowsProfiles { profiles })
                    .expect("registry values become evidence");
                assert!(
                    finding
                        .evidence
                        .iter()
                        .all(|e| { e.expected_or_threshold.contains("effective state") })
                );
            }
            FirewallStatus::NotAvailable { reason } => {
                assert!(reason.contains("DomainProfile"), "{reason}");
                assert!(reason.contains("EnableFirewall"), "{reason}");
                assert!(reason.contains("effective state"), "{reason}");
            }
            FirewallStatus::ConfigPresent { .. } => panic!("Unix config is not Windows evidence"),
        }
    }
}

#[cfg(windows)]
fn registry_dword(subkey: &str, name: &str) -> Result<Option<u32>, String> {
    use std::ffi::c_void;

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegGetValueW(
            key: isize,
            subkey: *const u16,
            name: *const u16,
            flags: u32,
            value_type: *mut u32,
            data: *mut c_void,
            size: *mut u32,
        ) -> i32;
    }
    const HKEY_LOCAL_MACHINE: isize = 0x8000_0002u32 as i32 as isize;
    const RRF_RT_REG_DWORD: u32 = 0x10;
    let subkey: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    let name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut value = 0u32;
    let mut value_type = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: the two strings are NUL-terminated and all output pointers refer
    // to live 32-bit values with the size passed to the API.
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            subkey.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_DWORD,
            &mut value_type,
            (&mut value as *mut u32).cast::<c_void>(),
            &mut size,
        )
    };
    match status {
        0 if value_type == 4 && size == 4 => Ok(Some(value)),
        0 => Err(format!("unexpected type {value_type} or size {size}")),
        2 | 3 => Ok(None), // Missing value or key; policy absence permits local fallback.
        code => Err(format!("RegGetValueW error {code}")),
    }
}

/// Wraps the status into either an advisory finding (config found ⇒ note its
/// presence as evidence-backed posture info) or an honest NotAvailable marker
/// finding so reports never show silent gaps.
pub fn firewall_findings(status: &FirewallStatus) -> Option<SecFinding> {
    match status {
        FirewallStatus::WindowsProfiles { profiles } => SecFinding::try_new(
            "SEC-FW-001",
            "fw.config_present",
            Severity::Advisory,
            profiles
                .iter()
                .map(|profile| EvidenceRef {
                    fact: format!("{} EnableFirewall registry configuration", profile.name),
                    observed: match profile.configured_enabled {
                        Some(true) => "configured-enabled",
                        Some(false) => "configured-disabled",
                        None => "unavailable",
                    }
                    .to_string(),
                    expected_or_threshold:
                        "registry configuration only; effective state requires a live firewall probe"
                            .to_string(),
                    source_location: profile.source.clone(),
                })
                .collect(),
            None,
            "sec.fw.configPresent",
            Confidence::Inferred,
        ),
        FirewallStatus::ConfigPresent { product, path } => SecFinding::try_new(
            "SEC-FW-001",
            "fw.config_present",
            Severity::Advisory,
            vec![EvidenceRef {
                fact: format!("{product} configuration file present at {path}"),
                observed: format!("{product}:config-present"),
                expected_or_threshold:
                    "host packet filter configured; rule review requires elevation lane".to_string(),
                source_location: path.clone(),
            }],
            None,
            "sec.fw.configPresent",
            Confidence::Inferred,
        ),
        FirewallStatus::NotAvailable { reason } => SecFinding::try_new(
            "SEC-FW-900",
            "fw.state_not_available",
            Severity::Advisory,
            vec![EvidenceRef {
                fact: reason.clone(),
                observed: "firewall=not-available".to_string(),
                expected_or_threshold: "active host firewall state (needs elevated probe lane)"
                    .to_string(),
                source_location: "host".to_string(),
            }],
            None,
            "sec.fw.notAvailable",
            Confidence::Heuristic,
        ),
    }
}
