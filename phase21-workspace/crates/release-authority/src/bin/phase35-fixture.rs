//! Test-evidence helper only. It uses the deterministic TEST-ONLY seed `[7; 32]`;
//! production signing is never implicit and this binary is not a release signer.
use aethercore_release_authority::{
    KEYRING_SCHEMA, ReleaseManifest, SignatureEnvelope, TrustedKeyring, TrustedReleaseKey,
    canonical_json, sign_bytes,
};
use ed25519_dalek::SigningKey;
use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        return Err("usage: phase35-fixture <manifest.json>".into());
    }
    let manifest_path = PathBuf::from(&args[1]);
    let manifest: ReleaseManifest = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    let bytes = canonical_json(&manifest)?;
    let seed = [7u8; 32];
    let signing = SigningKey::from_bytes(&seed);
    let envelope: SignatureEnvelope = sign_bytes(&bytes, "test-p35", &seed)?;
    let keyring = TrustedKeyring {
        schema: KEYRING_SCHEMA.into(),
        keys: vec![TrustedReleaseKey {
            key_id: "test-p35".into(),
            algorithm: "Ed25519".into(),
            public_key_hex: hex::encode(signing.verifying_key().to_bytes()),
            enabled: true,
            revoked: false,
            not_before_epoch: None,
            not_after_epoch: None,
        }],
    };
    let out = manifest_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    fs::write(
        out.join("release-manifest.signature.json"),
        serde_json::to_vec(&envelope)?,
    )?;
    fs::write(out.join("test-keyring.json"), serde_json::to_vec(&keyring)?)?;
    println!("TEST_FIXTURE_SIGNATURE=PASS");
    Ok(())
}
