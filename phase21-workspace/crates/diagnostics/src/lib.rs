#![forbid(unsafe_code)]

use std::{
    fs::{OpenOptions, create_dir_all},
    path::Path,
    sync::Mutex,
};

use anyhow::{Context, Result};
use tracing_subscriber::EnvFilter;

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Phase 29 (T2): typed structured-event registry.
// Every structured log call site references a name from this table; the audit
// (p29-log-registry-*) greps for these constants and validates emitted JSON-lines
// against the stable schema {"ts","level","event","fields"}.
// ---------------------------------------------------------------------------

/// Stable structured event names (never renamed; additions only).
pub mod events {
    pub const STARTUP_MATRIX_LINE: &str = "startup.matrix_line";
    pub const DAEMON_READY: &str = "daemon.ready";
    pub const DAEMON_STOPPING: &str = "daemon.stopping";
    pub const CARE_STEP_STARTED: &str = "care.step_started";
    pub const CARE_STEP_FINISHED: &str = "care.step_finished";
    pub const PERF_SAMPLE_TICK: &str = "perf.sample_tick";
    pub const IPC_SESSION_OPENED: &str = "ipc.session_opened";
    pub const EXPORT_PRODUCED: &str = "export.produced";
    pub const LOG_ROTATED: &str = "log.rotated";
}

/// Emits one structured JSON-LINES record with the stable schema
/// `{"ts","level","event","fields"}` to stdout (the daemon redirects/pipes this into
/// its rotating log in json mode). Returns serde errors as strings — never panics.
pub fn emit_structured(level: &str, event: &str, fields: serde_json::Value) {
    let record = serde_json::json!({
        "ts": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        "level": level,
        "event": event,
        "fields": fields,
    });
    println!("{record}");
}

pub fn init_console() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(filter()?)
        .with_target(true)
        .try_init()
        .map_err(|e| anyhow::anyhow!("initialize console diagnostics: {e}"))
}

pub fn init_json_file(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        create_dir_all(parent)
            .with_context(|| format!("create diagnostics directory {}", parent.display()))?;
    }
    rotate_if_needed(path, MAX_LOG_BYTES)?;

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("open diagnostics log {}", path.display()))?;

    tracing_subscriber::fmt()
        .json()
        .with_ansi(false)
        .with_target(true)
        .with_env_filter(filter()?)
        .with_writer(Mutex::new(file))
        .try_init()
        .map_err(|e| anyhow::anyhow!("initialize file diagnostics: {e}"))
}

// ---------------------------------------------------------------------------
// Phase 28 (QD-026-003): size-capped rotating daemon logs.
// Deterministic names: <path>, <path>.1 .. <path>.{keep-1}; oldest generation is
// removed on roll; the cap is enforced at runtime while the daemon writes.
// ---------------------------------------------------------------------------

pub const DAEMON_LOG_MAX_BYTES: u64 = 5 * 1024 * 1024;
pub const DAEMON_LOG_KEEP_FILES: usize = 3;

/// Rotating variant of [`init_json_file`] used by `--daemon` mode.
pub fn init_json_file_rotated(path: &Path) -> Result<()> {
    init_json_file_rotated_with(path, DAEMON_LOG_MAX_BYTES, DAEMON_LOG_KEEP_FILES)
}

pub fn init_json_file_rotated_with(path: &Path, max_bytes: u64, keep: usize) -> Result<()> {
    debug_assert!(keep >= 1);
    if let Some(parent) = path.parent() {
        create_dir_all(parent)
            .with_context(|| format!("create diagnostics directory {}", parent.display()))?;
    }
    rotate_files(path, keep);

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("open diagnostics log {}", path.display()))?;

    let writer = RotatingLog {
        inner: Mutex::new(RotatingState {
            path: path.to_path_buf(),
            file,
            size: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
            max_bytes,
            keep,
        }),
    };
    tracing_subscriber::fmt()
        .json()
        .with_ansi(false)
        .with_target(true)
        .with_env_filter(filter()?)
        .with_writer(writer)
        .try_init()
        .map_err(|e| anyhow::anyhow!("initialize rotated file diagnostics: {e}"))
}

fn generation_path(path: &Path, index: usize) -> std::path::PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(format!(".{index}"));
    std::path::PathBuf::from(name)
}

/// Rolls deterministically, keep-last-N over ROTATED generations (slots .1 .. .{keep-1}):
/// drops the oldest, shifts each generation up one slot, then moves the current file into
/// slot .1 so the caller can recreate it fresh. Used at startup AND at every runtime cap.
fn rotate_files(path: &Path, keep: usize) {
    if keep <= 1 {
        // Only the live file is kept; nothing to shift.
        return;
    }
    let oldest = generation_path(path, keep - 1);
    if oldest.exists() {
        let _ = std::fs::remove_file(&oldest);
    }
    for index in (1..keep - 1).rev() {
        let from = generation_path(path, index);
        if from.exists() {
            let _ = std::fs::rename(&from, generation_path(path, index + 1));
        }
    }
    if path.exists() {
        let _ = std::fs::rename(path, generation_path(path, 1));
    }
}

