use std::{
    ffi::OsStr,
    os::windows::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawHandle,
    },
    path::{Path, PathBuf},
    time::Duration,
};

use windows::{
    Win32::{
        Foundation::{FILETIME, HANDLE},
        Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_ATTRIBUTE_NORMAL,
            FILE_ATTRIBUTE_REPARSE_POINT, FILE_DISPOSITION_INFO, FILE_FLAG_BACKUP_SEMANTICS,
            FILE_FLAG_OPEN_REPARSE_POINT, FILE_FLAGS_AND_ATTRIBUTES, FILE_ID_INFO,
            FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
            FileDispositionInfo, FileIdInfo, GETFINALPATHNAMEBYHANDLE_FLAGS,
            GetFileInformationByHandle, GetFileInformationByHandleEx, GetFinalPathNameByHandleW,
            OPEN_EXISTING, SetFileInformationByHandle,
        },
    },
    core::PCWSTR,
};

use super::{
    CleanerError, CleanupCandidate, CleanupMutationLease, CleanupPlatform, Result, candidate,
    cap_files, evidence, older_than,
};
use aethercore_operation_engine::{CleanupDeleteAction, CleanupFileEvidence};
use aethercore_windows_foundation::{MachineMutationGuard, OwnedHandle};

const DELETE_ACCESS: u32 = 0x0001_0000;

pub struct WindowsCleanupPlatform;

impl CleanupPlatform for WindowsCleanupPlatform {
    fn scan(&self) -> Result<Vec<CleanupCandidate>> {
        scan_impl(true)
    }

    /// The real cross-process lease: the installer-provisioned ProgramData lock file,
    /// acquired exactly as before. Only the route to it moved (DBT-P61-002) — the
    /// boxed value is the guard itself, released by its own Drop.
    fn acquire_mutation_lease(&self) -> Result<CleanupMutationLease> {
        let guard = MachineMutationGuard::try_acquire()
            .map_err(|error| CleanerError::Safety(error.to_string()))?
            .ok_or(CleanerError::MutationBusy)?;
        Ok(Box::new(guard))
    }

    fn scan_passive(&self) -> Result<Vec<CleanupCandidate>> {
        let mut output = scan_impl(false)?;
        output.retain(|candidate| !candidate.requires_explicit_confirmation);
        Ok(output)
    }

    fn delete_action(&self, action: &CleanupDeleteAction) -> Result<(u64, u64, String)> {
        let mut deleted = 0u64;
        let mut skipped = 0u64;
        for file in &action.files {
            match delete_evidence(file) {
                Ok(bytes) => deleted = deleted.saturating_add(bytes),
                Err(_) => skipped = skipped.saturating_add(file.size_bytes),
            }
        }
        Ok((
            deleted,
            skipped,
            format!("{} provider completed", action.provider),
        ))
    }
}

