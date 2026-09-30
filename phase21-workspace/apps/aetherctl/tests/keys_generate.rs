//! `aetherctl keys generate` against the real binary: an existing seed file is never
//! overwritten, and the reported permissions are the ones the file really has.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// A fresh directory under the system temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("aetherctl-keygen-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn keys_generate(out: &Path) -> (Option<i32>, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_aetherctl"))
        .args(["--output", "json", "keys", "generate", "--out"])
        .arg(out)
        .output()
        .expect("spawn aetherctl");
    let envelope = serde_json::from_slice(&output.stdout).expect("one JSON envelope on stdout");
    (output.status.code(), envelope)
}

#[test]
fn refuses_to_overwrite_an_existing_seed_file() {
    let dir = TempDir::new("exists");
    let out = dir.0.join("owner.key");
    let old = format!("{}\n", "ab".repeat(32));
    std::fs::write(&out, &old).unwrap();

    let (status, envelope) = keys_generate(&out);

    assert_eq!(status, Some(8), "{envelope}");
    assert_eq!(envelope["ok"], false, "{envelope}");
    assert_eq!(envelope["error"]["message_key"], "local.keys.exists");
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        old,
        "the existing seed must survive byte for byte"
    );
}

#[test]
#[cfg(unix)]
fn seed_file_is_owner_only_and_the_report_matches_it() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = TempDir::new("mode");
    let out = dir.0.join("owner.key");

    let (status, envelope) = keys_generate(&out);

    assert_eq!(status, Some(0), "{envelope}");
    let mode = std::fs::metadata(&out).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "seed file mode is {mode:o}");
    assert_eq!(envelope["data"]["permissions"], "0600");
}

#[test]
#[cfg(windows)]
fn windows_key_is_protected_even_under_an_everyone_parent() {
    let dir = TempDir::new("acl");
    let grant = Command::new("icacls")
        .arg(&dir.0)
        .args(["/grant", "*S-1-1-0:(OI)(CI)F"])
        .output()
        .unwrap();
    assert!(
        grant.status.success(),
        "{}",
        String::from_utf8_lossy(&grant.stderr)
    );
    let out = dir.0.join("owner.key");

    let (status, envelope) = keys_generate(&out);

    assert_eq!(status, Some(0), "{envelope}");
    assert_eq!(
        envelope["data"]["permissions"], "owner-only-protected",
        "{envelope}"
    );
    let acl = Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command",
        "$ErrorActionPreference='Stop'; $a=[IO.File]::GetAccessControl($env:AC_KEY_TEST_PATH); $sid=[Security.Principal.WindowsIdentity]::GetCurrent().User; $r=@($a.Access); [pscustomobject]@{protected=$a.AreAccessRulesProtected; owner=($a.GetOwner([Security.Principal.SecurityIdentifier]).Value -eq $sid.Value); soleOwner=($r.Count -eq 1 -and $r[0].IdentityReference.Translate([Security.Principal.SecurityIdentifier]).Value -eq $sid.Value -and -not $r[0].IsInherited -and $r[0].AccessControlType -eq 'Allow' -and [int]$r[0].FileSystemRights -eq 2032127)} | ConvertTo-Json -Compress"])
        .env("AC_KEY_TEST_PATH", &out).output().unwrap();
    assert!(
        acl.status.success(),
        "{}",
        String::from_utf8_lossy(&acl.stderr)
    );
    let acl: Value = serde_json::from_slice(&acl.stdout).unwrap();
    assert_eq!(acl["protected"], true, "{acl}");
    assert_eq!(acl["owner"], true, "{acl}");
    assert_eq!(acl["soleOwner"], true, "{acl}");
}

#[test]
#[cfg(windows)]
fn existing_junction_target_is_refused_without_touching_its_destination() {
    let dir = TempDir::new("junction");
    let destination = dir.0.join("destination");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("sentinel"), b"unchanged").unwrap();
    let out = dir.0.join("owner.key");
    let link = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&out)
        .arg(&destination)
        .output()
        .unwrap();
    assert!(link.status.success());
    let (status, envelope) = keys_generate(&out);
    assert_eq!(status, Some(8), "{envelope}");
    assert_eq!(
        std::fs::read(destination.join("sentinel")).unwrap(),
        b"unchanged"
    );
    assert_eq!(std::fs::read_dir(&destination).unwrap().count(), 1);
    let nested = out.join("nested.key");
    let (status, envelope) = keys_generate(&nested);
    assert_eq!(status, Some(8), "{envelope}");
    assert!(
        !destination.join("nested.key").exists(),
        "a reparse parent was followed"
    );
    std::fs::remove_dir(&out).unwrap();
}
