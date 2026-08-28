# Phase 35 Architecture

Phase 35 adds a metadata authority above the existing `aethercore-update-engine` and mutation supervisor. `crates/release-authority` is the single typed contract for release identity, strict deterministic manifests, Ed25519 envelopes, trusted keyring/rotation, signed update metadata, rollback authorization, transaction states and immutable apply plans. It does not download or elevate processes.

The existing update engine remains the apply boundary: it owns bounded staging, streaming SHA-256, Authenticode verification on Windows, durable execution guards and the `MutationSupervisor::Update` lease. The desktop and `aetherctl` surfaces request typed operations and cannot construct arbitrary installer commands.

Verification order is fixed: keyring/signature → identity/channel/platform/architecture → package length/digest → manifest/signature → SBOM/provenance bindings → immutable plan → broker/mutation lease. A macOS build proves metadata/configuration and honest NotAvailable states; Windows runtime qualification is Phase 36.
