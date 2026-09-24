//! P75 lane cli-truth: the REAL aetherctl binary, offline commands only (no daemon,
//! no e2e feature), so this suite runs on every platform CI builds, Windows included.

use std::path::PathBuf;
use std::process::{Command, Output};

use aethercore_release_authority as ra;

fn aetherctl(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_aetherctl"))
        .args(args)
        .env_remove("AETHERCORE_LANG")
        .output()
        .expect("spawn aetherctl")
}

fn json(args: &[&str]) -> (i32, serde_json::Value) {
    let mut full = vec!["--output", "json"];
    full.extend_from_slice(args);
    let out = aetherctl(&full);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("not one JSON envelope ({e}): {stdout}"));
    (out.status.code().expect("exit code"), value)
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("axt-p75-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

fn path(p: &std::path::Path) -> &str {
    p.to_str().expect("utf-8 path")
}

// ---------------------------------------------------------------------------
// update verify: the installed version and the clock must be real
// ---------------------------------------------------------------------------

fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch")
        .as_secs()
}

fn this_platform() -> ra::TargetPlatform {
    if cfg!(windows) {
        ra::TargetPlatform::Windows
    } else if cfg!(target_os = "macos") {
        ra::TargetPlatform::Macos
    } else {
        ra::TargetPlatform::Linux
    }
}

fn this_architecture() -> ra::TargetArchitecture {
    if cfg!(target_arch = "aarch64") {
        ra::TargetArchitecture::Aarch64
    } else {
        ra::TargetArchitecture::X86_64
    }
}

/// Writes signed update metadata + keyring; returns the three CLI args' paths.
fn update_fixture(
    tag: &str,
    current_version: &str,
    target_version: &str,
    generated_epoch: u64,
    expires_epoch: u64,
) -> (PathBuf, PathBuf, PathBuf) {
    let dir = scratch(tag);
    let seed = [42u8; 32];
    let identity = ra::ReleaseIdentity {
        schema: ra::IDENTITY_SCHEMA.into(),
        product_id: "AetherCore".into(),
        version: target_version.into(),
        channel: ra::ReleaseChannel::Stable,
        release_sequence: 1,
        platform: this_platform(),
        architecture: this_architecture(),
        protocol_version: "p35".into(),
        minimum_updater_version: "0.1.0".into(),
        source_commit: None,
        build_epoch: None,
    };
    let metadata = ra::UpdateMetadata {
        schema: ra::UPDATE_METADATA_SCHEMA.into(),
        product_id: "AetherCore".into(),
        channel: ra::ReleaseChannel::Stable,
        current_version: current_version.into(),
        target_identity: identity,
        platform: this_platform(),
        architecture: this_architecture(),
        package_url: "https://updates.invalid/a.msi".into(),
        package_length: 4,
        package_sha256: "aa".repeat(32),
        release_manifest_sha256: "bb".repeat(32),
        signing_key_id: "p75".into(),
        update_contract_version: ra::UPDATE_CONTRACT_VERSION.into(),
        release_notes_reference: None,
        generated_epoch,
        expires_epoch,
    };
    let signature = ra::sign_bytes(&ra::canonical_json(&metadata).unwrap(), "p75", &seed).unwrap();
    let public = ed25519_dalek::SigningKey::from_bytes(&seed)
        .verifying_key()
        .to_bytes();
    let keyring = ra::TrustedKeyring {
        schema: ra::KEYRING_SCHEMA.into(),
        keys: vec![ra::TrustedReleaseKey {
            key_id: "p75".into(),
            algorithm: "Ed25519".into(),
            public_key_hex: public.iter().map(|b| format!("{b:02x}")).collect(),
            enabled: true,
            revoked: false,
            not_before_epoch: None,
            not_after_epoch: None,
        }],
    };
    let (m, s, k) = (
        dir.join("metadata.json"),
        dir.join("signature.json"),
        dir.join("keyring.json"),
    );
    std::fs::write(&m, serde_json::to_vec(&metadata).unwrap()).unwrap();
    std::fs::write(&s, serde_json::to_vec(&signature).unwrap()).unwrap();
    std::fs::write(&k, serde_json::to_vec(&keyring).unwrap()).unwrap();
    (m, s, k)
}

fn update_verify(files: &(PathBuf, PathBuf, PathBuf)) -> (i32, serde_json::Value) {
    json(&[
        "update",
        "verify",
        "--metadata",
        path(&files.0),
        "--signature",
        path(&files.1),
        "--keyring",
        path(&files.2),
    ])
}

#[test]
fn update_verify_checks_the_real_installed_version() {
    let now = now_epoch();
    // The metadata claims this machine runs 9.9.0; it runs CARGO_PKG_VERSION.
    let files = update_fixture("upd-version", "9.9.0", "9.9.1", now - 60, now + 3600);
    let (code, envelope) = update_verify(&files);
    assert_eq!(envelope["ok"], false, "{envelope}");
    assert_ne!(code, 0);
}

#[test]
fn update_verify_uses_the_real_clock() {
    // Signed long ago, expired long ago: the metadata's own generated_epoch is not "now".
    let installed = env!("CARGO_PKG_VERSION");
    let files = update_fixture("upd-clock", installed, "999.0.0", 1, 10);
    let (code, envelope) = update_verify(&files);
    assert_eq!(envelope["ok"], false, "{envelope}");
    assert_ne!(code, 0);
}

#[test]
fn update_verify_reports_what_it_checked_and_what_it_did_not() {
    let now = now_epoch();
    let installed = env!("CARGO_PKG_VERSION");
    let files = update_fixture("upd-ok", installed, "999.0.0", now - 60, now + 3600);
    let (code, envelope) = update_verify(&files);
    assert_eq!(code, 0, "{envelope}");
    let data = &envelope["data"];
    assert_eq!(data["status"], "verified");
    assert_eq!(data["installedVersion"], installed);
    assert!(data["checkedAtEpoch"].as_u64().unwrap() >= now);
    assert_eq!(data["notChecked"], serde_json::json!(["installedChannel"]));
}
