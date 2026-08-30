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

## A5 — install the realigned MSI — PASS (after one recorded refusal)

Snapshot taken first: `P36-MSI-BUILT {a2f665b8-0df6-46eb-9842-b7efca505671}`.

### Attempt 1 — `REINSTALLMODE=amus` — REFUSED, exit 1638

Observed vs expected: expected exit 0, observed **1638**
(`ERROR_PRODUCT_VERSION`, "Another version of this product is already
installed"). The machine was not modified — Windows Installer refused before
any action ran, and the service stayed RUNNING.

Raw cause line from the verbose log (`evidence/A5-install-key.txt`):

```
PROPERTY CHANGE: Adding PackagecodeChanging property. Its value is '1'.
Note: 1: 1729
Product: AetherCore -- Configuration failed.
Reconfiguration success or error status: 1638.
```

The rebuilt package has the same ProductCode and the same ProductVersion but a
**new PackageCode** (WiX generates one per build). Windows Installer will not
reconfigure from the cached package when the package identity has changed.

### Attempt 2 — `REINSTALLMODE=vamus` — exit 0

Adding `v` re-caches the changed package from source, which is the documented
mode for exactly this case. It remains the same-version reinstall decided in
A4; only the source-resolution flag changed.

```
msiexec /i C:\AetherCore-P36\build\out\AetherCore-0.1.0-arm64.msi \
        REINSTALL=ALL REINSTALLMODE=vamus /qn /l*v ...
MSIEXEC_EXIT=0
```

Log evidence (`evidence/A5-install-vamus-key.txt`): `InstallFiles` return 1,
`HardenInstalledSecurity` ran with sufficient privileges and returned 1,
`StartServices` returned 1, and every payload file logged
`Overwrite; Won't patch; REINSTALLMODE specifies all files to be overwritten`.

### Verification — every item of the Gate A list

Compared field-by-field against `evidence/verify-A0-baseline.json`
(post: `evidence/verify-A5-postinstall.json`):

| property | result |
|---|---|
| `sc qc` — LocalSystem, AUTO_START (DELAYED), correct binary path | **SAME** |
| `sc query` — STATE 4 RUNNING | **SAME** |
| `sc qsidtype` — SERVICE_SID_TYPE UNRESTRICTED | **SAME** |
| pipe SDDL | **SAME**, byte-for-byte |
| install-dir `icacls` (Users RX, no write; service SID RX) | **SAME** |
| `libomp140.aarch64.dll` present | **SAME** (true) |
| `ipc_probe.exe` absent | **SAME** (absent) |
| ARP entry `{FC8A3841-…}` AetherCore 0.1.0 | **SAME** |
| `HKLM\SOFTWARE\AetherCore\InstallVersion` | **SAME** (0.1.0) |
| `C:\ProgramData\AetherCore` + state/logs/support-staging | intact |

### The realignment itself — the defect this stage existed to close

| file in INSTALLFOLDER | changed by the install? | equals the A2 rebuild? |
|---|---|---|
| aethercore-consent-broker.exe | **REPLACED** | yes |
| aethercore-desktop.exe | **REPLACED** | yes |
| aethercore-install-hardener.exe | **REPLACED** | yes |
| aethercore-maintenance-service.exe | **REPLACED** | yes |
| aethercore-update-broker.exe | **REPLACED** | yes |
| libomp140.aarch64.dll | unchanged | yes (byte-identical either way) |
| update-trust.json | unchanged | yes (byte-identical either way) |
| aetherctl.exe | unchanged | **not an MSI payload file** |

The registered package and the installed files now agree. A future `msiexec /f`
repair or reinstall can no longer revert the IPC fixes, because the package
itself now carries binaries built from `main` at `218e0d8`.

**Recorded observation, not diagnosed:** `aetherctl.exe` (3,611,136 B, SHA-256
`1fc95bec…`) sits in `C:\Program Files\AetherCore` but is not authored in
`installer/wix/Product.wxs`. No MSI component owns it, so no MSI action
installs, repairs or removes it. Expected for A5's "package and installed files
agree" would be an exact set match; observed is the seven authored files plus
this one unmanaged file. Consequences are carried forward to B2 and B3.

### Verb runs — 8/8 RETURNED

Run with the **rebuilt** `aetherctl.exe`
(SHA-256 `ba2286f4af718fe9acec60d3f728cf9be719885aa1e1d25b5cd24c7060aac766`,
the artifact this recipe produces) against the newly installed service, under
both actual-token contexts via one-shot Scheduled Tasks:

| context | whoami | IsInRole(Administrator) | service detect | doctor | optimize status | scan status |
|---|---|---|---|---|---|---|
| P36StandardUser (RunLevel Limited) | `hasanalaaa3a44\p36standarduser` | False | RETURNED | RETURNED | RETURNED | RETURNED |
| P36Admin (RunLevel Highest) | `hasanalaaa3a44\p36admin` | True | RETURNED | RETURNED | RETURNED | RETURNED |

`doctor` returns the typed `diagnostics.stateUnavailable` rejection, which is a
PASS by the brief. Transcripts: `evidence/verbs-A5-postinstall-{STD,ADMIN}.txt`.
Baseline for comparison: `evidence/verbs-A0-baseline-{STD,ADMIN}.txt` (also 8/8).

## GATE A — PASS

Registered package and installed files agree (with the one recorded
`aetherctl.exe` exception), every security property is unchanged, and eight
verb runs return.

Snapshot `P36-MSI-ALIGNED {7d0696ae-ebc7-4c67-b076-438855d175f3}` — this is the
lifecycle recovery point for Stage B.