fn scan_impl(include_profile_roots: bool) -> Result<Vec<CleanupCandidate>> {
    let mut output = Vec::new();
    let system_root = PathBuf::from(
        std::env::var_os("SystemRoot").unwrap_or_else(|| OsStr::new(r"C:\Windows").to_os_string()),
    );
    let program_data = PathBuf::from(
        std::env::var_os("ProgramData")
            .unwrap_or_else(|| OsStr::new(r"C:\ProgramData").to_os_string()),
    );

    push_root(
        &mut output,
        "WindowsTemp",
        "Windows temporary files",
        "Temporary files older than 48 hours under the Windows temp root.",
        &system_root.join("Temp"),
        Duration::from_secs(48 * 3600),
        true,
        false,
    )?;

    let users = system_root
        .parent()
        .unwrap_or(Path::new(r"C:\"))
        .join("Users");
    if include_profile_roots && let Ok(entries) = std::fs::read_dir(&users) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if matches!(
                name.to_ascii_lowercase().as_str(),
                "public" | "default" | "default user" | "all users"
            ) {
                continue;
            }
            let profile = entry.path();
            push_root(
                &mut output,
                "UserTemp",
                &format!("{name} temporary files"),
                "Profile-specific temp files older than seven days. Because the service cannot infer that this is the interactive caller’s profile, this category requires explicit review.",
                &profile.join(r"AppData\Local\Temp"),
                Duration::from_secs(7 * 24 * 3600),
                false,
                true,
            )?;
            push_root(
                &mut output,
                "ShaderCache",
                &format!("{name} Direct3D shader cache"),
                "Profile-specific rebuildable Direct3D cache files older than 72 hours. This category requires explicit review.",
                &profile.join(r"AppData\Local\D3DSCache"),
                Duration::from_secs(72 * 3600),
                false,
                true,
            )?;
        }
    }

    if include_profile_roots {
        push_root(
            &mut output,
            "WER",
            "Windows Error Reporting archives",
            "Archived Windows Error Reporting files. Keep these when diagnosing crashes.",
            &program_data.join(r"Microsoft\Windows\WER\ReportArchive"),
            Duration::ZERO,
            false,
            true,
        )?;
        push_root(
            &mut output,
            "WER",
            "Windows Error Reporting queue",
            "Queued Windows Error Reporting files. Keep these when diagnosing crashes.",
            &program_data.join(r"Microsoft\Windows\WER\ReportQueue"),
            Duration::ZERO,
            false,
            true,
        )?;
        push_root(
            &mut output,
            "CrashDumps",
            "Windows minidumps",
            "Windows crash minidumps. Keep these when diagnosing BSODs.",
            &system_root.join("Minidump"),
            Duration::ZERO,
            false,
            true,
        )?;

        if let Some(memory_dump) = single_file(&system_root.join("MEMORY.DMP"), &system_root) {
            let (files, truncated) = cap_files(vec![memory_dump]);
            output.push(candidate(
                "CrashDumps",
                "Windows memory dump",
                "Full kernel memory dump. Keep it when diagnosing crashes.",
                files,
                false,
                true,
                "Files",
                truncated,
            ));
        }
    }

    output.retain(|candidate| candidate.reclaimable_bytes > 0);
    Ok(output)
}

// Same eight-field shape as `candidate`, which this forwards to and which
// carries the identical allow (lib.rs:1061).
#[allow(clippy::too_many_arguments)]
fn push_root(
    output: &mut Vec<CleanupCandidate>,
    provider: &str,
    title: &str,
    description: &str,
    root: &Path,
    age: Duration,
    selected: bool,
    explicit: bool,
) -> Result<()> {
    let files = scan_root(root, age)?;
    let (files, truncated) = cap_files(files);
    if !files.is_empty() {
        output.push(candidate(
            provider,
            title,
            description,
            files,
            selected,
            explicit,
            "Files",
            truncated,
        ));
    }
    Ok(())
}

fn scan_root(root: &Path, age: Duration) -> Result<Vec<CleanupFileEvidence>> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let (_root_guard, root_final_path) = open_stable_root(root)?;

    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(_) => continue,
        } {
            let Ok(entry) = entry else {
                continue;
            };
            let path = entry.path();
            let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
                continue;
            }
            if metadata.is_dir() {
                stack.push(path);
                continue;
            }
            if !metadata.is_file() || !older_than(&metadata, age) {
                continue;
            }
            if let Ok(value) = evidence_from_handle(&path, root, &root_final_path) {
                files.push(value);
            }
        }
    }
    Ok(files)
}

fn single_file(path: &Path, root: &Path) -> Option<CleanupFileEvidence> {
    let (_root_guard, root_final_path) = open_stable_root(root).ok()?;
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
        return None;
    }
    evidence_from_handle(path, root, &root_final_path).ok()
}

fn reject_reparse(path: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
        return Err(CleanerError::Safety(format!(
            "reparse point rejected: {}",
            path.display()
        )));
    }
    Ok(())
}

fn reject_ancestor_reparse_chain(path: &Path) -> Result<()> {
    let mut cursor = PathBuf::new();
    for component in path.components() {
        cursor.push(component.as_os_str());
        if cursor.exists() {
            reject_reparse(&cursor)?;
        }
    }
    Ok(())
}

fn open_stable_root(root: &Path) -> Result<(std::fs::File, PathBuf)> {
    reject_ancestor_reparse_chain(root)?;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .share_mode((FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE).0)
        .custom_flags((FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT).0)
        .open(root)?;
    let metadata = file.metadata()?;
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
        return Err(CleanerError::Safety(format!(
            "approved cleanup root became a reparse point: {}",
            root.display()
        )));
    }
    let handle = HANDLE(file.as_raw_handle());
    let final_root = strip_device_prefix(&final_path(handle)?);
    if !final_root.is_absolute() {
        return Err(CleanerError::Safety(
            "approved cleanup root did not resolve to an absolute final path".into(),
        ));
    }
    Ok((file, final_root))
}

