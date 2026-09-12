# `DBT-P49-002` — the consumer installer's UI

P49 installed `AetherCoreSetup.exe` for the first time anyone ever had, and
recorded four defects by observation. This is what each one was, what caused it,
and what it looks like fixed.

The app behind this installer is careful about every number it shows. Its
installer was not, and this is the first thing any user sees — before the
product they installed.

## The four findings, and their causes

### 1. The title bar read "AetherCore Setup Setup"

**Cause.** `Bundle.wxs` set `Name="AetherCore Setup"`. The WiX standard
bootstrapper composes its window caption as `<Name> Setup`, so a `Name` that
already ended in "Setup" got a second one. The same `Name` is the bundle's
Add/Remove Programs display name, so the entry read the same way.

**Fixed.** `Name="AetherCore"`. The caption becomes "AetherCore Setup" and the
ARP entry reads as the product rather than as the act of installing it.

### 2. The logo was WiX's stock placeholder

**Cause.** `bal:WixStandardBootstrapperApplication` had no `LogoFile`, so the
bootstrapper used its own built-in 852-byte image. That made a **third** mark in
circulation, after the shipped icon and the in-app mark that `DBT-P48-001`
deliberately settled.

**Fixed.** `LogoFile` now points at `apps/desktop/icons/64x64.png` — the Æ mark,
taken from the existing icon pipeline rather than authored a second time.

The reference is provenance-checked rather than trusted. `build-installer.ps1`
reads the expected hash out of `apps/desktop/icons/SOURCE.json`, the icon
pipeline's own record, and refuses to build if the file on disk is not the one
the pipeline generated:

```
SOURCE.json  64x64.png = 492020a6494cde65608ef3dd87f61715ee836be10071f2c4e495c76eddee28ca
on disk                = 492020a6494cde65608ef3dd87f61715ee836be10071f2c4e495c76eddee28ca   MATCH
```

The expected value is read from the provenance record, not pinned a second time
in the build script — a second copy is how `DBT-P42-012` happened.

**Recorded, not fixed:** `SOURCE.json` names `design/icon/aethercore-mark.svg`
as the source that generated the set, and **that file does not exist in the
tree**. The generated PNGs match their recorded hashes, so the shipped artwork
is authentic; but the SVG it came from is not in the repository, so the icon set
cannot currently be regenerated from source. Raised as `DBT-P55-005`.

### 3. The welcome screen showed no version and no license

**Version.** `ShowVersion="yes"` was already set and is unchanged. The version
comes from `$(var.ProductVersion)`, which `build-installer.ps1` takes from
`Get-ProductVersion.ps1` — the single decider `product-identity` became in P46.
No second literal was introduced. Whether the version now renders is a question
about the theme, not the value, and it is verified by screenshot in Item 4.D
rather than asserted here.

**License — deliberately still absent.** `LicenseUrl` stays `""`.

**The repository contains no product license text.** `Cargo.toml` declares
`license = "Apache-2.0 OR MIT"`, but there is no `LICENSE` or `EULA` file at the
repository root or the workspace root.

The two licence files that *do* exist are not the product's and must not be
used here:

```
assets/models/licenses/Apache-2.0.txt        11,358 B
assets/models/licenses/Qwen-GGUF-NOTICE.txt  11,343 B
```

Those are the **embedded model's** licences — the Qwen GGUF weights — shipped as
MSI payload because that model is redistributed inside the product. Displaying
the Qwen notice, or a bare Apache-2.0 text, as the thing a user accepts before
installing AetherCore would be presenting an unrelated document as this
product's terms. That is precisely the invention this item must not commit.

So there is nothing truthful to display, and an installer that shows invented
licence text is worse than one that shows none. With the `hyperlinkLicense`
theme and an empty `LicenseUrl`, the standard bootstrapper hides the link
rather than rendering a dead one.

This is **owner action**: supply the licence text, and `LicenseUrl` (or a
`LicenseFile`) gets pointed at it. It was not invented here.

### 4. Progress sat on "Initializing..." for the entire 30-second install

This is the one that matters most: it is a measuring instrument that lies, in
the place a user is most likely to believe it.

**Cause.** The standard bootstrapper builds its progress line from the
**executing package's `DisplayName`**. The two `ExePackage` entries (`VCRedist`,
`WebView2EvergreenBootstrapper`) both carried one. `MsiPackage` did not.

On a machine where both prerequisites are already present — which is every
machine P49 tested on, and most consumer machines — Burn detects them as
`Present`, resolves them to `execute: None`, and the **only** package that
actually executes is the MSI. The one package with no `DisplayName` was the one
package that ran, so the progress line never advanced past its initial state.

That also explains the asymmetry P49 noticed and could not account for: the
*uninstall* path named the package correctly, because there the MSI is
identified by its own `ProductName` from the installed product rather than by
the chain's authored `DisplayName`.

**Fixed.** `MsiPackage` now carries `DisplayName="AetherCore"`, so the progress
line names the package as it executes.

