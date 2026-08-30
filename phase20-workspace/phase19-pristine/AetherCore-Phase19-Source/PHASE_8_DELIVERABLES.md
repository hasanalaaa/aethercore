# AetherCore Phase 8 Deliverables

## Milestone

**Phase 8 — Packaging, Production Hardening, WiX MSI Installer Pipeline & Security Verification**

Phase 8 converts the Phase 0–7 engineering tree into a production-oriented Windows release pipeline without weakening AetherCore's existing privilege boundaries. Protocol v6 and the service-owned mutation model remain unchanged.

## Delivered architecture

### Installer and prerequisite chain

- WiX Toolset **6.0.2** is pinned as a repository-local .NET tool.
- `installer/wix/Product.wxs` authors a native x64 per-machine MSI for Windows 11 client build 22621+, with a stable upgrade family and rollback-safe major-upgrade scheduling.
- `installer/wix/Bundle.wxs` authors the consumer Burn bootstrapper, rejects unsupported/non-client Windows before chaining prerequisites, and chains the Microsoft Evergreen WebView2 bootstrapper only when the runtime is not detected.
- The release acquisition script owns the fixed Microsoft WebView2 URL and refuses a downloaded prerequisite unless Authenticode reports a valid Microsoft signature.
- Direct MSI deployment remains available for managed environments; the Burn bundle is the consumer path because it carries prerequisite handling.

### Privilege and service model

- `aethercore-desktop.exe` remains non-elevated (`asInvoker`).
- `aethercore-consent-broker.exe` remains the one-shot UAC boundary (`requireAdministrator`).
- `aethercore-maintenance-service.exe` is installed as a LocalSystem own-process service named `AetherCoreMaintenance`.
- Windows Installer owns service creation/removal; the fixed post-`InstallServices` hardener is the single authority that applies service SID type `UNRESTRICTED` and delayed-auto configuration.
- The MSI deliberately does not depend on MSI `ServiceConfig` for SID/delayed-start semantics; install, repair, stop, and uninstall behavior remains explicit.
- No Phase 8 component adds shell/process/network privileges to the Tauri web-content capability set.

### Fixed-purpose install hardener

`apps/install-hardener` is a Windows-only helper invoked by a deferred, non-impersonating MSI custom action. It accepts only the literal verb `apply` and has no general command execution surface.

It uses fixed `%SystemRoot%\System32\sc.exe` and `icacls.exe` paths to:

- reinforce unrestricted service SID type for broad maintenance compatibility;
- enforce delayed automatic LocalSystem service configuration;
- apply a fixed service DACL;
- reset stale explicit ACLs before applying the allowlist;
- restrict `%ProgramFiles%\AetherCore` to SYSTEM/Administrators full control, Users read/execute, and service read/execute;
- restrict `%ProgramData%\AetherCore` to SYSTEM/Administrators/service access, with no ordinary Users ACE.

MSI Repair deliberately re-runs this helper so installation-policy drift can be repaired. `ARPNOREPAIR` is intentionally not authored.

### Uninstall and recovery-data behavior

The MSI removes binaries, shortcuts, registry install metadata, and the Windows service. It intentionally does **not** recursively purge `%ProgramData%\AetherCore` recovery/journal data. A lifecycle verifier places a sentinel in ProgramData and requires it to survive uninstall.

## Security verification delivered

### IPC boundary hardening

- Request and response frame allocation ceilings remain enforced.
- Request IDs are now bounded to 128 bytes and a safe ASCII grammar before dispatch.
- `decode_request_frame_bytes` exposes the production frame parser for deterministic malformed-input tests and the libFuzzer target.
- Tests cover truncated frames, invalid protobuf payloads, trailing bytes, and a deterministic malformed corpus without panics.
- `fuzz/fuzz_targets/ipc_frame.rs` connects `cargo-fuzz` directly to the production parser.
- `cargo-fuzz` is pinned to 0.13.2 in the runner script.

### Consent/path defenses

Additional tests verify that broker authorization rejects:

- executable paths that only share a prefix with the expected broker path;
- relative expected paths;
- path variants that are not the exact normalized broker executable identity.

The existing cleaner reparse/final-path-by-handle protections are preserved and statically re-audited.

### Installer lifecycle verifier

`scripts/verify-installer-security.ps1` is destructive by design and therefore requires an explicit disposable-machine acknowledgement. It verifies:

- service account, start mode, delayed start, exact binary path;
- `sc qsidtype` reports `UNRESTRICTED`;
- the service DACL matches the fixed policy;
- installation and state-directory ACLs are protected and contain no broad Everyone/Authenticated Users allow ACE;
- desktop manifest is `asInvoker`;
- broker manifest is `requireAdministrator`;
- the elevated installer does not auto-launch the desktop;
- deliberate Everyone/Modify ACL drift is removed by MSI Repair;
- uninstall removes service/binaries while preserving ProgramData recovery state.

## Supply-chain and release pipeline

### Dependency freeze

A production release requires all three artifacts to move together:

1. `Cargo.lock`
2. `pnpm-lock.yaml`
3. `release/dependency-locks.sha256`

`scripts/freeze-dependencies.ps1` creates or verifies this freeze. `build-release.ps1` refuses to release when the freeze is absent or the approved hashes drift.

The authoring container could not reach crates.io/npm, so this source delivery does not fabricate lockfiles. The first trusted connected Windows bootstrap must resolve them and commit the reviewed freeze. Subsequent release builds are fail-closed.

### Dependency policy