struct RotatingLog {
    inner: Mutex<RotatingState>,
}

struct RotatingState {
    path: std::path::PathBuf,
    file: std::fs::File,
    size: u64,
    max_bytes: u64,
    keep: usize,
}

impl std::io::Write for RotatingOwnedGuard<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let state = &mut *self.0;
        if state.size.saturating_add(buf.len() as u64) > state.max_bytes {
            rotate_files(&state.path, state.keep);
            state.file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&state.path)?;
            state.size = 0;
        }
        let written = std::io::Write::write(&mut state.file, buf)?;
        state.size += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        std::io::Write::flush(&mut (*self.0).file)
    }
}

/// Owns the locked state for one formatted write; rolls generations when the cap would
/// be exceeded.
struct RotatingOwnedGuard<'a>(std::sync::MutexGuard<'a, RotatingState>);

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for RotatingLog {
    type Writer = RotatingOwnedGuard<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        // A poisoned lock only means a previous writer panicked mid-line; recover the
        // guarded data and keep logging rather than losing the daemon's audit trail.
        let guard = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        RotatingOwnedGuard(guard)
    }
}

fn filter() -> Result<EnvFilter> {
    Ok(EnvFilter::from_default_env().add_directive("info".parse()?))
}

fn rotate_if_needed(path: &Path, max_bytes: u64) -> Result<()> {
    let Ok(metadata) = std::fs::metadata(path) else {
        return Ok(());
    };
    if metadata.len() < max_bytes {
        return Ok(());
    }

    let previous = path.with_extension("previous.jsonl");
    if previous.exists() {
        std::fs::remove_file(&previous)
            .with_context(|| format!("remove old diagnostics log {}", previous.display()))?;
    }
    std::fs::rename(path, &previous).with_context(|| {
        format!(
            "rotate diagnostics log {} to {}",
            path.display(),
            previous.display()
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotates_at_configured_limit() {
        let root = std::env::temp_dir().join(format!("aethercore-log-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("service.jsonl");
        std::fs::write(&path, b"123456").unwrap();

        rotate_if_needed(&path, 6).unwrap();
        assert!(!path.exists());
        assert!(path.with_extension("previous.jsonl").exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rotated_generations_are_deterministic_keep_last_n() {
        let root =
            std::env::temp_dir().join(format!("aethercore-log-rot-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("service.jsonl");
        std::fs::write(&path, b"current").unwrap();
        std::fs::write(root.join("service.jsonl.1"), b"gen1").unwrap();
        std::fs::write(root.join("service.jsonl.2"), b"gen2").unwrap();

        // keep=3 (live + two generations): oldest drops, everything shifts up one slot.
        rotate_files(&path, 3);
        assert_eq!(
            std::fs::read(root.join("service.jsonl.1")).unwrap(),
            b"current"
        );
        assert_eq!(
            std::fs::read(root.join("service.jsonl.2")).unwrap(),
            b"gen1"
        );
        assert!(!path.exists(), "live path freed for recreation");
        assert!(!root.join("service.jsonl.3").exists());

        // A subsequent roll with a fresh live file keeps the invariant.
        std::fs::write(&path, b"newcur").unwrap();
        rotate_files(&path, 3);
        assert_eq!(
            std::fs::read(root.join("service.jsonl.1")).unwrap(),
            b"newcur"
        );
        assert_eq!(
            std::fs::read(root.join("service.jsonl.2")).unwrap(),
            b"current"
        );
        assert!(!root.join("service.jsonl.3").exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rotating_writer_rolls_at_runtime_cap() {
        use std::io::Write as _;
        let root =
            std::env::temp_dir().join(format!("aethercore-log-writer-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("service.jsonl");
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap();
        let log = RotatingLog {
            inner: Mutex::new(RotatingState {
                path: path.clone(),
                file,
                size: 0,
                max_bytes: 16,
                keep: 3,
            }),
        };
        {
            let mut guard = RotatingOwnedGuard(log.inner.lock().unwrap());
            guard.write_all(b"aaaaaaaaaaaaaaaa").unwrap(); // fills exactly 16
            guard.write_all(b"bbbbbbbbbbbbbbbb").unwrap(); // forces a roll
            guard.flush().unwrap();
        }
        assert_eq!(std::fs::read(&path).unwrap(), b"bbbbbbbbbbbbbbbb");
        assert_eq!(
            std::fs::read(root.join("service.jsonl.1")).unwrap(),
            b"aaaaaaaaaaaaaaaa"
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
