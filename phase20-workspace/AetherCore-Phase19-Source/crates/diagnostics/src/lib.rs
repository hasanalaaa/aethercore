#![forbid(unsafe_code)]

use std::{
    fs::{OpenOptions, create_dir_all},
    path::Path,
    sync::Mutex,
};

use anyhow::{Context, Result};
use tracing_subscriber::EnvFilter;

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

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
        let root = std::env::temp_dir().join(format!(
            "aethercore-log-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("service.jsonl");
        std::fs::write(&path, b"123456").unwrap();

        rotate_if_needed(&path, 6).unwrap();
        assert!(!path.exists());
        assert!(path.with_extension("previous.jsonl").exists());

        let _ = std::fs::remove_dir_all(root);
    }
}
