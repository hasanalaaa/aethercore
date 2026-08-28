# SBOM

The canonical format is CycloneDX JSON 1.5. `scripts/phase35-release.py sbom` derives Rust components from the authoritative `Cargo.lock`, sorts by name/version and writes deterministic JSON. Unknown license data is not guessed. The resulting SHA-256 is bound into the release manifest and provenance; generation is run twice during sealing. The manifest binds provenance one-way, while provenance binds the release identity and package without self-referential manifest hashing.
