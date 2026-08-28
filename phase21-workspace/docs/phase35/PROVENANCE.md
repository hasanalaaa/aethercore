# Provenance

`aethercore.release.provenance.v1` records product/version, Phase 34 authoritative base, source/toolchain identities, build commands, package/release-identity/SBOM hashes, deterministic unsigned-evidence result, host platform and signing/Windows qualification states. The release manifest carries the one-way `provenanceSha256` and `sbomSha256` bindings; provenance does not hash the manifest itself, avoiding an impossible mutual-hash cycle. It does not claim SLSA compliance. Build provenance is distinct from Windows runtime qualification.
