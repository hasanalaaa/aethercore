#![forbid(unsafe_code)]

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
}

impl GpuVendor {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Nvidia => "NVIDIA",
            Self::Amd => "AMD",
            Self::Intel => "Intel",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "nvidia" => Some(Self::Nvidia),
            "amd" => Some(Self::Amd),
            "intel" => Some(Self::Intel),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuVendorPolicy {
    pub vendor: GpuVendor,
    pub app_name: &'static str,
    pub executable_parts: &'static [&'static str],
    pub official_url: &'static str,
}

pub fn policy(vendor: GpuVendor) -> GpuVendorPolicy {
    match vendor {
        GpuVendor::Nvidia => GpuVendorPolicy {
            vendor,
            app_name: "NVIDIA App",
            executable_parts: &["NVIDIA Corporation", "NVIDIA App", "CEF", "NVIDIA App.exe"],
            official_url: "https://www.nvidia.com/en-us/software/nvidia-app/",
        },
        GpuVendor::Amd => GpuVendorPolicy {
            vendor,
            app_name: "AMD Software: Adrenalin Edition",
            executable_parts: &["AMD", "CNext", "CNext", "RadeonSoftware.exe"],
            official_url: "https://www.amd.com/en/support/download/drivers.html",
        },
        GpuVendor::Intel => GpuVendorPolicy {
            vendor,
            app_name: "Intel Driver & Support Assistant",
            executable_parts: &["Intel", "Driver and Support Assistant", "DSATray.exe"],
            official_url: "https://www.intel.com/content/www/us/en/support/detect.html",
        },
    }
}

pub fn candidate_app_paths(vendor: GpuVendor) -> Vec<PathBuf> {
    let policy = policy(vendor);
    let mut paths = Vec::new();
    for root in [
        std::env::var_os("ProgramFiles"),
        std::env::var_os("ProgramFiles(x86)"),
    ]
    .into_iter()
    .flatten()
    {
        let mut path = PathBuf::from(root);
        for part in policy.executable_parts {
            path.push(part);
        }
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    paths
}

pub fn installed_app_path(vendor: GpuVendor) -> Option<PathBuf> {
    candidate_app_paths(vendor)
        .into_iter()
        .find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_supported_vendors() {
        assert_eq!(GpuVendor::parse("NVIDIA"), Some(GpuVendor::Nvidia));
        assert_eq!(GpuVendor::parse(" amd "), Some(GpuVendor::Amd));
        assert_eq!(GpuVendor::parse("Intel"), Some(GpuVendor::Intel));
        assert_eq!(GpuVendor::parse("other"), None);
    }

    #[test]
    fn policies_are_https_and_have_executable_names() {
        for vendor in [GpuVendor::Nvidia, GpuVendor::Amd, GpuVendor::Intel] {
            let definition = policy(vendor);
            assert!(definition.official_url.starts_with("https://"));
            assert!(
                definition
                    .executable_parts
                    .last()
                    .is_some_and(|name| name.to_ascii_lowercase().ends_with(".exe"))
            );
        }
    }
}
