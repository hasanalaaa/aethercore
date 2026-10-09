# Installing the unsigned AetherCore 0.1.12

Draft written 2026-10-06, refreshed 2026-10-09 against `main` `2a0d797a`. Sources are cited; what the repository does not say
is left out. For what the release is and is not, read `RELEASE-NOTES-0.1.12.md` first.

## What you receive

This note is for someone installing the unsigned release on a Windows PC. Sections 1 and 3 to 5 need only
PowerShell; section 2 is for developers or reviewers who also have the source and Python.

The release folder (`out/release/0.1.12`, `scripts/build-release.ps1`) holds:

| file | what it is |
|---|---|
| `artifacts/AetherCoreSetup-0.1.12-x64.exe` | the setup bundle |
| `artifacts/AetherCore-0.1.12-x64.msi` | the Windows Installer package inside it |
| `payload/*.exe` | the six executables: desktop, maintenance service, consent broker, update broker, install hardener, `aetherctl` |
| `SHA256SUMS.txt` | one line per file: `<sha256>  <relative path>` |
| `RELEASE-METADATA.json` | version and `source_commit` |
| `RC-PROVENANCE.json` | the receipt: the hash of every artifact, the hash of `SHA256SUMS.txt` (`inventory_sha256`), and `signing: unsigned by owner decision D32` |

## 1. Check the files before you run them

The release is unsigned (D32), so the signature cannot vouch for it. Check the SHA-256 instead.

```powershell
# the setup bundle's hash, compared with its line in SHA256SUMS.txt
Get-FileHash -Algorithm SHA256 .\artifacts\AetherCoreSetup-0.1.12-x64.exe
Select-String 'AetherCoreSetup-0.1.12-x64.exe' .\SHA256SUMS.txt

# the file must report no signature: that is what an unsigned-by-decision release looks like
(Get-AuthenticodeSignature .\artifacts\AetherCoreSetup-0.1.12-x64.exe).Status   # expect: NotSigned
```

The two hashes must be identical (`Get-FileHash` prints capitals and `SHA256SUMS.txt` lower case, so
compare without regard to case). The rc-provenance gate requires every artifact to be natively
`NotSigned` and rejects a `Valid` one, so a `Valid` signature here means this is **not** the D32 release
(`docs/phase87/P87-RELEASE-PROVENANCE.md`).

**A limit of this check** (my reading, not a repository statement): `SHA256SUMS.txt` ships in the same
folder as the files, so matching it proves the download is intact, not that the folder is the one the
owner built. To detect a swapped folder, compare the setup bundle's hash against a copy you received
from the owner by another route.

## 2. Full verification (optional; needs the source and Python)

`scripts/rc-provenance.py verify` checks the receipt, the inventory, every hash and the unsigned status
together. It runs from a checkout of the release's source commit (`source_commit` in
`RELEASE-METADATA.json`):

```powershell
python scripts/rc-provenance.py verify --release-root <release folder> --expected-sha <source_commit> `
  --unsigned-d32 --receipt-sha256 <sha256 of RC-PROVENANCE.json> --bundle-sha256 <sha256 of the setup exe>
```

## 3. Install

Run `AetherCoreSetup-0.1.12-x64.exe`. Expect the SmartScreen / "unknown publisher" warning (D32). The
repository does not document that dialog's buttons, so they are not described here. The installed service
is `AetherCoreMaintenance`; its files are in `C:\Program Files\AetherCore` and its data in
`C:\ProgramData\AetherCore` (`release/UNINSTALL.txt`).

## 4. Upgrading from an earlier version, and your data

Back up first. `C:\ProgramData\AetherCore` holds your operation journal, scans, plans, findings and the
local signing key, and **every full uninstall deletes it**: the installer runs `PurgeMachineData`
(`release/UNINSTALL.txt`, `installer/wix/Product.wxs`).

- A **major upgrade** (installing the new version over the old one, without uninstalling first) is built
  not to purge it, **but only from a build that already has that protection** (see the warning below): `PurgeMachineData` is conditioned on `NOT
  UPGRADINGPRODUCTCODE` (`installer/wix/Product.wxs`), and the lifecycle test checks that machine data
  survives an MSI repair and a major upgrade (`DBT-P87-015`). Whether an in-place upgrade from 0.1.11 was
  run on a real machine for this release is not stated in the repository.
- **Upgrading from `v0.1.11-rc.1` (`ba9973bd`, 2026-09-09) deletes your data.** The protection is the
  `NOT UPGRADINGPRODUCTCODE` condition on `PurgeMachineData`, added in `a013de59` (2026-09-26,
  `DBT-P74-001`). Windows Installer removes the old product with the *old* package's own rules, and
  `v0.1.11-rc.1`'s rule has no such condition (`installer/wix/Product.wxs`, line 344 at that tag). That
  consequence is reasoned from how Windows Installer works and from that file; it was not run on a real
  upgrade from `rc.1`. Back up first. The upgrade check in the acceptance run starts from the 0.1.11
  build `578e9cd4` (2026-10-05), which has the condition (bundle `AetherCoreSetup-0.1.11-x64.exe`, SHA-256
  `0a3f66837025f33a002ba480256794f839e7ba88e7649c7934e7b9b9d6b9e0d2`); its result is run 7's.
- **Removing the old version first does delete the data.** The one real transition measured, the owner's
  own PC on 2026-10-05, removed 0.1.11 and the database fell from 2,445,312 bytes to a fresh 4,096; the
  protected backup taken first brought back all 6 files and the service reopened them (`DBT-P87-013`).
  That is why the acceptance run takes a SYSTEM/Administrators-only backup with a SHA-256 manifest before
  it touches anything.

To back up by hand: stop the `AetherCoreMaintenance` service, copy `C:\ProgramData\AetherCore`
somewhere only you can read, and start the service again.

## 5. Uninstalling

Uninstall removes the program files, the service, **all** machine data, and the registry keys and Start
Menu folder the product created. It keeps reports and bundles you exported elsewhere and any change
AetherCore made to your system on your instruction (startup items, drivers, cleaned files). A small
WebView2 cache of about 23 MB stays in `%LOCALAPPDATA%\com.aethercore.desktop` and Windows removes it
with the profile (`release/UNINSTALL.txt`). Uninstalling does not roll back a driver install: roll it
back first.
