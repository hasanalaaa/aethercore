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
fn windows_reports_the_inherited_acl_not_a_unix_mode() {
    let dir = TempDir::new("acl");
    let out = dir.0.join("owner.key");

    let (status, envelope) = keys_generate(&out);

    assert_eq!(status, Some(0), "{envelope}");
    assert_eq!(envelope["data"]["permissions"], "inherited", "{envelope}");
}
