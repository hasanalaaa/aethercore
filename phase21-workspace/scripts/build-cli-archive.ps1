[CmdletBinding()]
param(
    [ValidatePattern('^\d+\.\d+\.\d+$')][string]$Version,
    [ValidateSet('x64','arm64')][string]$Arch = 'x64',
    [string]$OutDir = 'out/cli'
)
# ---------------------------------------------------------------------------
# The portable aetherctl archive (Phase 37 Stage 3).
#
# WHY AN ARCHIVE AND NOT AN INSTALLER
#   The CLI has to be installable on a machine with no desktop app, no GUI, no
#   interactive session and no package manager. A zip that is expanded and put
#   on PATH needs none of those. The MSI stays the way you get the FULL product
#   (service + desktop + model); this is the way you get the command surface.
#
# WHAT GOES IN, AND WHY EXACTLY THIS
#   aetherctl.exe          the command surface
#   assets/vulndb/*        the CVE lane resolves this BESIDE THE EXECUTABLE, so
#                          it must travel with it or `sec audit` loses a lane.
#                          10 KB.
#   README.txt             the one documented install command
#   The CIS profiles are NOT here: they are compiled into the binary.
#   The 1.07 GB language model is NOT here: it belongs to the service, and
#   `self-check` says so honestly when it is absent.
# ---------------------------------------------------------------------------
$ErrorActionPreference = 'Stop'
$__canonicalVersion = & "$PSScriptRoot\Get-ProductVersion.ps1"
if (-not $Version) {
    $Version = $__canonicalVersion
} elseif ($Version -ne $__canonicalVersion) {
    throw "Requested version $Version disagrees with Cargo.toml $__canonicalVersion. The product version has ONE source: bump [workspace.package].version."
}
$Root = Split-Path $PSScriptRoot -Parent
Set-Location $Root

$target = if ($Arch -eq 'arm64') { 'aarch64-pc-windows-msvc' } else { 'x86_64-pc-windows-msvc' }
$exe = Join-Path $Root "target\release\aetherctl.exe"
if (-not (Test-Path $exe)) { throw "aetherctl.exe not built. Run: cargo build --release -p aetherctl (target $target)" }

$name  = "aetherctl-$Version-windows-$Arch"
$stage = Join-Path $Root "$OutDir\$name"
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force $stage | Out-Null
New-Item -ItemType Directory -Force (Join-Path $stage 'assets\vulndb') | Out-Null

Copy-Item $exe (Join-Path $stage 'aetherctl.exe') -Force
foreach ($f in 'vulndb.json','vulndb.manifest.json','cis_map.json') {
    Copy-Item (Join-Path $Root "assets\vulndb\$f") (Join-Path $stage "assets\vulndb\$f") -Force
}

@"
aetherctl $Version ($Arch) -- the AetherCore command surface
===========================================================

AetherCore runs entirely on this machine: no account, no sign-in, no telemetry,
no network at rest. This archive is the CLI on its own. It does not need the
desktop app, a GUI, or an interactive session.

INSTALL (one command):

  # just for you, no elevation needed
  Expand-Archive $name.zip -DestinationPath `$env:LOCALAPPDATA\AetherCLI -Force

  # or machine-wide, from an ELEVATED PowerShell
  Expand-Archive $name.zip -DestinationPath `$env:ProgramFiles\AetherCLI -Force

Then add that folder to PATH, or call the exe by full path. Nothing else runs,
nothing is registered, nothing is written outside the folder you chose.

FIRST COMMAND:

  aetherctl --output json service detect

WHAT WORKS WITHOUT THE MAINTENANCE SERVICE
  about | version | capabilities | engine-source | self-check
  service detect | service units --print
  sec audit ... | sec report | compliance summary | compliance verify
  release inspect | release verify | update offline verify

WHAT NEEDS THE SERVICE (install the full product for these)
  doctor, perf, optimize, timeline, care, insights, scan
  Without it they answer with exit code 3 and
  {"ok":false,"error":{"kind":"ServiceUnreachable",...}} -- an honest answer,
  not a hang.

self-check reports the embedded language model as absent here. That is correct:
the model ships with the full product, not with the CLI.

Run 'aetherctl --help' for the verb list, the exit-code table and the JSON
envelope contract.

UNINSTALL: delete the folder. That is all there is.
"@ | Set-Content (Join-Path $stage 'README.txt') -Encoding utf8

$zip = Join-Path $Root "$OutDir\$name.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -CompressionLevel Optimal

$hash = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
$len  = (Get-Item $zip).Length
"$hash  $len  $name.zip" | Set-Content (Join-Path $Root "$OutDir\SHA256SUMS.txt") -Encoding ascii
Write-Host "CLI archive: $zip"
Write-Host "sha256: $hash  bytes: $len"
Write-Host ""
Write-Host "NOT SIGNED by this script. Authenticode signing is a human-gated step"
Write-Host "(scripts/sign-artifacts.ps1, real certificate) and must happen before"
Write-Host "this archive is published or referenced from a winget manifest."
