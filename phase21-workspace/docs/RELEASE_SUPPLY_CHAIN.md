# Release & Software Supply-Chain Pipeline

## Release philosophy

AetherCore release artifacts are accepted only when the source graph, dependency graph, security evidence, signing identity, and packaging order are explicit. “Build succeeded” is not a release criterion.

## Dependency freeze state machine

### Seed state

A source snapshot may begin without lockfiles only when it has never been dependency-frozen. This is the state of the authoring package when outbound registry access was unavailable.

### Freeze

On a freeze source that meets the three criteria below:

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

`ci.yml` and `release.yml` are **verify-only**. `windows-installer.yml` runs `-Refresh`, and that is not a contradiction: **a refresh mints a candidate, never an approved graph.** Approval is the commit, and the commit is made by a person. No workflow in this repository can approve a dependency graph, because no workflow can commit one.

## What makes a freeze source acceptable

Until P60 this document, the freeze script, `omega-evidence.py`, the release
blocker and the ledger all said "the trusted dependency-freeze workstation" —
twenty-odd occurrences, not one criterion. The only attributes ever attached to
the phrase, both in passing prose, were *Windows* and *connected*. A precondition
nobody wrote down is why `OMEGA-RB-001` stood open for thirty-nine phases while
CI cleared it on every run and threw the result away.

The phrase is retired. **Trust is not a property of the machine; it is a property
of the evidence the machine leaves behind.** Three criteria, each checkable:

**1. The resolution is reproducible on an independent machine.**
Two hosts that share nothing but the pinned manifests must produce the same
lockfile bytes. This is the criterion that makes the host replaceable, and it is
measured, not assumed: on 2026-09-13 a GitHub-hosted `windows-2025` runner and a
macOS 26.6 laptop each ran the resolution against the same manifests and produced
`Cargo.lock` with sha256 `c95e3c79872d731d777c1baaaa2944d8820ebecf9cea7f3c97de29ba7e0cbb50`
— byte-identical, across two operating systems.
*Fails if:* the source installs its toolchain from somewhere other than the pins,
carries registry overrides in `~/.cargo/config`, or cannot be reproduced at all.

**2. The run leaves a durable third-party record naming the exact source commit.**
A freeze nobody can re-read is a freeze you have to take on someone's word. A CI
run id satisfies this: the log names the commit, the command, the tool versions
and the timestamps, it is retained, and it is readable without the freeze
author's cooperation. A developer machine satisfies it only if the author
transcribes those facts by hand, and P59 found that none of the previous forty
phases ever did.
*Fails if:* the only evidence that the refresh happened is the diff it produced.

**3. The graph it produces is compiled on the machine that produced it, before
the output leaves that machine.**
This is the criterion that has actual teeth, and P60 exists because nothing
enforced it. On 2026-09-13 a refresh resolved `llama-cpp-2`/`llama-cpp-sys-2` to
0.1.156, whose `LlamaSampler::penalties` gained a parameter;
`crates/intelligence-core/src/llama.rs` does not compile against it. Neither the
runner's identity nor its ephemerality would have caught that. Compiling it did,
on both hosts, identically. `windows-installer.yml` therefore uploads the freeze
set **after** the build step, not before.
*Fails if:* the freeze artifact can exist without a green build of the tree it
describes.

### Where that leaves the two candidate sources

A **GitHub-hosted runner qualifies.** It meets 1 (measured above), meets 2 by
construction, and meets 3 now that the upload follows the build. Its ephemerality
is a genuine advantage — a fresh VM per run cannot carry a developer's
accumulated cargo or npm state into the resolution — and `DBT-P55-007` records
the one supply-chain control anyone has actually measured on both:
`signatureValidationMode=require` is **inert** on the developer machine (a cold
restore with deliberately corrupted `trustedSigners` fingerprints still exited 0)
and **enforced** on CI, which is where `NU3034` surfaced.

The argument against it is real and is not dismissed: the runner is infrastructure
this project does not control, and after the run there is nothing left to inspect
but the log. That is exactly why criterion 1 exists. Reproduction on a second,
unrelated machine is what removes the need to trust the first one, and it is
cheap — the resolution takes about a minute.

A **developer workstation does not qualify on its own.** It fails criterion 2
outright and it has no way to satisfy criterion 1 without a second machine
anyway. It becomes acceptable the moment it is the *second* machine: a
workstation that reproduces the hosted runner's bytes is criterion 1 being
satisfied, and that is a stronger position than either host alone.

### What approval still is

None of this is a judgement about whether the graph *should* ship. Criteria 1–3
establish that the freeze is what it claims to be; whether the project accepts
these dependencies is a risk acceptance, it belongs to the owner, and it happens
when he commits the files.

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
