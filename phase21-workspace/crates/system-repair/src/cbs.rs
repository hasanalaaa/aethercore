//! P85-01: what the CBS log says about ONE `sfc` run.
//!
//! The log is shared by every servicing operation and keeps old runs, so a fixed tail cannot say
//! which run wrote what: an old corrupt run condemned a fresh clean one, and "some `[SR]` lines and no
//! error words" passed as clean. The run's own window is the bytes appended after a baseline taken
//! just before it started. A clean verdict needs that window to hold the run's completion marker and
//! the process to have exited 0. Whatever cannot be attributed to the run - a rotated, truncated or
//! missing log, a window past the bound, progress lines without a completion - is `Unknown`.
//!
//! CBS markers are log text, not a documented verdict API, so the parser is conservative: it raises
//! no confidence it cannot cite.

use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

/// The most of one run's log that is read. A longer window cannot be attributed.
const MAX_WINDOW: u64 = 4 * 1024 * 1024;

/// The log as it was just before the run started.
#[derive(Clone, Debug)]
pub struct CbsBaseline {
    len: u64,
    identity: Option<(u64, [u8; 16])>,
}

impl CbsBaseline {
    /// `None` when there is no log yet: the whole file, once it appears, belongs to the run.
    pub fn capture(path: &Path) -> Option<Self> {
        let file = match fs::File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
            Err(_) => {
                return Some(Self {
                    len: 0,
                    identity: None,
                });
            }
        };
        let metadata = file.metadata().ok();
        Some(Self {
            len: metadata.as_ref().map_or(0, fs::Metadata::len),
            identity: metadata
                .filter(fs::Metadata::is_file)
                .and_then(|_| file_identity(&file)),
        })
    }
}

pub enum Window {
    /// What was appended to the log during the run.
    Text(String),
    /// The window cannot be attributed to the run.
    Unknown,
}

pub fn window(path: &Path, baseline: Option<&CbsBaseline>) -> Window {
    let Ok(mut file) = fs::File::open(path) else {
        return Window::Unknown;
    };
    let Ok(metadata) = file.metadata() else {
        return Window::Unknown;
    };
    if !metadata.is_file() {
        return Window::Unknown;
    }
    let start = match baseline {
        None => 0,
        Some(before) => {
            if before.identity.is_none()
                || before.identity != file_identity(&file)
                || metadata.len() < before.len
            {
                return Window::Unknown; // rotated or truncated: the start of the run is gone
            }
            before.len
        }
    };
    if metadata.len() - start > MAX_WINDOW {
        return Window::Unknown;
    }
    if file.seek(SeekFrom::Start(start)).is_err() {
        return Window::Unknown;
    }
    let mut bytes = Vec::new();
    if file.take(MAX_WINDOW + 1).read_to_end(&mut bytes).is_err() || bytes.len() as u64 > MAX_WINDOW
    {
        return Window::Unknown;
    }
    Window::Text(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(unix)]
fn file_identity(file: &fs::File) -> Option<(u64, [u8; 16])> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file.metadata().ok()?;
    let mut id = [0; 16];
    id[..8].copy_from_slice(&metadata.ino().to_le_bytes());
    Some((metadata.dev(), id))
}

#[cfg(windows)]
fn file_identity(file: &fs::File) -> Option<(u64, [u8; 16])> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{FILE_ID_INFO, FileIdInfo, GetFileInformationByHandleEx},
    };
    let mut id = FILE_ID_INFO::default();
    // SAFETY: the file owns the live handle; the output buffer has FILE_ID_INFO's exact size.
    unsafe {
        GetFileInformationByHandleEx(
            HANDLE(file.as_raw_handle()),
            FileIdInfo,
            &mut id as *mut FILE_ID_INFO as *mut _,
            std::mem::size_of::<FILE_ID_INFO>() as u32,
        )
    }
    .ok()?;
    Some((id.VolumeSerialNumber, id.FileId.Identifier))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CbsEvidence {
    NoViolation,
    ViolationRepaired,
    ViolationUnresolved,
    Unknown,
}

/// Reads one run's window. Violation evidence counts whatever the exit code; a clean verdict needs
/// the completion marker and exit code 0.
pub fn classify(window: &str, exit_code: i32) -> CbsEvidence {
    let lines: Vec<String> = window
        .lines()
        .filter(|line| line.contains("[SR]"))
        .map(str::to_ascii_lowercase)
        .collect();
    let mut active = false;
    let mut began = false;
    for line in &lines {
        if line.contains("beginning verify and repair transaction") {
            if active {
                return CbsEvidence::Unknown;
            }
            began = true;
            active = true;
        } else if line.contains("verify complete") || line.contains("repair complete") {
            active = false;
        }
    }
    if !began || active {
        return CbsEvidence::Unknown;
    }
    if lines
        .iter()
        .any(|line| line.contains("cannot repair") || line.contains("repair failed"))
    {
        return CbsEvidence::ViolationUnresolved;
    }
    // "Repairing 0 components" and "Repair complete" are what a run with nothing to repair writes.
    let repaired = lines.iter().any(|line| {
        line.contains("repaired file")
            || line.contains("repairing corrupted")
            || (line.contains("repairing") && !line.contains("repairing 0 component"))
    });
    if repaired {
        return CbsEvidence::ViolationRepaired;
    }
    if exit_code == 0 && lines.iter().any(|line| line.contains("verify complete")) {
        CbsEvidence::NoViolation
    } else {
        CbsEvidence::Unknown
    }
}