struct HandleEvidence {
    size_bytes: u64,
    modified_unix_ms: i64,
    volume_serial_number: u64,
    file_id_128: String,
    attributes: u32,
}

fn evidence_from_handle(
    path: &Path,
    root: &Path,
    root_final_path: &Path,
) -> Result<CleanupFileEvidence> {
    let wide_path = wide(path.as_os_str());
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide_path.as_ptr()),
            FILE_READ_ATTRIBUTES.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(FILE_ATTRIBUTE_NORMAL.0 | FILE_FLAG_OPEN_REPARSE_POINT.0),
            None,
        )
    }
    .map_err(|error| CleanerError::Safety(error.to_string()))?;
    let _guard = OwnedHandle::new(handle);
    let final_target = strip_device_prefix(&final_path(handle)?);
    if !final_target.starts_with(root_final_path) {
        return Err(CleanerError::Safety(
            "scan target escaped approved cleanup root".into(),
        ));
    }
    let current = query_handle_evidence(handle)?;
    if current.attributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
        return Err(CleanerError::Safety(
            "scan target is a reparse point".into(),
        ));
    }
    Ok(evidence(
        path,
        root,
        root_final_path,
        current.size_bytes,
        current.modified_unix_ms,
        current.volume_serial_number,
        current.file_id_128,
    ))
}

fn query_handle_evidence(handle: HANDLE) -> Result<HandleEvidence> {
    let mut basic = BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(handle, &mut basic) }
        .map_err(|error| CleanerError::Safety(error.to_string()))?;
    let mut file_id = FILE_ID_INFO::default();
    unsafe {
        GetFileInformationByHandleEx(
            handle,
            FileIdInfo,
            &mut file_id as *mut FILE_ID_INFO as *mut _,
            std::mem::size_of::<FILE_ID_INFO>() as u32,
        )
    }
    .map_err(|error| CleanerError::Safety(error.to_string()))?;
    let size_bytes = ((basic.nFileSizeHigh as u64) << 32) | basic.nFileSizeLow as u64;
    let modified_unix_ms = filetime_to_unix_ms(basic.ftLastWriteTime)?;
    Ok(HandleEvidence {
        size_bytes,
        modified_unix_ms,
        volume_serial_number: file_id.VolumeSerialNumber,
        file_id_128: encode_file_id(&file_id.FileId.Identifier),
        attributes: basic.dwFileAttributes,
    })
}

fn filetime_to_unix_ms(value: FILETIME) -> Result<i64> {
    const WINDOWS_TO_UNIX_100NS: u64 = 116_444_736_000_000_000;
    let ticks = ((value.dwHighDateTime as u64) << 32) | value.dwLowDateTime as u64;
    if ticks < WINDOWS_TO_UNIX_100NS {
        return Err(CleanerError::Safety(
            "invalid handle modification time".into(),
        ));
    }
    Ok(((ticks - WINDOWS_TO_UNIX_100NS) / 10_000) as i64)
}

fn encode_file_id(value: &[u8; 16]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(32);
    for &b in value {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn delete_evidence(file: &CleanupFileEvidence) -> Result<u64> {
    let path = PathBuf::from(&file.path);
    let root = PathBuf::from(&file.root);
    if !path.is_absolute() || !root.is_absolute() {
        return Err(CleanerError::Safety(
            "cleanup target is not absolute".into(),
        ));
    }

    let (_root_guard, current_root_final) = open_stable_root(&root)?;
    let expected_root_final = PathBuf::from(&file.root_final_path);
    if current_root_final != expected_root_final {
        return Err(CleanerError::Safety(
            "approved cleanup root identity changed since scan".into(),
        ));
    }
    reject_path_chain(&root, &path)?;

    let wide_path = wide(path.as_os_str());
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide_path.as_ptr()),
            DELETE_ACCESS | FILE_READ_ATTRIBUTES.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(FILE_ATTRIBUTE_NORMAL.0 | FILE_FLAG_OPEN_REPARSE_POINT.0),
            None,
        )
    }
    .map_err(|error| CleanerError::Safety(error.to_string()))?;
    let guard = OwnedHandle::new(handle);

    let final_path = strip_device_prefix(&final_path(handle)?);
    if !final_path.starts_with(&current_root_final) {
        return Err(CleanerError::Safety(
            "final path escaped approved cleanup root".into(),
        ));
    }

    let current = query_handle_evidence(handle)?;
    if current.attributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
        return Err(CleanerError::Safety(
            "cleanup target became a reparse point".into(),
        ));
    }
    if current.size_bytes != file.size_bytes || current.modified_unix_ms != file.modified_unix_ms {
        return Err(CleanerError::Safety(
            "file size or timestamp changed since cleanup scan".into(),
        ));
    }
    if current.volume_serial_number != file.volume_serial_number
        || current.file_id_128 != file.file_id_128
    {
        return Err(CleanerError::Safety(
            "file identity changed since cleanup scan".into(),
        ));
    }

    let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    unsafe {
        SetFileInformationByHandle(
            handle,
            FileDispositionInfo,
            &disposition as *const _ as *const _,
            std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    }
    .map_err(|error| CleanerError::Safety(error.to_string()))?;

    drop(guard);
    Ok(file.size_bytes)
}

