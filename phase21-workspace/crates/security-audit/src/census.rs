//! Phase 32 — Read-only installed-package census.
//!
//! Parses platform package-state FILES only (no command execution, no
//! elevation): macOS via `pkgutil` receipts under /var/db/receipts plus
//! Homebrew Cellar directory structure; Linux via dpkg status file or rpm
//! database presence. Absence of any source is an honest typed answer.

use crate::vulnjoin::InstalledPackage;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Hard clamp: maximum receipts scanned per source.
pub const MAX_RECEIPTS: usize = 4_096;
/// Hard clamp: max bytes per pkgutil receipt plist.
const MAX_PLIST_BYTES: u64 = 262_144;
/// Hard clamp: max bytes for a dpkg state file.
const MAX_STATE_FILE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CensusSource {
    MacosPkgutil,
    MacosHomebrew,
    DpkgStatus,
    RpmState,
}

impl CensusSource {
    pub fn as_str(self) -> &'static str {
        match self {
            CensusSource::MacosPkgutil => "macos-pkgutil-receipts",
            CensusSource::MacosHomebrew => "macos-homebrew-cellar",
            CensusSource::DpkgStatus => "dpkg-status",
            CensusSource::RpmState => "rpm-state",
        }
    }
}

/// Why no census was produced from one platform source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum NotAvailableReason {
    /// This OS does not have this source by construction.
    PlatformAbsent { detail: String },
    /// The state files/dirs exist but are unreadable without elevation.
    PermissionDenied { path: String },
    /// Source root simply absent on this host.
    NotFound { path: String },
}

impl NotAvailableReason {
    pub fn summary(&self) -> String {
        match self {
            NotAvailableReason::PlatformAbsent { detail } => {
                format!("platform-absent: {detail}")
            }
            NotAvailableReason::PermissionDenied { path } => {
                format!("permission-denied at {path}")
            }
            NotAvailableReason::NotFound { path } => format!("not-found at {path}"),
        }
    }
}

/// Result of one census lane.
pub struct CensusLane {
    pub source: CensusSource,
    pub packages: Vec<InstalledPackage>,
    pub not_available: Option<NotAvailableReason>,
}

fn not_available_for(kind: std::io::ErrorKind, path: &Path) -> NotAvailableReason {
    if matches!(kind, std::io::ErrorKind::PermissionDenied) {
        NotAvailableReason::PermissionDenied {
            path: path.display().to_string(),
        }
    } else {
        NotAvailableReason::NotFound {
            path: path.display().to_string(),
        }
    }
}

fn read_capped(path: &Path, cap: u64) -> Option<Vec<u8>> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > cap {
        return None;
    }
    std::fs::read(path).ok()
}

fn parse_plist_string_field(raw: &str, field: &str) -> Option<String> {
    // Minimal dependency-free plist extraction: <key>X</key><string>v</string>.
    let key_pat = format!("<key>{}</key>", field);
    let k = raw.find(&key_pat)?;
    let rest = &raw[k + key_pat.len()..];
    let s = rest.find("<string>")? + "<string>".len();
    let tail = &rest[s..];
    let e = tail.find("</string>")?;
    Some(tail[..e].trim().to_string())
}

/// Scans /var/db/receipts/*.plist (macOS pkgutil receipts).
fn scan_pkgutil_receipts(dir: &Path) -> CensusLane {
    let mut packages = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            return CensusLane {
                source: CensusSource::MacosPkgutil,
                packages,
                not_available: Some(not_available_for(e.kind(), dir)),
            };
        }
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for p in paths.into_iter().take(MAX_RECEIPTS) {
        if p.extension().and_then(|x| x.to_str()) != Some("plist") {
            continue;
        }
        if let Some(raw) = read_capped(&p, MAX_PLIST_BYTES)
            && let Ok(text) = String::from_utf8(raw)
        {
            let id = parse_plist_string_field(&text, "PackageIdentifier");
            let ver = parse_plist_string_field(&text, "PackageVersion");
            if let (Some(id), Some(ver)) = (id, ver)
                && !id.is_empty()
                && !ver.is_empty()
            {
                packages.push(InstalledPackage {
                    name: id,
                    version: ver,
                });
            }
        }
    }
    CensusLane {
        source: CensusSource::MacosPkgutil,
        packages,
        not_available: None,
    }
}

