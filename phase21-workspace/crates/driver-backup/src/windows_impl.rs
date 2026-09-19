use std::{
    fs,
    os::windows::{fs::MetadataExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use sha2::{Digest, Sha256};

use crate::{
    BackupError, BackupEvidence, BackupFile, BackupManifest, Result, validate_oem_inf_name,
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const MAX_BACKUP_FILES: usize = 4096;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

pub fn export_driver_package(oem_inf: &str, destination: &Path) -> Result<BackupEvidence> {
    if !validate_oem_inf_name(oem_inf) {
        return Err(BackupError::InvalidInf);
    }
    fs::create_dir_all(destination)?;
    reject_reparse_point(destination)?;
    if destination.read_dir()?.next().is_some() {
        return Err(BackupError::InvalidRoot);
    }

    let system_root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    let pnputil = PathBuf::from(system_root)
        .join("System32")
        .join("pnputil.exe");
    if !pnputil.is_file() {
        return Err(BackupError::PnpUtil(
            "%SystemRoot%\\System32\\pnputil.exe is missing".into(),
        ));
    }

    let output = Command::new(&pnputil)
        .arg("/export-driver")
        .arg(oem_inf)
        .arg(destination)
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(BackupError::PnpUtil(format!(
            "exit {:?}: {} {}",
            output.status.code(),
            truncate(&stdout),
            truncate(&stderr)
        )));
    }

    let mut files = Vec::new();
    collect_files(destination, destination, &mut files)?;
    if files.is_empty()
        || !files
            .iter()
            .any(|f| f.relative_path.to_ascii_lowercase().ends_with(".inf"))
    {
        return Err(BackupError::PnpUtil(
            "PnPUtil reported success but no exported INF package was found".into(),
        ));
    }
    let total_bytes = files.iter().map(|f| f.bytes).sum();
    let manifest = BackupManifest {
        source_inf: oem_inf.to_ascii_lowercase(),
        files,
    };
    let manifest_path = destination.join("aethercore-backup.json");
    let json = serde_json::to_vec_pretty(&manifest)?;
    fs::write(&manifest_path, json)?;
    Ok(BackupEvidence {
        source_inf: oem_inf.to_ascii_lowercase(),
        backup_directory: destination.to_string_lossy().into_owned(),
        manifest_path: manifest_path.to_string_lossy().into_owned(),
        file_count: manifest.files.len().try_into().unwrap_or(u32::MAX),
        total_bytes,
        not_applicable: false,
    })
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<BackupFile>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        if out.len() >= MAX_BACKUP_FILES {
            return Err(BackupError::PnpUtil(
                "exported driver package exceeded file-count safety limit".into(),
            ));
        }
        let entry = entry?;
        let ty = entry.file_type()?;
        let path = entry.path();
        reject_reparse_point(&path)?;
        if ty.is_symlink() {
            return Err(BackupError::PnpUtil(
                "exported package unexpectedly contained a symbolic link".into(),
            ));
        }
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

fn reject_reparse_point(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(BackupError::PnpUtil(format!(
            "refusing reparse point inside driver backup tree: {}",
            path.display()
        )));
    }
    Ok(())
}

fn truncate(value: &str) -> String {
    value
        .chars()
        .take(512)
        .collect::<String>()
        .replace(['\r', '\n'], " ")
}
