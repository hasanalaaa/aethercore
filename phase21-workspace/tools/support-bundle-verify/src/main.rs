use anyhow::{Context, Result, bail};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let archive=args.next().context("usage: aethercore-support-bundle-verify <bundle.aetherdiag> <expected-public-key-fingerprint-sha256>")?;
    let fingerprint = args
        .next()
        .context("expected installation public-key fingerprint is required")?;
    if args.next().is_some() {
        bail!("unexpected extra arguments")
    }
    let bytes = std::fs::read(&archive).with_context(|| format!("read {archive}"))?;
    aethercore_support_bundle::verify_archive(&bytes, &fingerprint)
        .context("support bundle verification failed")?;
    println!(
        "AetherCore support bundle verified against the expected installation key fingerprint."
    );
    Ok(())
}
