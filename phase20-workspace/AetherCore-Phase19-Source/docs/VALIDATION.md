# Validation Status — Through Phase 15

AetherCore separates platform-neutral authoring qualification from authoritative native Windows qualification. The Phase 15 authoring runtime used for this source seal is Linux: it can validate source structure, contracts, migrations, localization, deterministic archive logic and static trust invariants, but it cannot compile the Windows Rust target against the Windows SDK, execute SCM/UAC/WTS/WinVerifyTrust behavior, build WiX MSI/Burn, or perform live signed-installer lifecycle testing.

## Phase 15 qualification added to the inherited Phase 0–14 chain

Phase 15 adds fail-closed gates for:

- user-scope HTTPS update retrieval with redirects disabled and finite connection/body limits;
- Ed25519 signature verification over exact static-manifest bytes before JSON parsing;
- stable/beta channel binding, expiry/future-date checks and monotonic rollback/equivocation floors;
- service-owned sequential staging with no caller-supplied privileged filesystem path;
- hash → Authenticode → hash verification before staged trust, install claim and broker execution;
- one-shot install intents with atomic claim reservation before expensive verification/lease work;
- `MutationSupervisor::Update` plus the existing `Global\\AetherCore.WindowsUpdateMutation.v1` cross-process mutex;
- restart-safe durable execution metadata that restores the exact release identity as `Installing`;
- fail-closed execution expiry: the Update lease is not released if durable-guard deletion fails;
- fixed update-broker command surface with no arbitrary URL/path/command/installer arguments;
- preview-first support export from an allowlist of sanitized evidence only;
- embedded account/SID/email/path/serial redaction and bounded retained preview/bundle quotas;
- deterministic USTAR archives, per-file SHA-256, ordered payload-root hash and installation-local Ed25519 proof;
- strict proof verification against an independently received installation-key fingerprint;
- TAR checksum, manifest/path/size/hash/root/signature/unmanifested-entry tamper rejection;
- desktop re-verification before final rename and deterministic discard of service-side export staging on success or failure.

At the source-seal point, the platform-neutral gates report:

- aggregate Phase 0–15 static invariants: **313/313 PASS**;
- dedicated Phase 15 update/support security audit: **106/106 PASS**;
- inherited Phase 13 reliability audit: **61/61 PASS**;
- inherited Phase 14 scheduler audit: **63/63 PASS**;
- English/Arabic catalog parity: **1023/1023 PASS** with CLDR plural-runtime validation;
- strict TypeScript source audit: **33/33 PASS**;
- Svelte structural/embedded-script audit: **25/25 PASS**;
- CSS parse audit: **9/9 PASS**;
- Rust lexical/delimiter audit: **80/80 PASS**;
- Phase 15 SQLite fresh schema and Phase 14→15 migration validation: **PASS**.

These authoring checks do not replace a Windows `cargo check`, `pnpm check/build`, WinVerifyTrust/UAC execution, WiX compilation, Authenticode validation or disposable-VM installer lifecycle.

## Authoritative Windows Phase 15 gate

Run from a reviewed dependency-frozen Windows checkout:

```powershell
.\scripts\verify-phase15.ps1
```

The gate first runs the complete Phase 0–14 chain without release packaging. It then runs the Phase 15 security audit, locked Rust workspace compilation, cryptographic/update/support regression tests, Svelte type/build checks, localization parity and aggregate static validation. Release packaging is intentionally deferred until all Phase 15 trust boundaries are green.

For signed/release packaging, provision an reviewed enabled `update-trust.json` through `-UpdateTrustPath` or protected `AETHERCORE_UPDATE_TRUST_PATH` and run:

```powershell
.\scripts\verify-phase15.ps1 -ReleasePackaging -RequireSigning -UpdateTrustPath <path>
```

For final disposable-machine lifecycle qualification:

```powershell
.\scripts\verify-phase15.ps1 -InstallerLifecycle -RequireSigning -UpdateTrustPath <path>
```

The final Windows qualification matrix must include real HTTPS manifest/artifact endpoints, good and bad Ed25519 signatures, manifest rollback/equivocation, truncated/chunk-reordered uploads, Authenticode failure/revocation behavior, UAC decline, service restart during an active update, Burn success/3010/failure, concurrent maintenance mutation attempts, support-bundle privacy inspection, bundle tampering, independent-fingerprint mismatch, repair/uninstall/upgrade and ACL/service recovery.

## Release-freeze boundary

Phase 15 remains fail-closed on the dependency freeze inherited from Phase 9. A trusted Windows freeze workstation must create/review and commit the approved `Cargo.lock`, `pnpm-lock.yaml`, `release/dependency-locks.sha256`, `release/dependency-manifests.sha256` and `release/dependency-freeze.json`. CI and signed-release jobs verify that freeze; they do not mint it implicitly.

## Zenith additive gate

`python scripts/zenith-adversarial-audit.py` is the platform-neutral regression gate for post-Enterprise interaction/accessibility findings. On Windows, `scripts\zenith-adversarial-audit.ps1` adapts it to the PowerShell toolchain and `scripts\verify-zenith.ps1` composes it with the full Enterprise gate.

CI continues to run the historical Enterprise master gate and additionally runs the Zenith source audit. The signed-release workflow runs Zenith before the existing Enterprise signing/packaging step. `verify-production.ps1` also requires Zenith before GA sealing. A Zenith source PASS never substitutes for Rust/Svelte/Windows/runtime/signing/soak evidence.

## Sigma evidence-integrity contract

`python scripts/omega-evidence.py --output <external-path>` is read-only with respect to the delivered source tree. The command rejects evidence paths inside the repository, verifies `MANIFEST.sha256` before executing repository gates, runs those gates against disposable source clones, detects clone-side mutation attempts, fingerprints the delivered tree before/after the run, and verifies the manifest again before determining source-verification success. `source_manifest_before`, `source_manifest`, `source_tree_integrity`, and `execution_integrity` are all fail-closed inputs to `source_verification_pass`.

Audit JSON materialization is explicit. `static_validate.py`, Phase 13-16 Python audits, Enterprise adversarial audit, and Zenith adversarial/recursive audits do not rewrite repository evidence files unless an explicit `--output` path is supplied. `scripts/sigma-evidence-integrity-test.py` is the adversarial regression for the prior 427/427 to 426/427 manifest corruption and false-zero-exit defect.
