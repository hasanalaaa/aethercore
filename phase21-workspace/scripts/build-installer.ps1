[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$PayloadDir,
    [ValidatePattern('^\d+\.\d+\.\d+$')][string]$Version,
    [string]$MsiOut = 'out\release\AetherCore.msi',
    [string]$BundleOut = 'out\release\AetherCoreSetup.exe',
    [string]$WebView2Bootstrapper = 'out\prereqs\MicrosoftEdgeWebview2Setup.exe',
    [switch]$MsiOnly,
    [switch]$BundleOnly
)
$ErrorActionPreference = 'Stop'
$__canonicalVersion = & "$PSScriptRoot\Get-ProductVersion.ps1"
if (-not $Version) {
    $Version = $__canonicalVersion
} elseif ($Version -ne $__canonicalVersion) {
    throw "Requested version $Version disagrees with Cargo.toml $__canonicalVersion. The product version has ONE source: bump [workspace.package].version."
}
$Root = Split-Path $PSScriptRoot -Parent
Set-Location $Root
if ($env:OS -ne 'Windows_NT') { throw 'WiX packaging must run on Windows.' }
if ($MsiOnly -and $BundleOnly) { throw 'MsiOnly and BundleOnly are mutually exclusive.' }
if (-not (Get-Command dotnet -ErrorAction SilentlyContinue)) { throw '.NET SDK is required for the pinned WiX .NET tool.' }

$Payload = (Resolve-Path $PayloadDir).Path
$required = @(
    'aethercore-desktop.exe',
    'aethercore-maintenance-service.exe',
    'aethercore-consent-broker.exe',
    'aethercore-update-broker.exe',
    'aethercore-install-hardener.exe',
    'aetherctl.exe',
    'update-trust.json',
    'UNINSTALL.txt',
    # P41: the x64 MSVC build imports VCOMP140.DLL (measured with llvm-readobj
    # coff-imports), where the ARM64 clang-cl build imports libomp140.aarch64.dll.
    # Product.wxs selects the right one on $(sys.BUILDARCH); this pipeline is x64.
    # Sourced and hash-verified explicitly below (DBT-P42-012) — this file only
    # asserts the final name is present in the payload once that has run.
    'vcomp140.dll'
)
# P37 Stage 1: the assets tree is packaged straight from the source tree (the embedded
# model alone is 1.07 GB; copying it into a payload dir per build buys nothing).
$assets = Join-Path $Root 'assets'
foreach ($rel in @('models\qwen2.5-1.5b-instruct-q4_k_m.gguf','models\models.manifest.json','models\licenses\Apache-2.0.txt','models\licenses\Qwen-GGUF-NOTICE.txt','vulndb\vulndb.json','vulndb\vulndb.manifest.json','vulndb\cis_map.json')) {
    if (-not (Test-Path (Join-Path $assets $rel))) { throw "Missing installer asset: assets\$rel" }
}

# DBT-P42-012: this script used to require vcomp140.dll in the payload without
# saying where it should come from. §42.5 measured five files of that name on
# the build machine and found the first plausible match — the onecore variant,
# VC\Redist\MSVC\<ver>\onecore\x64\Microsoft.VC143.OpenMP\vcomp140.dll,
# 72,712 bytes — is the WRONG one; it was caught only by hashing against a
# previously-recorded baseline before building, not by anything in this script.
# The correct file is the desktop x64 MSVC OpenMP redist: 193,152 bytes,
# sha256 55aba23cdcd6484fbb06f4155b8ca75adfce7a881f10afd0c49457165e677164
# (measured and installed under Gate 2, §42.5 / §41.14). Source it explicitly
# and verify the hash — a comment naming the right file is not a check; this is.
$VcompExpectedBytes = 193152
$VcompExpectedSha256 = '55aba23cdcd6484fbb06f4155b8ca75adfce7a881f10afd0c49457165e677164'

function Resolve-VcompSource {
    # Preferred: the standard MSVC toolchain environment variable set by
    # vcvarsall.bat / a Visual Studio Developer shell — portable across
    # machines and VS installs, unlike a hardcoded path.
    if ($env:VCToolsRedistDir) {
        $candidate = Join-Path $env:VCToolsRedistDir 'x64\Microsoft.VC143.OpenMP\vcomp140.dll'
        if (Test-Path $candidate) { return (Resolve-Path $candidate).Path }
    }
    # Fallback: this project's pinned toolchain checkout, the exact location
    # §42.5 measured the correct file at on the release-build machine.
    $pinned = 'C:\AetherCore-P36\toolchain\vs2022\VC\Redist\MSVC\14.44.35112\x64\Microsoft.VC143.OpenMP\vcomp140.dll'
    if (Test-Path $pinned) { return (Resolve-Path $pinned).Path }
    throw ("vcomp140.dll source not found. Checked `$env:VCToolsRedistDir\x64\Microsoft.VC143.OpenMP\vcomp140.dll " +
        "(env var " + $(if ($env:VCToolsRedistDir) { "set to $env:VCToolsRedistDir but the file is not there" } else { "not set" }) + ") " +
        "and the pinned fallback $pinned. Run from a VS Developer shell (sets VCToolsRedistDir) or update the " +
        "pinned path in this script to match the current toolchain checkout.")
}

