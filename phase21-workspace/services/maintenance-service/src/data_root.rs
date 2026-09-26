//! Where the service keeps its state: `--data-dir`, `$AETHERCORE_DATA_DIR`, or on Windows
//! `%ProgramData%\AetherCore`.

use std::path::PathBuf;

use anyhow::Result;

/// `--data-dir`, then `$AETHERCORE_DATA_DIR`, then (Windows only) `%ProgramData%\AetherCore`.
/// DBT-P60-003: a POSIX host has no such default, and falling back to the literal
/// `C:\ProgramData` made a relative directory under the working directory.
pub(crate) fn resolve(
    mut args: impl Iterator<Item = String>,
    data_dir_env: Option<String>,
    program_data: Option<std::ffi::OsString>,
) -> Result<PathBuf> {
    while let Some(arg) = args.next() {
        if arg == "--data-dir"
            && let Some(value) = args.next()
            && !value.trim().is_empty()
        {
            return Ok(PathBuf::from(value));
        }
    }
    if let Some(v) = data_dir_env.filter(|v| !v.trim().is_empty()) {
        return Ok(PathBuf::from(v));
    }
    if cfg!(windows) {
        return Ok(program_data
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
            .join(aethercore_product_identity::PRODUCT_NAME));
    }
    let _ = program_data;
    anyhow::bail!("no data directory: pass --data-dir <dir> or set AETHERCORE_DATA_DIR")
}

#[cfg(test)]
mod data_root_tests {
    use super::*;

    fn args(list: &[&str]) -> impl Iterator<Item = String> {
        list.iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// DBT-P60-003: with no directory named, a POSIX host got the relative path
    /// `C:\ProgramData/AetherCore` and wrote its database under the working directory.
    #[cfg(unix)]
    #[test]
    fn a_posix_host_never_gets_a_windows_path() {
        let root = resolve(args(&["svc", "--foreground"]), None, None);
        assert!(root.is_err(), "resolved to {root:?}");
        let root = resolve(args(&["svc"]), Some(" ".into()), Some("C:\\x".into()));
        assert!(root.is_err(), "resolved to {root:?}");
    }

    #[test]
    fn an_explicit_directory_wins() {
        let root = resolve(
            args(&["svc", "--data-dir", "/srv/ac"]),
            Some("/env".into()),
            None,
        );
        assert_eq!(root.unwrap(), PathBuf::from("/srv/ac"));
        let root = resolve(args(&["svc"]), Some("/env".into()), None);
        assert_eq!(root.unwrap(), PathBuf::from("/env"));
    }
}
