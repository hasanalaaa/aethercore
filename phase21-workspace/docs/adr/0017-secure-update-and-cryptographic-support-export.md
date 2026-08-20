# ADR 0017 — Secure Update and Cryptographic Support Export

**Status:** Accepted — Phase 15

## Decision

AetherCore will preserve WiX MSI/Burn as the single installation authority while splitting update orchestration across a non-elevated **user-scope downloader**, LocalSystem verification/staging, and a fixed elevated update broker. The maintenance service will not contain the production HTTP downloader dependency and will not accept arbitrary URLs, executable paths, or installer arguments from the renderer.

Static update metadata is authenticated with a channel-pinned Ed25519 key before parsing. The signed artifact hash/size is the immutable content identity. Candidate selection rejects releases requiring a Windows build newer than the actual host build returned by `RtlGetVersion`. The service maintains a durable monotonic manifest floor, verifies uploaded bytes, performs hash→Authenticode→hash validation at staging and again at install claim/recovery, and acquires `MutationSupervisor::Update` only when the fixed elevated broker claims a one-shot install intent. The broker additionally holds `Global\\AetherCore.WindowsUpdateMutation.v1` while it claims/runs Burn, retaining the existing process-restart defense-in-depth boundary. The claim reservation is linearized under the intent lock before expensive verification/lease work; pre-execution failures release the reservation, staging failures cannot leave the snapshot stuck in `Staging`, while decline/expiry returns the owner snapshot to its staged/available state. The durable execution guard retains exact release identity and reconstructs `Installing` after a service restart; expiration is fail-closed and cannot release the Update lease unless durable-guard deletion succeeds.

Support bundles use an allowlist, privacy sanitization before preview, deterministic archives, per-file SHA-256 plus an ordered root hash, validated TAR header checksums, serialized installation-key creation, and an installation-local Ed25519 signature verified with strict Ed25519 semantics. Sanitization also removes SIDs/email identifiers embedded inside free-form text, and bounded retained-object quotas cap previews/prepared bundles with at most one active prepared bundle per owner. The archive's embedded public key is metadata, not a root of trust; strong verification requires the **independent fingerprint** received over authenticated IPC/export UI. The claim must remain installation-scoped and must never be described as vendor or hardware attestation. The non-elevated desktop must run the strong verifier against the independently received fingerprint before atomically renaming the temporary bundle into the user-selected Downloads filename and must discard service-side staging on success or failure.

## Consequences

- Compromise of renderer-controlled values cannot create privileged network or arbitrary execution authority.
- LocalSystem has no update HTTP client dependency; network retrieval occurs at user privilege.
- The signed manifest prevents package-content substitution; Authenticode supplies a second Windows trust check.
- Update mutation cannot collide with driver, repair, cleanup, or startup mutations; the logical Update lease is reinforced by the installer-provisioned protected Windows mutation lock across process/service restart boundaries.
- Support exports cannot silently include raw arbitrary logs/files, cannot overwrite an existing user export, and cannot retain unbounded previews/bundles in service memory or staging storage.
- A self-signed modified support bundle is insufficient for strong verification because the expected installation-key fingerprint is external to the archive.
- Production signing must explicitly provide enabled update trust; the repository template remains disabled by design.
