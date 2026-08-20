# AetherCore Phase 9 Validation Summary

## Source implementation status

Phase 9 source implementation is complete in this delivery for the requested Security & Release Closure scope:

- named-pipe client principal binding from the impersonated client token plus Windows session ID;
- principal-owned immutable plans, snapshots, status/history and mutation entry paths;
- Protocol v7 secret-free consent intents and elevated fixed-broker review;
- atomic one-shot consent consumption with plan transition in the SQLite safety ledger;
- exact cleanup file identity using volume serial + 128-bit file ID from the opened file handle;
- fail-closed dependency/build freeze mechanism covering locks, dependency/tool manifests and freeze metadata;
- prototype/demo/challenge/grant surface retirement;
- Phase 9 source audit and authoritative Windows verification script.

## Authoring-runtime checks completed

- `scripts/static_validate.py`: **114/114 PASS**.
- SQLite migrations: **PASS** for both a fresh database and a simulated Phase 8 → Phase 9 upgrade. The upgrade quarantines incomplete ownerless legacy plans, preserves terminal history, removes legacy challenge/grant tables, creates consent intents, and leaves legacy diagnostic/plan history ownerless and therefore outside Phase 9 owner-scoped reads.
- Configuration/source parsing: **PASS** for 29 TOML, 7 JSON, 4 YAML, 3 XML and 1 Python source files in the current tree.
- Product sanitation grep: **PASS** for retired demo/simulation/challenge/grant/old authorization-timestamp surfaces outside the historical migration boundary.
- `git diff --no-index --check`: **PASS** with no whitespace errors.

## Checks that were not executable in this environment

This authoring environment does not contain Cargo/Rust, PowerShell, pnpm, protoc or the .NET/WiX toolchain and cannot execute Windows SCM, UAC, NTFS, Authenticode or installer lifecycle behavior. Therefore this delivery does **not** claim that native compilation, Rust tests, PowerShell execution, WiX MSI/Burn construction, Authenticode signing, live token verification, or disposable-VM installer lifecycle verification passed here.

The supplied Phase 8 archive also did not contain the resolved lock/freeze evidence. The following are intentionally absent rather than fabricated:

- `Cargo.lock`
- `pnpm-lock.yaml`
- `release/dependency-locks.sha256`
- `release/dependency-manifests.sha256`
- `release/dependency-freeze.json`

Until an approved freeze is minted on the trusted dependency-freeze workstation, Phase 9 is intentionally **pre-freeze** and the authoritative Windows gate fails closed.

## Required trusted Windows sequence

After reviewing dependency/tool manifests on the trusted connected Windows freeze workstation:

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\bootstrap.ps1 -InstallPrerequisites -RefreshDependencyFreeze
```

Review and commit the two lockfiles plus all three `release/dependency-*` freeze evidence files together. Then run the authoritative Phase 9 gate:

```powershell
.\scripts\verify-phase9.ps1
```

For signed production packaging and disposable-VM lifecycle verification:

```powershell
.\scripts\verify-phase9.ps1 -InstallerLifecycle -RequireSigning
```

From a normal installed, non-elevated user session, also run:

```powershell
.\scripts\verify-phase9.ps1 -UserShellPrivilegeCheck -SkipOnlineSupplyChain
```

The protected signed-release workflow uses `verify-phase9.ps1 -ReleasePackaging -RequireSigning` after verifying the committed freeze.
