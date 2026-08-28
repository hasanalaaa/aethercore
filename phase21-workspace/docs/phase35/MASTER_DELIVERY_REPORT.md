# Phase 35 Master Delivery Report

This report is sealed with pointer-only archive wording to avoid self-reference churn.

Authoritative archive hash: see PHASE35_FINAL_SHA256.txt

## Review and baseline

- P34 authoritative archive SHA: `9c6aabb431fcb117afa19bb519db58dddb338dd63d31c2644db2490b084e419b`; P34 full-tree verifier: 1066/1066; inherited audit: 930 checks, 0 failures.
- Hermes handoff review: PASS. The only handwritten Rust delta was a behavior-preserving `collapsible_if` cleanup in `crates/release-authority/src/lib.rs`; locked decisions changed: NONE.

## Authority and security

- Release identity: `AetherCore` `0.1.0`, stable, sequence 35, Windows x86-64, protocol p35; Cargo workspace version is the source value and Tauri/WiX/manifest/CLI projections are verified.
- Signing: Ed25519 via the existing `ed25519-dalek`; strict canonical manifest/signature bytes; unknown, revoked, disabled, malformed and tampered keys reject; rotation requires an already trusted old key. Test key `test-p35` is isolated to a TEST-only fixture binary.
- Production signing: `PRODUCTION_RELEASE_SIGNING=NotAvailable`; `AUTHENTICODE_PRODUCTION_SIGNING=NotAvailable`; no private signing key is present.
- Channels: typed stable/beta/dev authority with stable isolation; existing online desktop wire remains stable/beta, while dev publication is reserved for explicit signed metadata.
- Anti-downgrade: strict validated version/sequence policy blocks older/equal ordinary updates; no generic force bypass.
- Rollback: distinct signed `aethercore.release.rollback-authorization.v1`, bound to installed current version, target, channel, reason, expiry and signer.
- Verification order: metadata signature/keyring → identity/channel/platform/architecture → package length/hash → release manifest/signature → SBOM/provenance binding → immutable apply plan → broker/mutation lease.
- Update metadata: strict `aethercore.update.metadata.v1`, signed and bound to product/channel/platform/architecture, HTTPS source, bounded size/digest, manifest digest and freshness.
- Staging and package safety: per-transaction bounded streaming staging, fsync/rename, hash and Authenticode checks before eligibility; archive traversal/absolute/UNC/drive/symlink/collision/bounds rules are enforced.
- Transaction/recovery: typed state machine rejects illegal transitions; durable update execution guard, single mutation lease and immutable plan prevent concurrent or substituted apply.

## Installer and distribution

- Installer architecture: WiX v4 MSI/Burn with stable UpgradeCode/component policy, service registration and ProgramData preservation. Windows MSI/UAC/service/ACL/reboot/rollback runtime qualification is explicitly deferred to P36.
- Offline bundle: `release/phase35/AetherCore-0.1.0-windows-x86_64-offline.zip`, deterministic ZIP, verified offline; it contains unsigned-package/signature NotAvailable markers, signing state, identity, manifest, detached test signature, test keyring, SBOM and provenance.
- SBOM: CycloneDX JSON 1.5, 601 Cargo.lock-derived components, SHA-256 `d66f6980ed79badfbe38ff7293f27fc63bd8fa1e178f85b0e471b6b978218aa2`.
- Provenance: `aethercore.release.provenance.v1`, SHA-256 `0495f3cb6a66911c96de8691df690d96d502d5a018f2c6a747825405d10dab60`; it binds package, release identity and SBOM. The manifest carries the one-way provenance/SBOM bindings; no mutual-hash self-reference is claimed.

## Product surfaces and evidence

- GD-1..GD-10: PASS.
- CLI: release inspect/verify and update check/plan/download/verify/stage/status/cancel/rollback plus offline verify; unsafe macOS apply paths return typed NotAvailable.
- Desktop Update UX: typed backend surface with stable/beta states and explicit progress/verification/rollback/error vocabulary.
- EN/AR: update state catalog keys are parity-checked; Arabic RTL copy is present for every Phase 35 state.
- CI/release: protected workflow runs Phase 35 audit/tests/SBOM/provenance and uses CI-only signing references; no auto-publish or secret logging.
- Adversarial audit: inherited 930 + 68 new = 998 checks, failures `[]`, status PASS.

## Quality gates

- Cargo run 1: `cargo test --workspace --jobs 2`; 124 result lines (51 test suites + 41 doc suites), 557 passed, 0 failed, 0 ignored, exit 0.
- Cargo run 2: identical counts and exit 0; normalized per-suite results equivalent.
- Clippy: `cargo clippy --workspace --all-targets --jobs 2`, exit 0; P35-authored warnings 0.
- Svelte: direct `svelte-check` exit 0, 0 errors and 17 inherited warnings in 3 pre-existing files; `npm run check` exits 1 only because its `--fail-on-warnings` wrapper treats those inherited warnings as fatal; P35-authored warnings 0.

## Delta and reconstruction

- P34→P35 delta: modified 10, added 43, removed 0, binary artifacts 7, manifest entries 53; scoped full-tree ledger 1114.
- Reconstruction cycle 1: apply/patch/full-tree PASS, comparison 1114, mismatch 0.
- Reconstruction cycle 2: apply/patch/full-tree PASS, comparison 1114, mismatch 0.
- Count invariant: raw 1116 = scoped 1114 + self-excluded 2 (`.DS_Store`, `crates/.DS_Store`); ledger 1114 = comparison 1114.

## Seal inventory and boundaries

- Release artifacts: identity, manifest, detached signature, test keyring, update metadata, installer composition, production-signing state, unsigned-package/signature markers, offline bundle, CycloneDX SBOM and provenance.
- Deterministic master archive is built independently twice; both run hashes and equality are reported outside this sealed tree, and the authoritative pointer remains `PHASE35_FINAL_SHA256.txt`.
- Debt diff: four append-only root entries QD-035-001..004 for native Windows lifecycle/security, Authenticode/SmartScreen, live endpoint, and production key/HSM qualification (the pre-existing HSM entry remains untouched).
- Remaining P36 qualification: native Windows MSI/Burn lifecycle, UAC/service SID/ACL/named pipe/reboot/rollback, production Authenticode and SmartScreen, live update endpoint/CDN/interruption, and protected HSM/KMS signing ceremony.
- Deviations: production MSI/signing/live endpoint are NotAvailable by design on macOS; no production-signed artifact is claimed. No material blockers remain for the deterministic Phase 35 distribution authority seal.
