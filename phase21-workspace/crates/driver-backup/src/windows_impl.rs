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

// A held child, rather than directory sharing flags, prevents an in-place reparse tag:
// Windows refuses SET_REPARSE_POINT on a nonempty directory. Denying DELETE sharing on
// every held child keeps that condition true. Existing directories are never populated.
const SENTINEL_NAME: &str = ".aethercore-export-guard";

pub(crate) struct RootGuard {
    handles: Vec<fs::File>,
    sentinels: Vec<fs::File>,
    destination: PathBuf,
    leaf: usize,
    leaf_has_sentinel: bool,
    witness_budget: usize,
}

impl RootGuard {
    pub(crate) fn admit_export(&self) -> Result<()> {
        let mut entries = fs::read_dir(&self.destination)?;
        if entries
            .next()
            .transpose()?
            .is_none_or(|entry| entry.file_name() != SENTINEL_NAME)
            || entries.next().is_some()
        {
            return Err(BackupError::InvalidRoot);
        }
        Ok(())
    }

    pub(crate) fn finish_export(&mut self) -> Result<()> {
        // Acquire a real exported child's no-DELETE lease before releasing any sentinel.
        // The manifest is not a permanent exception: it does not exist at this handoff.
        let mut budget = self.witness_budget;
        let mut witnesses = Vec::new();
        pin_existing_child(
            &self.destination,
            &self.handles[self.leaf],
            &mut witnesses,
            &mut budget,
        )?;
        self.handles.extend(witnesses);
        self.witness_budget = budget;
        self.sentinels.clear();
        self.leaf_has_sentinel = false;
        Ok(())
    }
}

// The sole create/open helper: a one-component name is resolved against the retained
// parent object, with DONT_REPARSE and OPEN_REPARSE_POINT. No path-based write is used
// while a newly created directory could still be empty.
fn relative_file(
    parent: &fs::File,
    name: &std::ffi::OsStr,
    directory: bool,
    create: bool,
    sentinel: bool,
) -> Result<(fs::File, bool)> {
    use std::os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle},
    };
    use windows::{
        Wdk::{Foundation::OBJECT_ATTRIBUTES, Storage::FileSystem::*},
        Win32::{
            Foundation::{HANDLE, OBJECT_ATTRIBUTE_FLAGS, UNICODE_STRING},
            Storage::FileSystem::{FILE_ACCESS_RIGHTS, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_MODE},
            System::IO::IO_STATUS_BLOCK,
        },
        core::PWSTR,
    };
    let mut name: Vec<u16> = name.encode_wide().collect();
    if name.is_empty()
        || name.len() > 255
        || name == [46]
        || name == [46, 46]
        || name.iter().any(|c| [0, 47, 92, 58].contains(c))
    {
        return Err(BackupError::InvalidRoot);
    }
    let unicode = UNICODE_STRING {
        Length: (name.len() * 2) as u16,
        MaximumLength: (name.len() * 2) as u16,
        Buffer: PWSTR(name.as_mut_ptr()),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: std::mem::size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: HANDLE(parent.as_raw_handle()),
        ObjectName: &unicode,
        Attributes: OBJECT_ATTRIBUTE_FLAGS(0x1040),
        ..Default::default()
    }; // CASE_INSENSITIVE | DONT_REPARSE
    let mut handle = HANDLE::default();
    let mut status = IO_STATUS_BLOCK::default();
    let options = FILE_OPEN_REPARSE_POINT.0
        | FILE_SYNCHRONOUS_IO_NONALERT.0
        | if directory {
            FILE_DIRECTORY_FILE.0
        } else {
            FILE_NON_DIRECTORY_FILE.0
        }
        | if sentinel { FILE_DELETE_ON_CLOSE.0 } else { 0 };
    let result = unsafe {
        NtCreateFile(
            &mut handle,
            FILE_ACCESS_RIGHTS(
                0x0010_0080
                    | if directory { 0x21 } else { 1 }
                    | if sentinel { 0x0001_0000 } else { 0 },
            ),
            &attributes,
            &mut status,
            None,
            FILE_FLAGS_AND_ATTRIBUTES(0x80),
            FILE_SHARE_MODE(3),
            if sentinel {
                FILE_CREATE
            } else if create {
                FILE_OPEN_IF
            } else {
                FILE_OPEN
            },
            NTCREATEFILE_CREATE_OPTIONS(options),
            None,
            0,
        )
    };
    result
        .ok()
        .map_err(|e| BackupError::PnpUtil(format!("anchored export path open failed: {e}")))?;
    // NtCreateFile succeeded: ownership transfers immediately to File, including every
    // subsequent metadata/error path. IO_STATUS_BLOCK's FILE_CREATED value is documented 2.
    let file = unsafe { fs::File::from_raw_handle(handle.0) };
    let metadata = file.metadata()?;
    if metadata.file_attributes() & 0x400 != 0 || metadata.is_dir() != directory {
        return Err(BackupError::InvalidRoot);
    }
    Ok((file, status.Information == 2))
}

