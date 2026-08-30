# Release & Software Supply-Chain Pipeline

## Release philosophy

AetherCore release artifacts are accepted only when the source graph, dependency graph, security evidence, signing identity, and packaging order are explicit. “Build succeeded” is not a release criterion.

## Dependency freeze state machine

### Seed state

A source snapshot may begin without lockfiles only when it has never been dependency-frozen. This is the state of the authoring package when outbound registry access was unavailable.

### Freeze

On a trusted connected Windows workstation:

```powershell
.\scripts\freeze-dependencies.ps1 -Refresh
```

The script resolves `Cargo.lock` and `pnpm-lock.yaml`, verifies Cargo with `--locked`, hashes those lockfiles into `release\dependency-locks.sha256`, hashes the dependency/toolchain manifests into `release\dependency-manifests.sha256`, and emits `release\dependency-freeze.json` tying the evidence to the pinned tool versions.

The two lockfiles and all three release evidence files must be reviewed and committed together: `Cargo.lock`, `pnpm-lock.yaml`, `release/dependency-locks.sha256`, `release/dependency-manifests.sha256`, and `release/dependency-freeze.json`.

### Verified state

```powershell
.\scripts\freeze-dependencies.ps1 -VerifyOnly
```

A release fails if any freeze artifact is absent, if only part of the freeze exists, if lock or manifest hashes differ from the approved baselines, if metadata baseline hashes do not match those baseline files, or if the pinned Rust/pnpm tool versions drift.

Both general CI and the protected release workflow are **verify-only**. Neither is allowed to mint or refresh an approved dependency graph. `-Refresh` is reserved for the trusted dependency-freeze workstation after explicit dependency review.

## Rust policy

`deny.toml` enforces:

- yanked crates denied;
- wildcard dependencies denied;
- unknown registries denied;
- unknown Git sources denied;
- Git sources must use immutable `rev` specification;
- explicit license allowlist with private workspace crates excluded from third-party license policy;
- duplicate versions surfaced as warnings for engineering review.

The release audit uses pinned `cargo-deny` 0.20.2 and saves its output.

## pnpm policy

The workspace uses exact direct dependency versions and a frozen lockfile in release/CI. Workspace policy also enables:

- minimum dependency release age;
- exotic subdependency blocking;
- strict dependency build-script approval;
- explicit build allowance only for esbuild;
- high advisory threshold.

`pnpm audit --audit-level high` fails on high/critical vulnerabilities. The release evidence also contains `pnpm licenses list --json`.

## SBOM

### Rust

Pinned `cargo-cyclonedx` 0.5.9 generates CycloneDX JSON adjacent to every Cargo workspace manifest. The release script then collects these files into the release evidence directory. `SOURCE_DATE_EPOCH` is mandatory, making SBOM timestamps deterministic and suppressing random serial generation supported by this version.

### UI

pnpm generates CycloneDX 1.7 directly from the committed lockfile using `--lockfile-only`.

Every generated JSON document is parsed before the release continues.

## Build determinism

Rust release profile:

- thin LTO;
- one codegen unit;
- incremental compilation disabled;
- panic abort;
- symbol stripping;
- MSVC `/Brepro`;
- MSVC `/INCREMENTAL:NO`.

`verify-reproducible.ps1 -NativeDoubleBuild` can build privileged native binaries twice into independent target directories and requires matching SHA-256 hashes.

### MSI boundary

AetherCore does not overclaim reproducibility. The WiX project currently has a known open issue in which MSI PackageCode and summary timestamps vary between builds. Therefore:

- deterministic source/dependency/product-code inputs are recorded;
- native binaries and SBOM controls can be checked independently;
- MSI is signed and hashed as a release artifact;
- the release metadata explicitly sets `msi_byte_reproducible_claim = false`;
- byte-identical MSI is not a release assertion until the upstream issue is resolved or AetherCore adopts a separately verified canonicalization strategy.

## Signing model

Production signing occurs only after binaries are fully built.

Order:

1. sign service/desktop/broker/hardener EXEs;
2. build MSI from signed payload;
3. sign MSI;
4. build Burn bundle around the signed MSI and verified WebView2 prerequisite;
5. sign bundle.

`signtool` uses SHA-256 file digest and RFC3161 timestamping. When `-RequireSigning` is enabled, absence of the certificate thumbprint or HTTPS timestamp URL is fatal.

The protected GitHub workflow targets a dedicated self-hosted Windows signing runner. The signing certificate is expected to be pre-provisioned in the Windows certificate store and preferably non-exportable; the private key is not checked into the repository and is not transported as a PFX secret.

## Release evidence

A release directory contains:

- signed/unsigned payload binaries according to gate mode;
- MSI;
- Burn setup executable;
- Rust and UI SBOMs;
- cargo-deny/pnpm audit/license evidence when online auditing is enabled;
- reproducibility-control report;
- `RELEASE-METADATA.json`;
- `SHA256SUMS.txt`.

## Release workflows

### Normal Windows CI

Validates source and creates an **unsigned** packaging candidate. It is useful for proving packaging buildability but is not a production release.

### Protected signing workflow

Runs on the dedicated self-hosted signing runner, requires approved dependency locks, audits dependencies, generates SBOMs, signs all required layers, verifies the Phase 9 principal/consent/filesystem/release-closure gates, and uploads the signed candidate for controlled release review.

## Release-blocking failures

Any of these must stop the release:

- lockfile/baseline drift;
- malformed dependency freeze;
- Rust advisory/license/source denial;
- pnpm high/critical advisory;
- missing SBOM or invalid SBOM JSON;
- failed Rust/UI production build;
- invalid Microsoft signature on the downloaded WebView2 bootstrapper;
- WiX build or MSI validation failure;
- missing required Authenticode identity in signed mode;
- failed signature verification;
- failed Phase 9 security/release-closure gate;
- failed disposable-VM installer lifecycle verification before public promotion.

## Native artifact mitigation gate

Before any production executable is signed, the release pipeline parses its PE header and requires AMD64 PE32+ plus `HIGH_ENTROPY_VA`, `DYNAMIC_BASE`, and `NX_COMPAT`. The check is deterministic and local and therefore remains active even when online supply-chain auditing is deliberately skipped after a previous authoritative audit in the same verification run.

## Burn signing order

The release order is payload EXE signing → MSI build/validation → MSI signing → Burn bundle build → detached Burn-engine signing/reattachment → final bundle signing. `sign-burn-bundle.ps1` uses the pinned WiX `burn detach`/`burn reattach` commands and the same pre-provisioned certificate identity as the rest of the release. The private key is never accepted as a repository file or command-line PFX path.


### Phase 9 freeze closure

The approved freeze binds both resolved lockfiles and the manifests/tool policies that produced them. `release/dependency-locks.sha256` covers `Cargo.lock` and `pnpm-lock.yaml`; `release/dependency-manifests.sha256` covers the root `package.json` pnpm pin, Cargo/Rust toolchain policy, WiX/.NET tool manifest, NuGet policy, and workspace dependency manifests; `release/dependency-freeze.json` records the pinned Rust/pnpm versions, direct lock hashes, and hashes of both baseline files. CI is verify-only and cannot silently create a new approved dependency graph.
