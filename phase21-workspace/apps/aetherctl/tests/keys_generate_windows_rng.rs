//! On Windows `/dev/urandom` means `<current drive>:\dev\urandom`, a file any local
//! user can create. `keys generate` must never take a signing seed from it.
//!
//! Its own test binary on purpose: cargo runs test binaries one after another, so no
//! other `keys generate` test can observe the planted file.

#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// `<drive>\dev\urandom` filled with one repeated byte. Removes only what it created.
struct PlantedUrandom {
    dev: PathBuf,
    created_dev: bool,
}

impl PlantedUrandom {
    fn plant(drive: &Path, byte: u8) -> Self {
        let dev = drive.join("dev");
        let file = dev.join("urandom");
        assert!(
            !file.exists(),
            "{} already exists; refusing to replace it",
            file.display()
        );
        let created_dev = !dev.exists();
        std::fs::create_dir_all(&dev).unwrap();
        std::fs::write(&file, vec![byte; 4096]).unwrap();
        PlantedUrandom { dev, created_dev }
    }
}

impl Drop for PlantedUrandom {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.dev.join("urandom"));
        if self.created_dev {
            let _ = std::fs::remove_dir(&self.dev);
        }
    }
}

#[test]
fn seed_never_comes_from_a_planted_dev_urandom() {
    let work = std::env::temp_dir().join(format!("aetherctl-keygen-rng-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    // The child runs in `work`, so `/dev/urandom` resolves against this drive's root.
    let drive = work.ancestors().last().unwrap().to_path_buf();
    let _planted = PlantedUrandom::plant(&drive, 0x41);
    let out = work.join("owner.key");

    let output = Command::new(env!("CARGO_BIN_EXE_aetherctl"))
        .current_dir(&work)
        .args(["--output", "json", "keys", "generate", "--out"])
        .arg(&out)
        .output()
        .expect("spawn aetherctl");
    let seed = std::fs::read_to_string(&out);
    let _ = std::fs::remove_dir_all(&work);

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_ne!(
        seed.unwrap().trim(),
        "41".repeat(32),
        "the signing seed was read from the planted {}dev\\urandom",
        drive.display()
    );
}