fn pin_existing_child(
    path: &Path,
    directory: &fs::File,
    handles: &mut Vec<fs::File>,
    budget: &mut usize,
) -> Result<()> {
    if path.components().count() > 128 {
        return Err(BackupError::InvalidRoot);
    }
    for entry in fs::read_dir(path)? {
        if *budget == 0 {
            return Err(BackupError::InvalidRoot);
        }
        *budget -= 1;
        let entry = entry?;
        if entry.file_name() == SENTINEL_NAME {
            continue;
        }
        let metadata = entry.metadata()?;
        if metadata.file_attributes() & 0x400 != 0 {
            continue;
        }
        let child = match relative_file(
            directory,
            &entry.file_name(),
            metadata.is_dir(),
            false,
            false,
        ) {
            Ok((file, _)) => file,
            Err(_) => continue,
        };
        if metadata.is_dir() {
            let count = handles.len();
            if pin_existing_child(&entry.path(), &child, handles, budget).is_err() {
                handles.truncate(count);
                continue;
            }
        } else if !metadata.is_file() {
            continue;
        }
        if directory.metadata()?.file_attributes() & 0x400 != 0 {
            return Err(BackupError::InvalidRoot);
        }
        handles.push(child);
        return Ok(());
    }
    Err(BackupError::InvalidRoot)
}

