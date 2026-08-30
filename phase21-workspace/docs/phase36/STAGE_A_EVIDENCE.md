# Phase 36 — Stage A evidence: canonical build and MSI realignment

## A1 — canonical recorded build script — PASS

Deliverable: [`phase21-workspace/scripts/build-arm64-msi.cmd`](../../scripts/build-arm64-msi.cmd)
plus [`installer/tauri.no-before-build.json`](../../installer/tauri.no-before-build.json).

`scripts/build-release.ps1` is the production x64 pipeline and cannot run here:
it hard-codes `-arch x64` and requires the signed dependency-freeze baseline,
the online supply-chain audit and an Authenticode signer, none of which exist
on this VM. `build-arm64-msi.cmd` is the ARM64 payload+package equivalent.

### Which binaries come from which invocation, and why not one

| # | invocation | produces |
|---|---|---|
| 1 | `pnpm --dir apps/ui install --frozen-lockfile` then `pnpm --dir apps/ui build` | `apps/ui/dist` |
| 2 | `cargo build --release -p aethercore-maintenance-service -p aethercore-consent-broker -p aethercore-update-broker -p aethercore-install-hardener -p aetherctl` | 5 native binaries |
| 3 | `tauri build --no-bundle` from `apps/desktop` | `aethercore-desktop.exe` |

[3] cannot be folded into [2]. The Tauri CLI drives its **own** cargo
invocation for `apps/desktop` and embeds the `apps/ui/dist` bundle produced by
[1] into the executable. Its cargo invocation resolves features independently
of [2], and it has a hard input dependency on [1]. That is also the second of
the two documented reasons a rebuild legitimately differs from a previous
build.

[2] is deliberately ONE invocation. Cargo unifies features across every package
selected in a single invocation, so the package SET is part of the recipe.
Splitting or extending it changes the emitted code. That is the first
documented reason a rebuild legitimately differs.

`aetherctl.exe` is built by [2] but is **not** an MSI payload file — see the
observation in A5 below.

### One defect found and fixed in the recipe itself

`tauri.conf.json` declares `beforeBuildCommand: "pnpm --dir ../ui build"`. The
Tauri CLI runs that hook from its own discovered app directory rather than from
the directory holding `tauri.conf.json`, so `../ui` resolved to
`<root>\ui` and the first run failed:

```
[ERROR] ENOENT: no such file or directory, lstat 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery\ui'
beforeBuildCommand `pnpm --dir ../ui build` failed with exit code 1
```

Step [1] already builds `apps/ui/dist`, so the hook is redundant. It is cleared
with a recorded config overlay (`installer/tauri.no-before-build.json`) passed
as `--config`, leaving the shared `tauri.conf.json` untouched for the x64
pipeline. `frontendDist` is a config path and IS resolved relative to
`tauri.conf.json`, so it continues to work.

## A2 — rebuild from main, artifacts recorded — PASS

`target/release` was moved aside to `target/release-preA2-hold` (7.0 GB, 13,243
files, preserved, not deleted) and the recipe was run from scratch.

Raw log: [`evidence/A2-build.log`](evidence/A2-build.log).
Manifest: [`evidence/A2-rebuild-manifest.txt`](evidence/A2-rebuild-manifest.txt).
Pre-build reference: [`evidence/A2-prebuild-baseline.json`](evidence/A2-prebuild-baseline.json).

| artifact | rebuilt size | rebuilt SHA-256 | installed size | same? |
|---|---:|---|---:|---|
| aethercore-consent-broker.exe | 586,240 | `09709ec8…7a181f` | 612,864 | differs |
| aethercore-desktop.exe | 6,349,824 | `5f8d771f…e8d4a1` | 6,375,424 | differs |
| aethercore-install-hardener.exe | 246,784 | `536984c7…23bab7` | 246,784 | same size, differs |
| aethercore-maintenance-service.exe | 9,810,944 | `f2978381…1bd1bd` | 9,810,944 | same size, differs |
| aethercore-update-broker.exe | 672,768 | `8d55850b…003482` | 698,880 | differs |
| libomp140.aarch64.dll | 599,504 | `d2649698…1a962e3` | 599,504 | **byte-identical** |
| update-trust.json | 83 | `d4ad925d…3c8b37` | 83 | **byte-identical** |

Differences are EXPECTED and are not a failure. The two non-code artifacts are
byte-identical because they are copied, not compiled. Byte-equality with the
hand-deployed set is explicitly not a criterion anywhere in Phase 36;
correctness is proven functionally by the verbs returning.

## A3 — package and validate — PASS

```
=== [5/6] wix build -arch arm64
ProductCode={FC8A3841-759D-B452-1864-161F84F56C03}
=== [6/6] wix msi validate (zero ICE required, no suppression)
=== BUILD OK: C:\AetherCore-P36\build\out\AetherCore-0.1.0-arm64.msi
```

- `wix build -arch arm64` exit 0.
- `wix msi validate` exit 0 (the recipe aborts on any non-zero exit).
- **ZERO ICE findings**: `Select-String 'ICE\d+'` over the entire build log
  returns 0 matches. No `-sice` or any other suppression appears anywhere in
  `build-arm64-msi.cmd`. Tranche 1's zero-ICE result holds; no regression.
- MSI: `AetherCore-0.1.0-arm64.msi`, 6,205,440 B,
  SHA-256 `3f3702946bd93ea1b905a182c50dab8e85a4bb998d151bf6272b42b66c94fd34`.
- Copied to the Mac at `~/Documents/p36-stage/out/AetherCore-0.1.0-arm64.msi`
  before any install step. (Not committed: 6 MB binary.)

## A4 — reinstall or upgrade: decided from the authoring, not guessed

`scripts/build-installer.ps1` derives the ProductCode deterministically:

```
first 16 bytes of SHA256("AetherCore/MSI/ProductCode/v1" + "AetherCore/<version>/<arch>")
read as a .NET Guid
```

Recomputed independently on the Mac:

| version | arch | ProductCode |
|---|---|---|
| 0.1.0 | **arm64** | `{FC8A3841-759D-B452-1864-161F84F56C03}` — **equals the installed ProductCode** |
| 0.1.0 | x64 | `{2D5CF2F8-EEFA-65EE-3780-E5A924AB1FB1}` |
| 0.1.1 | arm64 | `{84140FFD-5CBC-175D-928D-E493493F5F51}` |

The rebuilt MSI printed exactly `{FC8A3841-759D-B452-1864-161F84F56C03}`,
confirming the derivation.

`Product.wxs` authors `UpgradeCode {45598C77-2C32-5BCE-8510-19C7E51EE3B8}` and
`<MajorUpgrade Schedule="afterInstallInitialize" ...>` **without**
`AllowSameVersionUpgrades`.

**Decision: same-version REINSTALL, `REINSTALL=ALL REINSTALLMODE=amus`.**

Justification: the rebuilt package has the identical ProductCode AND the
identical ProductVersion `0.1.0` as the installed product. Windows Installer
therefore cannot treat it as a major upgrade — `FindRelatedProducts` will not
match a product whose version is not greater, and `AllowSameVersionUpgrades` is
not authored. Without `REINSTALL`/`REINSTALLMODE` the transaction would be a
no-op maintenance pass and the pre-fix binaries would remain. `REINSTALLMODE=amus`
forces `a` (all files, regardless of version comparison — required here, since
the file versions are unchanged), `m` (all HKLM registry), `u` (all HKCU),
`s` (reinstall shortcuts). A version bump is reserved for Stage B4, where
0.1.1 produces a genuinely different ProductCode under the same UpgradeCode and
so exercises the real major-upgrade path.
