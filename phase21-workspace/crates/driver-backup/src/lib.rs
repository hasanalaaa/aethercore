#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

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
    #[error("the driver backup changed after it was exported: {0}")]
    Changed(String),
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

const MANIFEST_NAME: &str = "aethercore-backup.json";
const MAX_BACKUP_FILES: usize = 4096;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct BackupManifest {
    source_inf: String,
    files: Vec<BackupFile>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct BackupFile {
    relative_path: String,
    bytes: u64,
    sha256: String,
}

/// Seals an exported driver package: every file's path, size and SHA-256 in a manifest beside it
/// (P84-04). An export with no INF, a link, or too many files is refused, not sealed.
pub fn seal_export(source_inf: &str, destination: &Path) -> Result<BackupEvidence> {
    let files = read_export(destination)?;
    if !files
        .iter()
        .any(|f| f.relative_path.to_ascii_lowercase().ends_with(".inf"))
    {
        return Err(BackupError::PnpUtil(
            "PnPUtil reported success but no exported INF package was found".into(),
        ));
    }
    let manifest = BackupManifest {
        source_inf: source_inf.to_ascii_lowercase(),
        files,
    };
    let manifest_path = destination.join(MANIFEST_NAME);
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
    Ok(BackupEvidence {
        source_inf: manifest.source_inf.clone(),
        backup_directory: destination.to_string_lossy().into_owned(),
        manifest_path: manifest_path.to_string_lossy().into_owned(),
        file_count: manifest.files.len().try_into().unwrap_or(u32::MAX),
        total_bytes: manifest.files.iter().map(|f| f.bytes).sum(),
        not_applicable: false,
    })
}

/// Re-reads a sealed export immediately before the install may start (P84-04): the directory is
/// still a real directory where the evidence says, its manifest is the one sealed there, every
/// listed file still has its hash, and nothing was added or removed. The export is the recovery
/// evidence; one that drifted or was replaced proves nothing and blocks the mutation.
pub fn verify_export(evidence: &BackupEvidence) -> Result<()> {
    let directory = Path::new(&evidence.backup_directory);
    if Path::new(&evidence.manifest_path) != directory.join(MANIFEST_NAME) {
        return Err(BackupError::Changed(
            "the manifest is not in the export".into(),
        ));
    }
    let sealed: BackupManifest = serde_json::from_slice(&fs::read(&evidence.manifest_path)?)?;
    let mut present = read_export(directory)?;
    present.retain(|f| f.relative_path != MANIFEST_NAME);
    let mut expected = sealed.files;
    present.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    expected.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    if expected.is_empty() || expected.len() != evidence.file_count as usize {
        return Err(BackupError::Changed(
            "the manifest does not list the export".into(),
        ));
    }
    if present != expected {
        return Err(BackupError::Changed(
            "a file was changed, added or removed".into(),
        ));
    }
    Ok(())
}

fn read_export(root: &Path) -> Result<Vec<BackupFile>> {
    reject_link(root)?;
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    Ok(files)
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<BackupFile>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        if out.len() >= MAX_BACKUP_FILES {
            return Err(BackupError::PnpUtil(
                "exported driver package exceeded file-count safety limit".into(),
            ));
        }
        let path = entry?.path();
        reject_link(&path)?;
        let ty = fs::symlink_metadata(&path)?.file_type();
        if ty.is_dir() {
            collect_files(root, &path, out)?;
            continue;
        }
        if !ty.is_file() {
            continue;
        }
        let bytes = fs::read(&path)?;
        let relative = path
            .strip_prefix(root)
            .map_err(|_| BackupError::InvalidRoot)?
            .to_string_lossy()
            .replace('\\', "/");
        out.push(BackupFile {
            relative_path: relative,
            bytes: bytes.len() as u64,
            sha256: hex::encode(Sha256::digest(&bytes)),
        });
    }
    Ok(())
}

/// A symbolic link, or on Windows any reparse point (a junction included), is refused: it could
/// point the backup, or its re-read, somewhere else.
fn reject_link(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    #[cfg(windows)]
    let link = {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    };
    #[cfg(not(windows))]
    let link = metadata.file_type().is_symlink();
    if link {
        return Err(BackupError::PnpUtil(format!(
            "refusing reparse point inside driver backup tree: {}",
            path.display()
        )));
    }
    Ok(())
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
    fn export_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "aethercore-backup-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("oem7.inf"), b"[Version]").unwrap();
        fs::write(dir.join("sub").join("driver.sys"), b"bytes").unwrap();
        dir
    }

    /// P84-04: the export is re-read just before the install; one that drifted, grew, shrank or
    /// was never an INF package blocks the mutation instead of passing as recovery evidence.
    #[test]
    fn a_sealed_export_verifies_until_anything_in_it_changes() {
        let dir = export_dir("seal");
        let evidence = seal_export("OEM7.INF", &dir).expect("sealed");
        assert_eq!(evidence.file_count, 2);
        verify_export(&evidence).expect("unchanged export verifies");

        fs::write(dir.join("sub").join("driver.sys"), b"other").unwrap();
        assert!(matches!(
            verify_export(&evidence),
            Err(BackupError::Changed(_))
        ));
        fs::write(dir.join("sub").join("driver.sys"), b"bytes").unwrap();
        verify_export(&evidence).expect("restored content verifies again");

        fs::write(dir.join("extra.cat"), b"x").unwrap();
        assert!(matches!(
            verify_export(&evidence),
            Err(BackupError::Changed(_))
        ));
        fs::remove_file(dir.join("extra.cat")).unwrap();

        fs::remove_file(dir.join("sub").join("driver.sys")).unwrap();
        assert!(matches!(
            verify_export(&evidence),
            Err(BackupError::Changed(_))
        ));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_export_without_an_inf_is_not_sealed() {
        let dir = export_dir("noinf");
        fs::remove_file(dir.join("oem7.inf")).unwrap();
        assert!(seal_export("oem7.inf", &dir).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    /// The directory swapped for a link after sealing is refused on the re-read.
    #[cfg(unix)]
    #[test]
    fn an_export_replaced_by_a_link_is_refused() {
        let dir = export_dir("link");
        let evidence = seal_export("oem7.inf", &dir).expect("sealed");
        let moved = dir.with_extension("moved");
        fs::rename(&dir, &moved).unwrap();
        std::os::unix::fs::symlink(&moved, &dir).unwrap();
        assert!(verify_export(&evidence).is_err());
        let _ = fs::remove_file(&dir);
        let _ = fs::remove_dir_all(&moved);
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
