//! Locate the ADK's DismApi import library so `#[link(name = "DismApi")]` in
//! `src/dism_api.rs` resolves without the caller having pre-seeded `LIB`.
//!
//! Before this script existed, linking depended on the invoking shell already
//! carrying the right `LIB` entry. `scripts/build-arm64-msi.cmd` set it,
//! `scripts/build-release.ps1` did not until 52e6228, and CI never did — which
//! is how P49's first x64 build died with `LNK1181: cannot open input file
//! 'DismApi.lib'`. The directory is spelled `amd64`, never `x64`; that mapping
//! is encoded in `adk_arch_dir` and exists in exactly one place now.

use std::{env, path::PathBuf, process::Command};

/// Path under a Windows Kits root at which the ADK places the DismApi SDK.
const DISM_SDK_SUBPATH: &str = r"Assessment and Deployment Kit\Deployment Tools\SDKs\DismApi\Lib";

/// Escape hatch for an ADK installed somewhere the probes below cannot see
/// (notably an `adksetup.exe /installpath` that records no registry location).
const OVERRIDE_ENV: &str = "AETHERCORE_DISMAPI_LIB_DIR";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed={OVERRIDE_ENV}");
    println!("cargo:rerun-if-env-changed=LIB");

    // The `#[link]` attribute this supports is behind `#![cfg(windows)]`.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let Some(arch_dir) = adk_arch_dir(&arch) else {
        panic!(
            "aethercore-system-repair: no ADK DismApi library directory is defined for target architecture '{arch}'"
        );
    };

    let mut probed = Vec::new();
    match find_dism_lib_dir(arch_dir, &mut probed) {
        Some(dir) => println!("cargo:rustc-link-search=native={}", dir.display()),
        None => panic!("{}", not_found_message(arch_dir, &probed)),
    }
}

/// Maps a Rust target architecture onto the ADK's own directory spelling.
/// The ADK uses `amd64` where Rust and MSVC use `x86_64`/`x64`.
fn adk_arch_dir(target_arch: &str) -> Option<&'static str> {
    match target_arch {
        "x86_64" => Some("amd64"),
        "aarch64" => Some("arm64"),
        "x86" => Some("x86"),
        _ => None,
    }
}

/// Returns the first candidate directory that actually contains the import
/// library, recording every directory tried in `probed` for the error message.
fn find_dism_lib_dir(arch_dir: &str, probed: &mut Vec<PathBuf>) -> Option<PathBuf> {
    for candidate in candidate_dirs(arch_dir) {
        if candidate.join("dismapi.lib").is_file() {
            return Some(candidate);
        }
        probed.push(candidate);
    }
    None
}

/// Candidate directories, most authoritative first.
fn candidate_dirs(arch_dir: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut push = |p: PathBuf| {
        if !out.contains(&p) {
            out.push(p);
        }
    };

    // 1. Explicit override wins over any probe.
    if let Some(dir) = env::var_os(OVERRIDE_ENV) {
        push(PathBuf::from(dir));
    }

    // 2. The registered Windows Kits root. This is the only probe that follows
    //    a non-default ADK install, and only when the installer registered it.
    for root in kits_roots() {
        push(root.join(DISM_SDK_SUBPATH).join(arch_dir));
    }

    // 3. Default install locations, for an ADK whose registry entry is missing.
    for var in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Some(pf) = env::var_os(var) {
            push(
                PathBuf::from(pf)
                    .join(r"Windows Kits\10")
                    .join(DISM_SDK_SUBPATH)
                    .join(arch_dir),
            );
        }
    }

    // 4. Anything the invoking shell already put on `LIB`, so environments that
    //    work today keep working even if every probe above comes up empty.
    if let Some(lib) = env::var_os("LIB") {
        for entry in env::split_paths(&lib) {
            if !entry.as_os_str().is_empty() {
                push(entry);
            }
        }
    }

    out
}

/// Reads `KitsRoot10` from both registry views. Shelling out to `reg.exe` keeps
/// this script free of build-dependencies, so the SBOM and `deny.toml` review
/// surface are unchanged by it.
fn kits_roots() -> Vec<PathBuf> {
    const KEYS: [&str; 2] = [
        r"HKLM\SOFTWARE\Microsoft\Windows Kits\Installed Roots",
        r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows Kits\Installed Roots",
    ];

    let mut roots = Vec::new();
    for key in KEYS {
        let Ok(output) = Command::new("reg")
            .args(["query", key, "/v", "KitsRoot10"])
            .output()
        else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            // `    KitsRoot10    REG_SZ    C:\Program Files (x86)\Windows Kits\10\`
            let Some((_, value)) = line.trim().split_once("REG_SZ") else {
                continue;
            };
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            let root = PathBuf::from(value);
            if !roots.contains(&root) {
                roots.push(root);
            }
        }
    }
    roots
}

fn not_found_message(arch_dir: &str, probed: &[PathBuf]) -> String {
    let mut msg = format!(
        "aethercore-system-repair: could not locate dismapi.lib for the '{arch_dir}' architecture.\n\
         This crate links the Windows ADK DismApi SDK, which ships with the ADK's\n\
         Deployment Tools feature and is not part of the Windows SDK.\n\n\
         Install it with:\n  \
         adksetup.exe /quiet /norestart /features OptionId.DeploymentTools\n\n\
         If the ADK is installed to a non-default location, point this crate at it:\n  \
         set {OVERRIDE_ENV}=<adk>\\{DISM_SDK_SUBPATH}\\{arch_dir}\n\n\
         Directories probed, in order:\n"
    );
    for dir in probed {
        msg.push_str(&format!("  {}\n", dir.display()));
    }
    msg
}
