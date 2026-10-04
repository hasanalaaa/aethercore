#![allow(unsafe_code)]

use std::{
    fs,
    os::windows::{
        fs::{MetadataExt, OpenOptionsExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::{BackupError, BackupEvidence, Result, reject_link, validate_oem_inf_name};
use windows::{
    Win32::{
        Devices::DeviceAndDriverInstallation::SetupGetInfDriverStoreLocationW,
        Storage::FileSystem::{GetDiskFreeSpaceExW, GetDiskFreeSpaceW, GetDriveTypeW},
    },
    core::PCWSTR,
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
// GetDriveTypeW's documented DRIVE_FIXED result; no extra Windows feature for this constant.
const DRIVE_FIXED: u32 = 3;

/// Lock each ancestor before creating its child. Omitting DELETE sharing prevents an ancestor
/// or the export directory being renamed/replaced until export/seal/re-read has finished.
pub(crate) fn guard_root(path: &Path, create: bool) -> Result<Vec<fs::File>> {
    use std::path::{Component, Prefix};
    if !path.is_absolute()
        || !matches!(path.components().next(), Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
    {
        return Err(BackupError::InvalidRoot);
    }
    use std::os::windows::ffi::OsStrExt;
    let volume: PathBuf = path.components().take(2).collect();
    let volume: Vec<u16> = volume.as_os_str().encode_wide().chain([0]).collect();
    // Reject mapped/network/removable destinations before creating anything there.
    if unsafe { GetDriveTypeW(PCWSTR(volume.as_ptr())) } != DRIVE_FIXED {
        return Err(BackupError::InvalidRoot);
    }
    let mut cursor = PathBuf::new();
    let mut handles = Vec::new();
    for component in path.components() {
        if matches!(component, Component::ParentDir | Component::CurDir) {
            return Err(BackupError::InvalidRoot);
        }
        cursor.push(component);
        if matches!(component, Component::Prefix(_)) {
            continue;
        }
        if create && !cursor.try_exists()? {
            fs::create_dir(&cursor)?;
        }
        let file = fs::OpenOptions::new()
            .access_mode(0x81) // LIST_DIRECTORY | READ_ATTRIBUTES
            .share_mode(1) // READ only: no writer may replace an in-place reparse tag; no DELETE.
            .custom_flags(0x0220_0000) // BACKUP_SEMANTICS | OPEN_REPARSE_POINT
            .open(&cursor)?;
        let metadata = file.metadata()?;
        if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
            return Err(BackupError::InvalidRoot);
        }
        handles.push(file);
    }
    Ok(handles)
}

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

    Ok(())
}

fn truncate(value: &str) -> String {
    value
        .chars()
        .take(512)
        .collect::<String>()
        .replace(['\r', '\n'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "aethercore-export-{name}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir(&root).unwrap();
        root
    }

    #[test]
    fn held_ancestor_handles_prevent_directory_replacement() {
        let root = fixture("locked");
        let destination = root.join("parent/child");
        let guards = guard_root(&destination, true).unwrap();
        assert!(fs::rename(root.join("parent"), root.join("moved")).is_err());
        assert!(fs::rename(&destination, root.join("other")).is_err());
        drop(guards);
        fs::rename(root.join("parent"), root.join("moved")).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ancestor_junction_is_refused_before_creating_or_exporting_a_child() {
        let root = fixture("junction");
        let target = root.join("target");
        fs::create_dir(&target).unwrap();
        let link = root.join("link");
        let cmd = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/cmd.exe");
        // cmd's built-in rejects the canonical \\?\ prefix; fixture paths below MAX_PATH use
        // their ordinary drive spelling. Production guards retain the original path semantics.
        let link_arg = link
            .to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_string();
        let target_arg = target
            .to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_string();
        let output = Command::new(cmd)
            .args(["/D", "/C", "mklink", "/J"])
            .arg(link_arg)
            .arg(target_arg)
            .output()
            .unwrap();
        assert!(output.status.success(), "junction fixture: {:?}", output);
        let called = std::cell::Cell::new(false);
        assert!(
            crate::export_into(
                "oem7.inf",
                &link.join("child"),
                |_| Ok((1, 1)),
                |_| {
                    called.set(true);
                    Ok(())
                }
            )
            .is_err()
        );
        assert!(!called.get());
        assert!(!target.join("child").exists());
        fs::remove_dir(link).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn held_directory_refuses_the_writable_handle_needed_to_set_a_reparse_tag() {
        let root = fixture("tag-write");
        let destination = root.join("empty");
        let guards = guard_root(&destination, true).unwrap();
        let writable = || {
            fs::OpenOptions::new()
                .access_mode(0x4000_0000) // GENERIC_WRITE
                .share_mode(3)
                .custom_flags(0x0220_0000)
                .open(&destination)
        };
        let while_held = writable();
        drop(guards);
        assert!(
            writable().is_ok(),
            "positive control: ordinary empty directory allows tag-write access"
        );
        let held_was_refused = while_held.is_err();
        drop(while_held);
        fs::remove_dir_all(root).unwrap();
        assert!(
            held_was_refused,
            "holding the export path must refuse tag-write access"
        );
    }

    #[test]
    fn held_directory_refuses_in_place_reparse_tag_changes() {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::{
            Foundation::HANDLE,
            System::{
                IO::DeviceIoControl,
                Ioctl::{FSCTL_GET_REPARSE_POINT, FSCTL_SET_REPARSE_POINT},
            },
        };
        let root = fixture("tag-ioctl");
        let target = root.join("target");
        fs::create_dir(&target).unwrap();
        let link = root.join("template");
        let spelling = |p: &Path| p.to_string_lossy().trim_start_matches(r"\\?\").to_string();
        let cmd = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/cmd.exe");
        assert!(
            Command::new(cmd)
                .args(["/D", "/C", "mklink", "/J"])
                .arg(spelling(&link))
                .arg(spelling(&target))
                .output()
                .unwrap()
                .status
                .success()
        );
        let open = |path: &Path, access| {
            fs::OpenOptions::new()
                .access_mode(access)
                .share_mode(3)
                .custom_flags(0x0220_0000)
                .open(path)
        };
        let template = open(&link, 0x80).unwrap();
        let mut data = vec![0u8; 16 * 1024];
        let mut returned = 0;
        unsafe {
            DeviceIoControl(
                HANDLE(template.as_raw_handle()),
                FSCTL_GET_REPARSE_POINT,
                None,
                0,
                Some(data.as_mut_ptr().cast()),
                data.len() as u32,
                Some(&mut returned),
                None,
            )
        }
        .unwrap();
        data.truncate(returned as usize);
        drop(template);
        let set = |file: &fs::File| {
            let mut returned = 0;
            unsafe {
                DeviceIoControl(
                    HANDLE(file.as_raw_handle()),
                    FSCTL_SET_REPARSE_POINT,
                    Some(data.as_ptr().cast()),
                    data.len() as u32,
                    None,
                    0,
                    Some(&mut returned),
                    None,
                )
            }
        };
        let control = root.join("control");
        fs::create_dir(&control).unwrap();
        let writable = open(&control, 0x4000_0000).unwrap();
        set(&writable)
            .expect("positive control: actual junction tag can be set on an unguarded directory");
        drop(writable);
        assert_ne!(
            fs::symlink_metadata(&control).unwrap().file_attributes() & 0x400,
            0
        );
        let destination = root.join("guarded");
        let guards = guard_root(&destination, true).unwrap();
        // Metadata-only handles may still be shared by the OS. They must not change the tag.
        for access in [0x80, 0x100] {
            // READ_ATTRIBUTES / WRITE_ATTRIBUTES
            if let Ok(file) = open(&destination, access) {
                assert!(set(&file).is_err());
            }
        }
        assert_eq!(
            fs::symlink_metadata(&destination)
                .unwrap()
                .file_attributes()
                & 0x400,
            0
        );
        fs::write(destination.join("child.inf"), b"[Version]").unwrap();
        drop(guards);
        fs::remove_dir(&link).unwrap();
        fs::remove_dir(&control).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
