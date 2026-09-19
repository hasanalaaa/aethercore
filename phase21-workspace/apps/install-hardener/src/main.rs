#[cfg(windows)]
use std::{
    ffi::OsStr,
    os::windows::fs::MetadataExt,
    path::{Component, Path, PathBuf, Prefix},
    process::{Command, Output},
};

// DBT-P46-D1: the third independent declaration of the service name, and the
// second full re-typing of its account, both now derived from one decider.
use aethercore_product_identity::{PRODUCT_NAME, SERVICE_NAME, service_principal};
const SERVICE_SDDL: &str = "D:(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWLOCRRC;;;AU)";
const MACHINE_MUTATION_LOCK_RELATIVE_PATH: &str = r"state\machine-mutation.lock";
#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

#[cfg(not(windows))]
fn main() {
    eprintln!("aethercore-install-hardener is Windows-only");
    std::process::exit(2);
}

#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    let mut args = std::env::args_os();
    let _exe = args.next();
    match (args.next(), args.next()) {
        (Some(mode), None) if mode == OsStr::new("apply") => apply(),
        (Some(mode), None) if mode == OsStr::new("purge-data") => purge_data(),
        _ => anyhow::bail!("usage: aethercore-install-hardener.exe apply|purge-data"),
    }
}

#[cfg(windows)]
/// Deletes `%ProgramData%\AetherCore` in full, on uninstall, AFTER the service is gone.
///
/// WHY THIS IS NOT `util:RemoveFolderEx`. That was the first implementation and it
/// failed a gate. RemoveFolderEx enumerates the tree and injects RemoveFile rows
/// before CostInitialize, and Windows Installer then hard-fails at InstallValidate
/// with error 2318 if any file it snapshotted has since disappeared. The maintenance
/// service is still RUNNING at that point in an uninstall — StopServices does not run
/// until the execute sequence — and it is still writing `logs\service.jsonl`. So the
/// snapshot is taken against a live writer, and the uninstall becomes a race it can
/// lose. Observed: exit 1603, `Error 2318: File does not exist:
/// C:\ProgramData\AetherCore\logs\service.jsonl`, with the whole product left behind.
///
/// Deleting after DeleteServices is the only ordering that cannot race. It is
/// idempotent by construction: an absent directory is success, not an error.
///
/// Like `apply`, this takes NO path from its command line. The directory is derived
/// from %ProgramData% through the same trusted-path helpers, and a reparse point
/// anywhere in the tree is refused rather than followed — a low-privilege junction
/// planted under here must never turn an uninstall into a recursive delete somewhere
/// else.
fn purge_data() -> anyhow::Result<()> {
    let program_data = std::env::var_os("ProgramData")
        .ok_or_else(|| anyhow::anyhow!("ProgramData is not defined"))?;
    let data_dir = trusted_child(PathBuf::from(program_data), PRODUCT_NAME)?;
    if !data_dir.exists() {
        return Ok(());
    }
    reject_reparse_tree(&data_dir)?;
    // One retry: the service has just been deleted and a handle can still be closing.
    for attempt in 0..2 {
        match std::fs::remove_dir_all(&data_dir) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                if attempt == 1 {
                    return Err(anyhow::anyhow!(
                        "could not remove {}: {error}",
                        data_dir.display()
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(1500));
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
fn apply() -> anyhow::Result<()> {
    let system_root = trusted_absolute_env("SystemRoot")?;
    let system32 = system_root.join("System32");
    let sc = system32.join("sc.exe");
    let icacls = system32.join("icacls.exe");
    ensure_fixed_tool(&sc, "sc.exe")?;
    ensure_fixed_tool(&icacls, "icacls.exe")?;

    let program_files = std::env::var_os("ProgramW6432")
        .or_else(|| std::env::var_os("ProgramFiles"))
        .ok_or_else(|| anyhow::anyhow!("ProgramW6432/ProgramFiles is not defined"))?;
    let program_data = std::env::var_os("ProgramData")
        .ok_or_else(|| anyhow::anyhow!("ProgramData is not defined"))?;
    let bin_dir = trusted_child(PathBuf::from(program_files), PRODUCT_NAME)?;
    let data_dir = trusted_child(PathBuf::from(program_data), PRODUCT_NAME)?;
    if !bin_dir.is_dir() || !data_dir.is_dir() {
        anyhow::bail!("installer-created AetherCore directories are missing");
    }
    // A low-privilege pre-creation/junction must never redirect recursive ACL operations outside
    // AetherCore. Reject every reparse point before mutation; icacls also receives /L so a
    // link discovered during a race is operated on as a link rather than followed to its target.
    reject_reparse_tree(&bin_dir)?;
    reject_reparse_tree(&data_dir)?;
    let mutation_lock = ensure_mutation_lock_file(&data_dir)?;

    let principal = service_principal();

    // These are fixed, non-user-controlled SCM operations. The helper is intentionally not a
    // general command runner and accepts no paths or service names on its command line.
    run_checked(&sc, ["sidtype", SERVICE_NAME, "unrestricted"])?;
    run_checked(
        &sc,
        [
            "config",
            SERVICE_NAME,
            "start=",
            "delayed-auto",
            "obj=",
            "LocalSystem",
        ],
    )?;
    run_checked(&sc, ["sdset", SERVICE_NAME, SERVICE_SDDL])?;

    // Normalize each tree back to its parent ACL first, then replace inheritance with the fixed
    // product ACL. This deliberately removes stale explicit ACEs before the allowlist is applied,
    // so MSI repair closes permission drift instead of merely appending permissions.
    reset_acl_tree(&icacls, &bin_dir)?;
    reset_acl_tree(&icacls, &data_dir)?;
    run_icacls(
        &icacls,
        &bin_dir,
        &[
            "*S-1-5-18:(OI)(CI)F",      // LocalSystem
            "*S-1-5-32-544:(OI)(CI)F",  // Administrators
            "*S-1-5-32-545:(OI)(CI)RX", // Users: read/execute only
            &format!("{principal}:(OI)(CI)RX"),
        ],
    )?;
    run_icacls(
        &icacls,
        &data_dir,
        &[
            "*S-1-5-18:(OI)(CI)F",
            "*S-1-5-32-544:(OI)(CI)F",
            &format!("{principal}:(OI)(CI)F"),
        ],
    )?;
    // Give the authority file its own protected ACL so later parent drift cannot silently widen
    // who can participate in cross-process mutation arbitration.
    run_icacls(
        &icacls,
        &mutation_lock,
        &["*S-1-5-18:F", "*S-1-5-32-544:F", &format!("{principal}:F")],
    )?;

    Ok(())
}

#[cfg(windows)]
fn ensure_mutation_lock_file(data_dir: &Path) -> anyhow::Result<PathBuf> {
    let path = data_dir.join(MACHINE_MUTATION_LOCK_RELATIVE_PATH);
    validate_absolute_no_parent(&path)?;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("mutation lock has no parent"))?;
    if !parent.is_dir() {
        anyhow::bail!(
            "installer-created state directory is missing: {}",
            parent.display()
        );
    }
    // Existence-only: an already-present lock file keeps its contents.
    let _ = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)?;
    let metadata = std::fs::symlink_metadata(&path)?;
    if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        anyhow::bail!(
            "mutation authority path is not a regular file: {}",
            path.display()
        );
    }
    Ok(path)
}

#[cfg(windows)]
fn trusted_absolute_env(name: &str) -> anyhow::Result<PathBuf> {
    let path = PathBuf::from(
        std::env::var_os(name).ok_or_else(|| anyhow::anyhow!("{name} is not defined"))?,
    );
    validate_absolute_no_parent(&path)?;
    Ok(path)
}

#[cfg(windows)]
fn trusted_child(base: PathBuf, child: &str) -> anyhow::Result<PathBuf> {
    validate_absolute_no_parent(&base)?;
    let path = base.join(child);
    validate_absolute_no_parent(&path)?;
    Ok(path)
}

#[cfg(windows)]
fn validate_absolute_no_parent(path: &Path) -> anyhow::Result<()> {
    if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
        anyhow::bail!("untrusted installer path: {}", path.display());
    }
    match path.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(_) | Prefix::VerbatimDisk(_) => {}
            _ => anyhow::bail!(
                "installer path must be on a local drive: {}",
                path.display()
            ),
        },
        _ => anyhow::bail!(
            "installer path has no local drive prefix: {}",
            path.display()
        ),
    }
    Ok(())
}

