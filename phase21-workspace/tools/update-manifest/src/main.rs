use aethercore_update_engine::{
    ManifestSignature, manifest_signature_from_hex, sign_manifest_bytes,
};
use anyhow::{Context, Result, bail};
use std::{fs, path::PathBuf};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 7 {
        bail!(
            "usage: aethercore-update-manifest-tool --manifest <path> --private-key <32-byte-hex-file> --out <path>"
        )
    };
    let manifest = PathBuf::from(arg(&args, "--manifest")?);
    let key_path = PathBuf::from(arg(&args, "--private-key")?);
    let out = PathBuf::from(arg(&args, "--out")?);
    let bytes = fs::read(&manifest).context("read manifest")?;
    let key_text = fs::read_to_string(&key_path).context("read private key")?;
    let decoded = hex::decode(key_text.trim()).context("private key hex")?;
    let key: [u8; 32] = decoded
        .try_into()
        .map_err(|_| anyhow::anyhow!("private key must be exactly 32 bytes"))?;
    let signature =
        sign_manifest_bytes(&bytes, &key).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let key_id = std::env::var("AETHERCORE_UPDATE_KEY_ID")
        .context("AETHERCORE_UPDATE_KEY_ID is required")?;
    let envelope: ManifestSignature = manifest_signature_from_hex(key_id, &signature);
    fs::write(out, serde_json::to_vec_pretty(&envelope)?).context("write signature")?;
    Ok(())
}
fn arg(args: &[String], name: &str) -> Result<String> {
    let index = args
        .iter()
        .position(|v| v == name)
        .with_context(|| format!("missing {name}"))?;
    args.get(index + 1)
        .cloned()
        .with_context(|| format!("missing value for {name}"))
}
