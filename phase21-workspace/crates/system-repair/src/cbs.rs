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
    time::SystemTime,
};

/// The most of one run's log that is read. A longer window cannot be attributed.
const MAX_WINDOW: u64 = 4 * 1024 * 1024;

/// The log as it was just before the run started.
#[derive(Clone, Debug)]
pub struct CbsBaseline {
    len: u64,
    created: Option<SystemTime>,
}

impl CbsBaseline {
    /// `None` when there is no log yet: the whole file, once it appears, belongs to the run.
    pub fn capture(path: &Path) -> Option<Self> {
        let metadata = fs::metadata(path).ok()?;
        Some(Self {
            len: metadata.len(),
            created: metadata.created().ok(),
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
    let Ok(metadata) = fs::metadata(path) else {
        return Window::Unknown;
    };
    let start = match baseline {
        None => 0,
        Some(before) => {
            let replaced =
                matches!((before.created, metadata.created().ok()), (Some(a), Some(b)) if a != b);
            if replaced || metadata.len() < before.len {
                return Window::Unknown; // rotated or truncated: the start of the run is gone
            }
            before.len
        }
    };
    if metadata.len() - start > MAX_WINDOW {
        return Window::Unknown;
    }
    let Ok(mut file) = fs::File::open(path) else {
        return Window::Unknown;
    };
    if file.seek(SeekFrom::Start(start)).is_err() {
        return Window::Unknown;
    }
    let mut bytes = Vec::new();
    if file.take(MAX_WINDOW).read_to_end(&mut bytes).is_err() {
        return Window::Unknown;
    }
    Window::Text(String::from_utf8_lossy(&bytes).into_owned())
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
    if lines.is_empty() {
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