fn reject_path_chain(root: &Path, path: &Path) -> Result<()> {
    if !path.starts_with(root) {
        return Err(CleanerError::Safety(
            "target is outside approved root".into(),
        ));
    }

    let mut cursor = root.to_path_buf();
    reject_reparse(&cursor)?;
    let relative = path
        .strip_prefix(root)
        .map_err(|_| CleanerError::Safety("target root mismatch".into()))?;
    for component in relative.components() {
        cursor.push(component.as_os_str());
        if cursor.exists() {
            reject_reparse(&cursor)?;
        }
    }
    Ok(())
}

fn final_path(handle: HANDLE) -> Result<PathBuf> {
    let mut buffer = vec![0u16; 32_768];
    let length = unsafe {
        GetFinalPathNameByHandleW(handle, &mut buffer, GETFINALPATHNAMEBYHANDLE_FLAGS(0))
    };
    if length == 0 || length as usize >= buffer.len() {
        return Err(CleanerError::Safety(
            "unable to resolve final path by handle".into(),
        ));
    }
    Ok(PathBuf::from(String::from_utf16_lossy(
        &buffer[..length as usize],
    )))
}

fn strip_device_prefix(path: &Path) -> PathBuf {
    let value = path.to_string_lossy();
    if let Some(stripped) = value.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod phase9_tests {
    use super::*;
    use uuid::Uuid;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("aethercore-phase9-fileid-{}", Uuid::new_v4()))
    }

    #[test]
    fn replacement_with_same_path_and_shape_is_rejected_by_file_identity() {
        let root = temp_dir();
        std::fs::create_dir_all(&root).expect("create test root");
        let path = root.join("candidate.tmp");
        std::fs::write(&path, b"AAAA").expect("write original");

        let (_guard, root_final) = open_stable_root(&root).expect("open root");
        let mut approved = evidence_from_handle(&path, &root, &root_final).expect("scan evidence");
        let original_file_id = approved.file_id_128.clone();

        std::fs::remove_file(&path).expect("replace original");
        std::fs::write(&path, b"BBBB").expect("write replacement");
        let replacement =
            evidence_from_handle(&path, &root, &root_final).expect("replacement evidence");
        assert_ne!(
            original_file_id, replacement.file_id_128,
            "fixture must allocate a new file object"
        );

        // Normalize the mutable metadata to the replacement so this assertion specifically proves
        // that the volume/file-ID barrier, not size or timestamp drift, stops the deletion.
        approved.size_bytes = replacement.size_bytes;
        approved.modified_unix_ms = replacement.modified_unix_ms;
        assert!(
            matches!(delete_evidence(&approved), Err(CleanerError::Safety(message)) if message.contains("file identity changed"))
        );
        assert!(
            path.exists(),
            "replacement file must survive identity mismatch"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn exact_opened_file_identity_can_be_deleted() {
        let root = temp_dir();
        std::fs::create_dir_all(&root).expect("create test root");
        let path = root.join("candidate.tmp");
        std::fs::write(&path, b"AAAA").expect("write candidate");
        let (_guard, root_final) = open_stable_root(&root).expect("open root");
        let approved = evidence_from_handle(&path, &root, &root_final).expect("scan evidence");
        let deleted = delete_evidence(&approved).expect("delete exact file identity");
        assert_eq!(deleted, 4);
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(root);
    }
}
