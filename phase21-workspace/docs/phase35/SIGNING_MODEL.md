# Signing Model

Metadata uses the reviewed `ed25519-dalek` Ed25519 implementation already used by P29/P33. `SignatureEnvelope` records schema, `Ed25519`, key ID and signature bytes. `TrustedKeyring` records key ID, public key, enabled/revoked state and validity window. Unknown, malformed, disabled, revoked and mismatched keys reject.

Production private keys never enter the repository, archive, logs, CLI, IPC or UI. A production run requires an explicit external key/HSM reference. Current host has no configured production credential, therefore `PRODUCTION_RELEASE_SIGNING=NotAvailable` and `AUTHENTICODE_PRODUCTION_SIGNING=NotAvailable`; test seeds are deterministic fixtures only. Key rotation is a signed artifact authorized by an already trusted key and adds a key without silently replacing the trust root.
