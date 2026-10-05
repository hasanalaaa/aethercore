#![allow(unsafe_code)]

use std::{
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::{BackupError, BackupEvidence, Result, reject_link, validate_oem_inf_name};
use windows::{
    Win32::{
        Devices::DeviceAndDriverInstallation::SetupGetInfDriverStoreLocationW,
        Storage::FileSystem::{GetDiskFreeSpaceExW, GetDiskFreeSpaceW},
    },
    core::PCWSTR,
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn export_driver_package(oem_inf: &str, destination: &Path) -> Result<BackupEvidence> {
    crate::export_into(
        oem_inf,
        destination,
        |root| export_capacity(oem_inf, root),
        |root| run_export(oem_inf, root),
    )
}

fn export_capacity(oem_inf: &str, destination: &Path) -> Result<(u64, u64)> {
    use std::os::windows::ffi::OsStrExt;
    let inf: Vec<u16> = oem_inf.encode_utf16().chain([0]).collect();
    // SetupAPI documents MAX_PATH as the supported buffer bound for this read-only resolver.
    let mut store_inf = [0u16; 260];
    unsafe {
        SetupGetInfDriverStoreLocationW(
            PCWSTR(inf.as_ptr()),
            None,
            PCWSTR::null(),
            &mut store_inf,
            None,
        )
    }
    .map_err(|e| BackupError::PnpUtil(format!("cannot establish Driver Store export size: {e}")))?;
    let end = store_inf
        .iter()
        .position(|c| *c == 0)
        .ok_or(BackupError::InvalidRoot)?;
    let store_inf =
        PathBuf::from(String::from_utf16(&store_inf[..end]).map_err(|_| BackupError::InvalidRoot)?);
    let volume_root: PathBuf = destination.components().take(2).collect();
    let volume: Vec<u16> = volume_root.as_os_str().encode_wide().chain([0]).collect();
    let (mut sectors, mut bytes_per_sector) = (0, 0);
    unsafe {
        GetDiskFreeSpaceW(
            PCWSTR(volume.as_ptr()),
            Some(&mut sectors),
            Some(&mut bytes_per_sector),
            None,
            None,
        )
    }
    .map_err(|e| BackupError::PnpUtil(format!("cannot establish export allocation unit: {e}")))?;
    let required = crate::package_footprint(
        store_inf.parent().ok_or(BackupError::InvalidRoot)?,
        u64::from(sectors) * u64::from(bytes_per_sector),
    )?;
    let wide: Vec<u16> = destination.as_os_str().encode_wide().chain([0]).collect();
    let mut available = 0;
    unsafe { GetDiskFreeSpaceExW(PCWSTR(wide.as_ptr()), Some(&mut available), None, None) }
        .map_err(|e| BackupError::PnpUtil(format!("cannot establish export capacity: {e}")))?;
    Ok((required, available))
}

fn run_export(oem_inf: &str, destination: &Path) -> Result<()> {
    if !validate_oem_inf_name(oem_inf) {
        return Err(BackupError::InvalidInf);
    }
    reject_link(destination)?;
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

    Ok(())
}

fn truncate(value: &str) -> String {
    value
        .chars()
        .take(512)
        .collect::<String>()
        .replace(['\r', '\n'], " ")
}