- `cargo-deny` 0.20.2 is pinned for Rust advisory/license/ban/source checks.
- wildcard Rust dependency versions are denied.
- unknown registries and unknown Git sources are denied; Git dependencies require immutable `rev` pinning.
- pnpm high/critical advisories fail the gate.
- pnpm license inventory and locked Cargo metadata are retained as release evidence.
- pnpm workspace policy enables dependency-age delay, exotic-subdependency blocking, strict dependency-build approval, and a narrow esbuild build allowlist.

### SBOM

- Rust CycloneDX is generated with pinned `cargo-cyclonedx` 0.5.9.
- UI CycloneDX 1.7 is generated from the pnpm lockfile.
- `SOURCE_DATE_EPOCH` is required for deterministic SBOM metadata.
- Generated JSON is parsed before release acceptance.

### Release ordering

The release pipeline intentionally performs:

1. dependency-freeze verification;
2. frozen UI restore and production checks;
3. locked native release build;
4. dependency/license audit;
5. SBOM generation;
6. reproducibility-control report;
7. payload EXE signing;
8. verified Microsoft WebView2 acquisition;
9. MSI build and validation;
10. MSI signing;
11. Burn bundle build around the signed MSI;
12. bundle signing;
13. SHA-256 release manifest and machine-readable release metadata.

The production signing workflow runs only on a dedicated self-hosted Windows runner with a pre-provisioned non-exportable certificate. Private signing-key material is not accepted as a repository file or ordinary GitHub secret.

## Reproducibility contract

Rust release inputs use:

- locked dependencies;
- `codegen-units = 1`;
- incremental compilation disabled;
- MSVC linker `/Brepro` and `/INCREMENTAL:NO`;
- fixed `SOURCE_DATE_EPOCH` for SBOM metadata.

An optional double native build compares privileged EXE hashes.

AetherCore intentionally does **not** claim byte-for-byte reproducible MSI output at this stage because current WiX has a known upstream nondeterminism boundary involving package code/summary timestamps. `verify-reproducible.ps1` records this as an explicit non-claim rather than hiding it.

## CI/CD

- `.github/workflows/ci.yml` runs the Windows source/security/build pipeline and can create a CI-only seed dependency freeze only when the delivered seed snapshot contains no lockfiles at all.
- `.github/workflows/release.yml` is stricter: it requires the approved dependency freeze and protected signing environment.
- Dependabot is configured for Cargo, npm, and GitHub Actions review queues.

## Authoritative Windows verification

Developer/source gate:

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\bootstrap.ps1 -InstallPrerequisites
.\scripts\verify-phase8.ps1
```

Build an unsigned packaging candidate:

```powershell
.\scripts\verify-phase8.ps1 -ReleasePackaging
```

Run the full signed lifecycle on a disposable Windows VM:

```powershell
$env:AETHERCORE_CODESIGN_THUMBPRINT = '<thumbprint>'
$env:AETHERCORE_TIMESTAMP_URL = 'https://<rfc3161-service>'
.\scripts\verify-phase8.ps1 -InstallerLifecycle -RequireSigning
```

Optional long-running parser fuzzing:

```powershell
.\scripts\verify-phase8.ps1 -LibFuzzer
```

## Authoring-runtime validation status

The Linux authoring runtime statically validates Phase 0–8 structure and security invariants but cannot honestly execute:

- native MSVC/Windows SDK compilation;
- WiX MSI/Burn compilation and ICE validation;
- Authenticode signing/timestamping;
- Service Control Manager/service-SID behavior;
- NTFS ACL repair/uninstall lifecycle;
- WebView2 prerequisite installation;
- Windows PE manifest extraction;
- native libFuzzer execution.

Those remain hard gates in `verify-phase8.ps1`, Windows CI, and a disposable Windows installer VM.

### PE mitigation gate

Production payloads are parsed as PE32+ AMD64 images before signing. `scripts/verify-pe-hardening.ps1` reads the PE headers directly and requires `HIGH_ENTROPY_VA`, `DYNAMIC_BASE`, and `NX_COMPAT` on the desktop, maintenance service, consent broker, and install hardener. This is a release-blocking check; it does not rely on localized `dumpbin` text.

### Burn two-piece signing and desktop privilege boundary

The consumer Burn bundle is signed using WiX's required two-piece flow: detach the Burn engine, Authenticode-sign the detached engine, reattach it, then Authenticode-sign the complete bundle. `scripts/sign-burn-bundle.ps1` owns this sequence and is the only bundle-signing path used by `build-release.ps1`.

A separate post-install check, `scripts/verify-user-shell-privilege.ps1`, must be run from a normal non-elevated user session. It launches the installed desktop and inspects the process token, failing if the desktop is elevated while independently confirming that the maintenance service remains on the LocalSystem side of the privilege boundary.

## Final static authoring gate

The completed Phase 8 source tree passes **98/98** platform-neutral repository/security invariants with zero failures. Independent parsers/structure checks report **0 errors** across TOML, JSON, WiX XML, Rust source structure, PowerShell gross structure, and cross-script references. Production `TODO`, `FIXME`, and `unimplemented!` markers are also absent.

The delivered source snapshot intentionally remains **pre-dependency-freeze** because the Linux authoring runtime cannot resolve the Windows/Rust/npm registries. On a trusted connected Windows machine, `bootstrap.ps1 -InstallPrerequisites` creates `Cargo.lock`, `pnpm-lock.yaml`, and `release/dependency-locks.sha256` together; those three dependency-freeze artifacts must then be reviewed and committed together before a production release build.
