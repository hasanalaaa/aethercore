# Build, Verify, Package, and Release

## Supported build host

AetherCore is Windows-native. The production build contract targets **Windows 11 x64 build 22621 or newer** and pins:

- Rust 1.97.1;
- pnpm 11.22.0;
- WiX Toolset 6.0.2 via repository-local .NET tool manifest;
- Node.js LTS baseline used by CI;
- Microsoft C++ Build Tools / Windows SDK;
- Microsoft Edge WebView2 Evergreen.

## First bootstrap

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\bootstrap.ps1 -InstallPrerequisites -RefreshDependencyFreeze
```

If the source snapshot has never been dependency-frozen, bootstrap resolves and records:

- `Cargo.lock`;
- `pnpm-lock.yaml`;
- `release\dependency-locks.sha256`;
- `release\dependency-manifests.sha256`;
- `release\dependency-freeze.json`.

The manifest baseline also binds the root pnpm pin, Cargo manifests/toolchain, WiX .NET tool manifest, NuGet signature policy, and other dependency-resolution inputs. Review and commit the entire freeze set together. Once present, bootstrap verifies the approved hashes instead of silently refreshing them.

## One-command development setup and run

```powershell
.\scripts\setup-and-run.ps1 -InstallPrerequisites -RefreshDependencyFreeze
```

This bootstraps prerequisites, executes the **Phase 15 verification gate**, starts the maintenance service console elevated for development, and launches the Tauri desktop non-elevated.

## Development run

```powershell
.\scripts\run-dev.ps1
```

Development elevation remains limited to the privileged native boundary. The desktop process is not permanently elevated.

## Authoritative Phase 15 secure-update and support-export gate

```powershell
.\scripts\verify-phase15.ps1
```

This inherits the complete Phase 0–14 verification chain, including dependency freeze, principal/consent/file-identity security, Operation Kernel/IPC v7, design/motion qualification, full English/Arabic localization/RTL validation, engine-reliability/fault-injection gates, and the owner-bound autonomous read-only scheduler. Phase 15 then adds split-authority signed update orchestration, bounded user-scope download/upload staging, Authenticode/hash revalidation, the global Update mutation lease, preview-first privacy sanitization, deterministic support archives, and independent installation-key fingerprint verification. The inherited Phase 13 reliability guarantees include:

- finite WMI and System Restore enumeration waits with explicit `WBEM_S_TIMEDOUT` / terminal `WBEM_S_FALSE` handling;
- shared monotonic collector deadlines, hierarchical cooperative cancellation and watchdog quarantine;
- structured Event Log/WHEA rendering through `EvtRenderEventValues`, with pre-allocation byte/property caps and RAII event-handle ownership;
- byte-level ATA SMART and NVMe health parsing with checked offsets, lengths, returned-byte counts and vendor-tail tolerance;
- typed `PermissionDenied`, timeout, malformed, I/O and provider failures that preserve successful sibling evidence;
- bounded provider-fault details and visible diagnostic journal/spawn failures;
- deterministic malformed-buffer, watchdog, panic, quarantine, EventLog-allocation and provider-isolation fault injection.

Optional inherited read-only telemetry remains available:

```powershell
.\scripts\verify-phase15.ps1 -LiveTelemetry
```

Run the Phase 13 ignored read-only provider probes on a Windows lab machine with:

```powershell
.\scripts\verify-phase15.ps1 -LiveReadOnlyFaultInjection
```

For the inherited Phase 14 **eligibility-only** live probe (no workload is started and no mutation is possible):

```powershell
.\scripts\verify-phase15.ps1 -LiveReadOnlySchedulerProbe
```

Optional Phase 5 read-only startup inventory can still be combined with the inherited telemetry gate:

```powershell
.\scripts\verify-phase15.ps1 -LiveTelemetry -IncludePhase5Inventory
```

## Dependency freeze

Create/update only after deliberate dependency review:

```powershell
.\scripts\freeze-dependencies.ps1 -Refresh
```

Verify without changes:

```powershell
.\scripts\freeze-dependencies.ps1 -VerifyOnly
```

A production release refuses missing or drifting freeze artifacts.

## Unsigned packaging candidate

For CI or local packaging validation:

```powershell
.\scripts\verify-phase15.ps1 -ReleasePackaging
```

This builds the release payload, generates SBOM/evidence, downloads and Authenticode-verifies the Microsoft WebView2 Evergreen bootstrapper, builds/validates the MSI, and builds the Burn bundle. Without signing configuration, the candidate remains explicitly unsigned.

## Signed production release candidate

Configure a code-signing certificate already installed in the Windows certificate store and an HTTPS RFC3161 timestamp service:

```powershell
$env:AETHERCORE_CODESIGN_THUMBPRINT = '<certificate thumbprint>'
$env:AETHERCORE_TIMESTAMP_URL = 'https://<timestamp-service>'
.\scripts\build-release.ps1 -RequireSigning -CertificateStore LocalMachine
```

The signing order is deliberate:

1. native EXE payload;
2. MSI built from signed payload;
3. MSI signing;
4. Burn bundle built around signed MSI;
5. Burn bundle signing.

Do not put a PFX/private key in the repository. The protected GitHub release workflow expects a dedicated self-hosted Windows signing runner with a pre-provisioned non-exportable certificate.

## Full installer lifecycle security test

This **installs, repairs, and uninstalls AetherCore and deliberately injects ACL drift**. Use only a disposable Windows VM:

```powershell
.\scripts\verify-phase15.ps1 -InstallerLifecycle -RequireSigning
```

The lifecycle requires:

- unrestricted service SID with service-specific ACL identity;
- LocalSystem service account;
- delayed automatic start;
- fixed service DACL;
- protected Program Files/ProgramData ACLs;
- desktop `asInvoker` manifest;
- broker `requireAdministrator` manifest;
- no elevated auto-launch of the desktop;
- MSI Repair closes injected Everyone/Modify ACL drift;
- uninstall removes service/binaries while preserving ProgramData recovery state.

## IPC fuzzing

The current Phase 15 gate inherits the Phase 11/10/9 malformed-frame corpus and always runs the deterministic malformed corpus. For libFuzzer:

```powershell
.\scripts\verify-phase15.ps1 -LibFuzzer
```

The fuzz target calls the production framed-protobuf decoder rather than a duplicate parser.

## SBOM and dependency evidence

Generate independently when needed:

```powershell
$env:SOURCE_DATE_EPOCH = (Get-Content .\release\source-date-epoch.txt -Raw).Trim()
.\scripts\generate-sbom.ps1
.\scripts\audit-dependencies.ps1
```

The release pipeline generates Rust CycloneDX SBOMs with pinned `cargo-cyclonedx` 0.5.9 and UI CycloneDX 1.7 with pnpm. Rust advisory/license/source policy is enforced with pinned `cargo-deny` 0.20.2.

## Reproducibility controls

```powershell
$env:SOURCE_DATE_EPOCH = (Get-Content .\release\source-date-epoch.txt -Raw).Trim()
.\scripts\verify-reproducible.ps1 -NativeDoubleBuild
```

Rust release controls include one codegen unit, no incremental compilation, and MSVC `/Brepro` + `/INCREMENTAL:NO`.

AetherCore does **not** claim byte-for-byte reproducible MSI output while the known upstream WiX package-code/summary-timestamp nondeterminism remains unresolved. The exact shipped MSI is instead identified by its SHA-256 and Authenticode signature.

## Output layout

A release build writes beneath:

```text
out/release/<version>/
  payload/
  artifacts/
  evidence/
  prereqs/
  RELEASE-METADATA.json
  SHA256SUMS.txt
```

## Read next

- `docs/PRODUCTION_PACKAGING_SECURITY.md`
- `docs/RELEASE_SUPPLY_CHAIN.md`
- `docs/VALIDATION.md`
- `docs/THREAT_MODEL.md`

## Verify the live non-elevated desktop boundary

After an installer lifecycle run, sign in as a normal user and run this from a **non-elevated** PowerShell window:

```powershell
.\scripts\verify-phase15.ps1 -UserShellPrivilegeCheck -SkipOnlineSupplyChain
```

The check starts the installed desktop, inspects its process token, and fails if it is elevated. The LocalSystem maintenance service remains the privileged boundary.

The production Burn bundle uses a two-piece signing flow (`wix burn detach`, sign engine, `wix burn reattach`, sign full bundle) through `scripts/sign-burn-bundle.ps1`.


## Dependency freeze inherited from Phase 9

The authoritative gate never creates a dependency baseline. After dependency review, a trusted Windows freeze workstation runs `scripts\freeze-dependencies.ps1 -Refresh`; the resulting native/UI lockfiles and `release/dependency-*` evidence are committed together. All CI/release executions use `-VerifyOnly` and fail if either a dependency manifest or lock hash drifts.