#[cfg(windows)]
fn reject_reparse_tree(root: &Path) -> anyhow::Result<()> {
    fn visit(path: &Path) -> anyhow::Result<()> {
        let metadata = std::fs::symlink_metadata(path)?;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            anyhow::bail!(
                "reparse point refused in installer-owned tree: {}",
                path.display()
            );
        }
        if metadata.is_dir() {
            for entry in std::fs::read_dir(path)? {
                visit(&entry?.path())?;
            }
        }
        Ok(())
    }
    visit(root)
}

#[cfg(windows)]
fn ensure_fixed_tool(path: &Path, expected_name: &str) -> anyhow::Result<()> {
    validate_absolute_no_parent(path)?;
    if path.file_name() != Some(OsStr::new(expected_name)) || !path.is_file() {
        anyhow::bail!("required System32 tool missing: {}", path.display());
    }
    Ok(())
}

#[cfg(windows)]
fn run_checked<I, S>(exe: &Path, args: I) -> anyhow::Result<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new(exe).args(args).output()?;
    if !output.status.success() {
        anyhow::bail!(
            "{} failed ({}): {}",
            exe.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output)
}

#[cfg(windows)]
fn reset_acl_tree(exe: &Path, root: &Path) -> anyhow::Result<()> {
    let output = Command::new(exe)
        .arg(root)
        .args(["/reset", "/T", "/C", "/L", "/Q"])
        .output()?;
    if !output.status.success() {
        anyhow::bail!(
            "ACL reset failed for {} ({}): {}",
            root.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(windows)]
fn run_icacls(exe: &Path, root: &Path, grants: &[&str]) -> anyhow::Result<()> {
    // P36 Tranche 1 defect fix (Hermes): /T must NOT be combined with /inheritance:r +
    // (OI)(CI) grants. With /T, icacls applies the full argument set to every descendant:
    // on FILES, /inheritance:r leaves a protected empty DACL (D:PAI) and the (OI)(CI)
    // grant silently no-ops (inheritance flags are invalid on files), leaving critical
    // executables unreadable even to SYSTEM — the service then fails to start
    // (SCM access denied, MSI Error 1920). Without /T, the directory-level (OI)(CI)
    // ACEs propagate to every inheritable child (the preceding reset_acl_tree pass has
    // already restored all children to pure inheritance), which is the declared policy.
    let mut command = Command::new(exe);
    command.arg(root).arg("/inheritance:r").arg("/grant:r");
    for grant in grants {
        command.arg(grant);
    }
    command.args(["/C", "/L", "/Q"]);
    let output = command.output()?;
    if !output.status.success() {
        anyhow::bail!(
            "ACL hardening failed for {} ({}): {}",
            root.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}