/// Walks Homebrew Cellar/<name>/<version>/ directories. Pure listing; brew is
/// never invoked (no subprocess ⇒ deterministic).
fn scan_homebrew_cellar(cellar: &Path) -> CensusLane {
    let mut packages = Vec::new();
    let entries = match std::fs::read_dir(cellar) {
        Ok(e) => e,
        Err(e) => {
            return CensusLane {
                source: CensusSource::MacosHomebrew,
                packages,
                not_available: Some(not_available_for(e.kind(), cellar)),
            };
        }
    };
    let mut formula_dirs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    formula_dirs.sort();
    for fdir in formula_dirs.into_iter().take(MAX_RECEIPTS) {
        let Some(name) = fdir.file_name().map(|n| n.to_string_lossy().to_string()) else {
            continue;
        };
        let Ok(versions) = std::fs::read_dir(&fdir) else {
            continue;
        };
        let mut version_dirs: Vec<PathBuf> = versions.flatten().map(|e| e.path()).collect();
        // Newest modified first, then name — deterministic selection of the
        // active version directory when several kegs coexist.
        version_dirs.sort_by_key(|p| {
            (
                std::cmp::Reverse(p.metadata().ok().and_then(|m| m.modified().ok())),
                p.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
            )
        });
        for vdir in version_dirs {
            let Some(vname) = vdir.file_name().map(|n| n.to_string_lossy().to_string()) else {
                continue;
            };
            if vdir.is_dir() && !vname.ends_with(".receipt.json") {
                packages.push(InstalledPackage {
                    name: name.clone(),
                    version: vname,
                });
                break;
            }
        }
    }
    CensusLane {
        source: CensusSource::MacosHomebrew,
        packages,
        not_available: None,
    }
}

/// dpkg /var/lib/dpkg/status parsing (Linux).
fn scan_dpkg_status(status_path: &Path) -> CensusLane {
    let mut packages = Vec::new();
    let raw = match read_capped(status_path, MAX_STATE_FILE_BYTES) {
        Some(r) => r,
        None => {
            let reason = if status_path.exists() {
                NotAvailableReason::PermissionDenied {
                    path: status_path.display().to_string(),
                }
            } else {
                NotAvailableReason::NotFound {
                    path: status_path.display().to_string(),
                }
            };
            return CensusLane {
                source: CensusSource::DpkgStatus,
                packages,
                not_available: Some(reason),
            };
        }
    };
    let text = String::from_utf8_lossy(&raw);
    let flush = |name: &mut Option<String>,
                 version: &mut Option<String>,
                 out: &mut Vec<InstalledPackage>| {
        if let (Some(n), Some(v)) = (name.take(), version.take())
            && !n.is_empty()
            && !v.is_empty()
        {
            out.push(InstalledPackage { name: n, version: v });
        }
    };
    let mut name: Option<String> = None;
    let mut version: Option<String> = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Package: ") {
            name = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("Version: ") {
            version = Some(rest.trim().to_string());
        } else if line.is_empty() {
            flush(&mut name, &mut version, &mut packages);
        }
    }
    flush(&mut name, &mut version, &mut packages);
    CensusLane {
        source: CensusSource::DpkgStatus,
        packages,
        not_available: None,
    }
}

/// rpm /var/lib/rpm/{Packages,rpmdb.sqlite} presence probe. Binary DB formats
/// are NOT parsed offline (no sqlite/bdb dependency); honest NotAvailable.
fn scan_rpm_state(rpm_dir: &Path) -> CensusLane {
    let present = rpm_dir.join("Packages").exists() || rpm_dir.join("rpmdb.sqlite").exists();
    let reason = if present {
        NotAvailableReason::PlatformAbsent {
            detail: "rpm db present but binary format not parsed offline".into(),
        }
    } else {
        NotAvailableReason::NotFound {
            path: rpm_dir.display().to_string(),
        }
    };
    CensusLane {
        source: CensusSource::RpmState,
        packages: Vec::new(),
        not_available: Some(reason),
    }
}

/// Full census across every lane applicable to the current host OS.
pub fn census() -> Vec<CensusLane> {
    let mut lanes = Vec::new();
    if cfg!(target_os = "macos") {
        lanes.push(scan_pkgutil_receipts(Path::new("/var/db/receipts")));
        lanes.push(scan_homebrew_cellar(Path::new("/opt/homebrew/Cellar")));
    } else if cfg!(target_os = "linux") {
        lanes.push(scan_dpkg_status(Path::new("/var/lib/dpkg/status")));
        lanes.push(scan_rpm_state(Path::new("/var/lib/rpm")));
    } else {
        lanes.push(CensusLane {
            source: CensusSource::MacosPkgutil,
            packages: vec![],
            not_available: Some(NotAvailableReason::PlatformAbsent {
                detail: "unsupported host OS".into(),
            }),
        });
    }
    lanes
}
