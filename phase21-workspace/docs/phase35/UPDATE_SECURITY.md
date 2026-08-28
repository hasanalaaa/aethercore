# Update Security

Metadata is `aethercore.update.metadata.v1`, strict and detached-signed. It binds product, current/target identity, channel, platform, architecture, HTTPS package reference, bounded length/digest, release-manifest digest, updater contract, key ID and deterministic freshness fields. Stable clients do not consume beta/dev metadata silently.

The client authenticates exact bytes before parsing, checks key status and identity, rejects stale/equivocating sequence floors and blocks ordinary semver downgrade. Package staging is unique per transaction, size bounded, streamed and hashed, fsynced, then renamed only after manifest/SBOM/provenance and Authenticode checks. No network endpoint is fabricated; local/offline fixtures are hermetic and HTTPS is the only production scheme.