pub(crate) fn guard_root(path: &Path, create: bool) -> Result<RootGuard> {
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Component, Prefix};
    let components: Vec<_> = path.components().collect();
    if components.len() < 3
        || components.len() > 128
        || !path.is_absolute()
        || !matches!(components.first(), Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
        || components
            .iter()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(BackupError::InvalidRoot);
    }
    let volume: PathBuf = components.iter().take(2).collect();
    let wide: Vec<u16> = volume.as_os_str().encode_wide().chain([0]).collect();
    if unsafe { GetDriveTypeW(PCWSTR(wide.as_ptr())) } != DRIVE_FIXED {
        return Err(BackupError::InvalidRoot);
    }
    let root = fs::OpenOptions::new()
        .access_mode(0x81)
        .share_mode(3)
        .custom_flags(0x0220_0000)
        .open(&volume)?;
    if root.metadata()?.file_attributes() & 0x400 != 0 {
        return Err(BackupError::InvalidRoot);
    }
    let mut guard = RootGuard {
        handles: vec![root],
        sentinels: Vec::new(),
        destination: path.to_owned(),
        leaf: 0,
        leaf_has_sentinel: false,
        witness_budget: crate::MAX_BACKUP_FILES - 1,
    };
    let mut cursor = volume;
    let mut budget = crate::MAX_BACKUP_FILES - 1;
    for (index, component) in components.iter().enumerate().skip(2) {
        let Component::Normal(name) = component else {
            return Err(BackupError::InvalidRoot);
        };
        if budget == 0 {
            return Err(BackupError::InvalidRoot);
        }
        budget -= 1;
        // Reading an existing child first gives the parent a held witness without scanning
        // the drive root or writing there. A missing child may be created only after an
        // existing witness has been retained (or a previously owned sentinel is still held).
        let (child, created) =
            match relative_file(&guard.handles[guard.leaf], name, true, false, false) {
                Ok(child) => child,
                Err(error) if !create => return Err(error),
                Err(_) => {
                    let has_sentinel = guard.leaf_has_sentinel;
                    // Only newly created directories receive sentinels below. Existing directories
                    // need a read-only existing witness; an empty unowned ancestor is refused.
                    if !has_sentinel {
                        let mut witnesses = Vec::new();
                        pin_existing_child(
                            &cursor,
                            &guard.handles[guard.leaf],
                            &mut witnesses,
                            &mut budget,
                        )?;
                        guard.handles.extend(witnesses);
                    }
                    relative_file(&guard.handles[guard.leaf], name, true, true, false)?
                }
            };
        if guard.handles[guard.leaf].metadata()?.file_attributes() & 0x400 != 0 {
            return Err(BackupError::InvalidRoot);
        }
        cursor.push(name);
        guard.leaf = guard.handles.len();
        guard.handles.push(child);
        guard.leaf_has_sentinel = false;
        let leaf = index + 1 == components.len();
        let mut witnesses = Vec::new();
        if created || (create && leaf && fs::read_dir(&cursor)?.next().is_none()) {
            if budget == 0 {
                return Err(BackupError::InvalidRoot);
            }
            budget -= 1;
            let (sentinel, _) = relative_file(
                &guard.handles[guard.leaf],
                std::ffi::OsStr::new(SENTINEL_NAME),
                false,
                true,
                true,
            )?;
            // A concurrent tag write cannot redirect the relative create. Refuse a tagged
            // object before a path-based caller receives this lease.
            if guard.handles[guard.leaf].metadata()?.file_attributes() & 0x400 != 0 {
                return Err(BackupError::InvalidRoot);
            }
            guard.sentinels.push(sentinel);
            guard.leaf_has_sentinel = true;
        } else if leaf {
            pin_existing_child(
                &cursor,
                &guard.handles[guard.leaf],
                &mut witnesses,
                &mut budget,
            )?;
            guard.handles.extend(witnesses);
        }
    }
    guard.witness_budget = budget;
    Ok(guard)
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
        fs::write(root.join("fixture-witness"), b"fixture").unwrap();
        root
    }

    #[test]
    fn witness_handoff_preserves_export_and_sibling_availability() {
        let root = fixture("handoff");
        let destination = root.join("new-parent/leaf");
        let mut guard = guard_root(&destination, true).unwrap();
        guard.admit_export().unwrap();
        assert!(fs::remove_file(destination.join(SENTINEL_NAME)).is_err());
        fs::write(destination.join("package.inf"), b"[Version]").unwrap();
        fs::write(root.join("unrelated"), b"sibling").unwrap();
        fs::create_dir(root.join("sibling")).unwrap();
        guard.finish_export().unwrap();
        assert!(!destination.join(SENTINEL_NAME).exists());
        assert!(!root.join("new-parent").join(SENTINEL_NAME).exists());
        assert!(fs::remove_file(destination.join("package.inf")).is_err());
        let evidence = crate::seal_export("oem7.inf", &destination).unwrap();
        assert_eq!(evidence.file_count, 1);
        crate::verify_export(&evidence).unwrap();
        drop(guard);
        fs::remove_file(destination.join("package.inf")).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unowned_empty_ancestors_are_not_populated_and_failed_export_drops_owned_sentinel() {
        let root = fixture("unowned");
        let empty = root.join("empty");
        fs::create_dir(&empty).unwrap();
        assert!(guard_root(&empty.join("child"), true).is_err());
        assert!(empty.read_dir().unwrap().next().is_none());
        assert!(guard_root(&empty, false).is_err());
        let destination = root.join("owned");
        let mut guard = guard_root(&destination, true).unwrap();
        guard.admit_export().unwrap();
        assert!(guard.finish_export().is_err());
        drop(guard);
        assert!(destination.read_dir().unwrap().next().is_none());
        fs::remove_dir_all(root).unwrap();
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
        let cmd = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32")
            .join("cmd.exe");
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
        let cmd = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32")
            .join("cmd.exe");
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
        // Even a tag raced before the first owned witness cannot redirect a native
        // RootDirectory-relative create into its junction target.
        let raced = root.join("raced");
        fs::create_dir(&raced).unwrap();
        let original = open(&raced, 0x81).unwrap();
        let attacker = open(&raced, 0x4000_0000).unwrap();
        set(&attacker).unwrap();
        let created = relative_file(
            &original,
            std::ffi::OsStr::new("must-not-escape"),
            false,
            true,
            true,
        );
        drop(created);
        assert!(
            !target.join("must-not-escape").exists(),
            "handle-relative creation cannot follow a replaced root tag"
        );
        drop(attacker);
        drop(original);
        fs::remove_dir(&raced).unwrap();
        let nonempty = root.join("nonempty");
        fs::create_dir(&nonempty).unwrap();
        fs::write(nonempty.join("anchor"), b"fixture").unwrap();
        let anchor = fs::OpenOptions::new()
            .read(true)
            .share_mode(3)
            .open(nonempty.join("anchor"))
            .unwrap();
        let nonempty_handle = open(&nonempty, 0x4000_0000).unwrap();
        let refused = set(&nonempty_handle).expect_err("a held child keeps its parent nonempty");
        assert_eq!(
            refused.code().0 as u32,
            0x8007_0091,
            "native ERROR_DIR_NOT_EMPTY"
        );
        eprintln!("native nonempty tag control: {:?}", refused.code());
        drop(nonempty_handle);
        drop(anchor);
        let destination = root.join("guarded");
        let mut guards = guard_root(&destination, true).unwrap();
        // Metadata-only handles may still be shared by the OS. They must not change the tag.
        for access in [0x80, 0x100, 0x4000_0000] {
            // READ_ATTRIBUTES / WRITE_ATTRIBUTES
            if let Ok(file) = open(&destination, access) {
                let error = set(&file).expect_err("held witness prevents in-place tag replacement");
                eprintln!("held tag control access={access:#x}: {:?}", error.code());
                if access == 0x4000_0000 {
                    assert_eq!(
                        error.code().0 as u32,
                        0x8007_0091,
                        "full writer must fail because the held directory is nonempty"
                    );
                }
            }
        }
        assert_eq!(
            fs::symlink_metadata(&destination)
                .unwrap()
                .file_attributes()
                & 0x400,
            0
        );
        let sibling = root.join("sibling-junction");
        let output = Command::new(
            PathBuf::from(std::env::var_os("SystemRoot").unwrap())
                .join("System32")
                .join("cmd.exe"),
        )
        .args(["/D", "/C", "mklink", "/J"])
        .arg(spelling(&sibling))
        .arg(spelling(&target))
        .output()
        .unwrap();
        assert!(
            output.status.success(),
            "sibling creation must remain available: {:?}",
            output
        );
        fs::remove_dir(sibling).unwrap();
        let ancestor_handle = open(&root, 0x4000_0000).unwrap();
        assert_eq!(
            set(&ancestor_handle).unwrap_err().code().0 as u32,
            0x8007_0091
        );
        drop(ancestor_handle);
        fs::write(destination.join("child.inf"), b"[Version]").unwrap();
        guards.finish_export().unwrap();
        // File presence is the witness; truncation cannot make the directory empty.
        fs::write(destination.join("child.inf"), b"").unwrap();
        assert!(fs::remove_file(destination.join("child.inf")).is_err());
        let after_handoff = open(&destination, 0x4000_0000).unwrap();
        assert_eq!(
            set(&after_handoff).unwrap_err().code().0 as u32,
            0x8007_0091
        );
        drop(after_handoff);
        drop(guards);
        fs::remove_dir(&link).unwrap();
        fs::remove_dir(&control).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
