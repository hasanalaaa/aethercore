use std::{
    fs,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::{BackupError, BackupEvidence, Result, reject_link, validate_oem_inf_name};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn export_driver_package(oem_inf: &str, destination: &Path) -> Result<BackupEvidence> {
    if !validate_oem_inf_name(oem_inf) {
        return Err(BackupError::InvalidInf);
    }
    fs::create_dir_all(destination)?;
    reject_link(destination)?;
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

    crate::seal_export(oem_inf, destination)
}

fn truncate(value: &str) -> String {
    value
        .chars()
        .take(512)
        .collect::<String>()
        .replace(['\r', '\n'], " ")
}