function Assert-VcompHash([string]$Path) {
    $bytes = (Get-Item $Path).Length
    $sha256 = (Get-FileHash -Algorithm SHA256 $Path).Hash.ToLowerInvariant()
    if ($bytes -ne $VcompExpectedBytes -or $sha256 -ne $VcompExpectedSha256) {
        throw ("vcomp140.dll at $Path is NOT the expected desktop x64 MSVC OpenMP redist " +
            "(expected $VcompExpectedBytes bytes / sha256 $VcompExpectedSha256; " +
            "got $bytes bytes / sha256 $sha256). DBT-P42-012: this machine has carried five files of " +
            "this name (onecore x64/x86, desktop x64/x86, System32) and the first plausible match is wrong. " +
            "Do not suppress this check or substitute a different file.")
    }
}

$vcompTarget = Join-Path $Payload 'vcomp140.dll'
if (-not (Test-Path $vcompTarget)) {
    $vcompSource = Resolve-VcompSource
    Copy-Item -Path $vcompSource -Destination $vcompTarget -Force
    Write-Host "vcomp140.dll staged from $vcompSource" -ForegroundColor Yellow
}
Assert-VcompHash $vcompTarget

foreach ($name in $required) {
    if (-not (Test-Path (Join-Path $Payload $name))) { throw "Missing installer payload: $name" }
}

function Deterministic-ProductCode([string]$input) {
    $namespace = [Text.Encoding]::UTF8.GetBytes('AetherCore/MSI/ProductCode/v1')
    $value = [Text.Encoding]::UTF8.GetBytes($input)
    $sha = [Security.Cryptography.SHA256]::Create()
    try { $hash = $sha.ComputeHash($namespace + $value) } finally { $sha.Dispose() }
    $bytes = [byte[]]$hash[0..15]
    # Windows Installer requires a GUID, not a UUID namespace scheme. The first 128 SHA-256 bits
    # give us a stable per-version ProductCode without pretending to implement RFC UUIDv5 byte order.
    return ([Guid]::new($bytes)).ToString('B').ToUpperInvariant()
}

& dotnet tool restore
if ($LASTEXITCODE -ne 0) { throw 'Pinned WiX tool restore failed.' }
$WixVersion = (& dotnet tool run wix --version | Out-String).Trim()
if ($WixVersion -notmatch '^6\.0\.2') { throw "Unexpected WiX version: $WixVersion (expected 6.0.2)" }

# Pin extension versions to the same WiX servicing release; never float during release builds.
& dotnet tool run wix extension add WixToolset.Util.wixext/6.0.2
if ($LASTEXITCODE -ne 0) { throw 'WiX Util extension restore failed.' }
& dotnet tool run wix extension add WixToolset.BootstrapperApplications.wixext/6.0.2
if ($LASTEXITCODE -ne 0) { throw 'WiX BootstrapperApplications extension restore failed.' }

# The ARP icon (P47 item 3). Generated by tools/icon-pipeline/build-icons.mjs
# from one SVG; the artwork is provisional and is an owner decision (DBT-P36-004).
$icon = Join-Path $Root 'apps\desktop\icons\icon.ico'
if (-not (Test-Path $icon)) { throw "Product icon not found: $icon. Run: node tools/icon-pipeline/build-icons.mjs" }

$msi = Join-Path $Root $MsiOut
$bundle = Join-Path $Root $BundleOut
New-Item -ItemType Directory -Force (Split-Path $msi -Parent) | Out-Null
New-Item -ItemType Directory -Force (Split-Path $bundle -Parent) | Out-Null
$productCode = Deterministic-ProductCode "AetherCore/$Version/x64"

if (-not $BundleOnly) {
    & dotnet tool run wix build installer\wix\Product.wxs -arch x64 -o $msi `
        -d "PayloadDir=$Payload" -d "AssetsDir=$assets" -d "IconFile=$icon" -d "ProductVersion=$Version" -d "ProductCode=$productCode"
    if ($LASTEXITCODE -ne 0) { throw 'AetherCore MSI build failed.' }
    & dotnet tool run wix msi validate $msi
    if ($LASTEXITCODE -ne 0) { throw 'WiX MSI validation failed.' }
    # Payload check: every file in the package must be authored in Product.wxs.
    # See scripts/check-msi-payload.ps1 and DBT-P36-001 / P36-008 / P40-001.
    & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'check-msi-payload.ps1') -Msi $msi -Wxs (Join-Path $Root 'installer\wix\Product.wxs')
    if ($LASTEXITCODE -ne 0) { throw "MSI payload check failed (exit $LASTEXITCODE)" }
    Write-Host "MSI built: $msi" -ForegroundColor Green
}

if (-not $MsiOnly) {
    if (-not (Test-Path $msi)) { throw "Bundle requires an existing MSI: $msi" }
    $webview = (Resolve-Path $WebView2Bootstrapper).Path
    & dotnet tool run wix build installer\wix\Bundle.wxs -arch x64 `
        -ext WixToolset.Util.wixext -ext WixToolset.BootstrapperApplications.wixext `
        -o $bundle -d "ProductVersion=$Version" -d "MsiPath=$msi" -d "WebView2Bootstrapper=$webview"
    if ($LASTEXITCODE -ne 0) { throw 'AetherCore bootstrapper bundle build failed.' }
    Write-Host "Bundle built: $bundle" -ForegroundColor Green
}