**Burn can do this**, so no "why it cannot" is owed. What Burn does *not* give
is a meaningful percentage for an MSI whose own progress it cannot subdivide;
the bar advances per package, not per file. Naming the executing package is the
honest improvement available, and it is the one made.

## What changed

| file | change |
|---|---|
| `installer/wix/Bundle.wxs` | `Name`, `LogoFile`, `MsiPackage/@DisplayName` |
| `scripts/build-installer.ps1` | resolves and hash-checks the logo against `SOURCE.json`, passes `-d LogoFile=` |

Nothing was changed about the chain order, the detect conditions, the
`Permanent`/`Vital` flags, or the `Visible="no"` on the MSI. Those were settled
by `DBT-P41-001` and P47 §47.9 and are out of this item's scope.

## Still owed

* **Screenshots of every screen** — Item 4.D. The owner has never seen this
  installer.
* **Licence text** — owner action, above.
* **`DBT-P55-005`** — the icon source SVG is missing from the tree.

---

# Item 4.A — the artefacts, built through CI

Built by the green CI run, not locally, so the artefact is reproducible rather
than a one-off from this machine's accumulated setup.

| | |
|---|---|
| run | **`34683812129`**, conclusion `success`, 58m0s |
| commit | `d39e490181947c7d7d09d9cfbd06dd123a8e1dc8` |
| workflow | `.github/workflows/windows-installer.yml` |

## The two artefacts

| artefact | bytes | sha256 |
|---|---|---|
| `AetherCoreSetup-0.1.11-x64.exe` | 1,121,407,585 | `747bb6ee3265c4b141bfc58277c4fea71bc8155826b54503f3b7562556b6de3f` |
| `AetherCore-0.1.11-x64.msi` | 1,100,300,288 | `c335e5f9055dc73e10bf450d2e8db5a4b0d2ef7e5036cd810574398e11abd946` |

The MSI is **not** in the uploaded artifact — the workflow's upload path filter
is `out/release/**/*.exe`. Its hash above was taken from the copy inside the
bundle's own `WixAttachedContainer`, extracted with WiX, which is the byte the
bundle will actually install. Worth fixing in the workflow so the MSI is
uploaded too; recorded rather than changed in this pass.

## The gates, from the run's own log

```
AUTHORED_FILES=17
MSI_FILE_ROWS=16
PAYLOAD_CHECK=PASS every MSI file is authored in Product.wxs; no developer artefacts
MSI built:    ...\artifacts\AetherCore-0.1.11-x64.msi
Bundle built: ...\artifacts\AetherCoreSetup-0.1.11-x64.exe
```

* `wix msi validate` — **output empty. 0 ICE lines in the entire run log**, and
  the call is `dotnet tool run wix msi validate $msi` with no `-sval`, no
  `-sice`, no suppression of any kind.
* payload check — **PASS**, **16 file rows**, as EXPECTED.
* `vcomp140.dll staged from C:\Program Files\Microsoft Visual Studio\18\
  Enterprise\VC\Redist\MSVC\14.44.35112\x64\Microsoft.VC143.OpenMP\vcomp140.dll`
  — found by the new `vswhere` probe, and it passed the unchanged strict hash
  check, which is what closes `DBT-P42-012` check 5.

## The Burn chain, read from its own manifest

Extracted with `wix burn extract`, then read out of `manifest.xml` as XML — not
a string scan of the binary.

```
1. ExePackage   Id=VCRedist
2. ExePackage   Id=WebView2EvergreenBootstrapper
3. MsiPackage   Id=AetherCoreMsi
```

**EXPECTED `VCRedist` → `WebView2` → `AetherCoreMsi`. Observed exactly that.**

## `DBT-P49-002` verified in the built artefact

These are read out of the shipped bundle, not out of the source that was meant
to produce it.

| finding | evidence from the artefact |
|---|---|
| "AetherCore Setup Setup" | `Registration/Arp/@DisplayName` = **`AetherCore`**. Caption becomes "AetherCore Setup", ARP entry reads "AetherCore" |
| stock WiX logo | embedded `ba/logo.png` is **1,466 B**, sha256 `492020a6…ee28ca` — **byte-identical to `apps/desktop/icons/64x64.png`**, the Æ mark. WiX's placeholder is 852 B; it is not in this bundle. The theme places it via `<ImageControl X="11" Y="11" Width="64" Height="64" ImageFile="logo.png"/>` |
| version absent | `Registration/@Version` = **`0.1.11`**, and `WixBundleProperties/@DisplayName` = `AetherCore`. Sourced from `Get-ProductVersion.ps1`, the single decider |
| progress stuck on "Initializing..." | `WixPackageProperties` now carries a `DisplayName` for **all three** packages: `Microsoft Visual C++ 2015-2022 Redistributable`, `Microsoft Edge WebView2 Runtime`, and **`AetherCore`** — the last being the one that was missing and the only one that executes on a machine with the prerequisites already present |
| licence absent | still absent, deliberately. See above |

The first four are fixed in the artefact. Whether the version and the progress
text *render* as intended is a claim about pixels, and pixels are Item 4.D.
