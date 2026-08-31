[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$PayloadDir,
    [Parameter(Mandatory=$true)][ValidatePattern('^\d+\.\d+\.\d+$')][string]$Version,
    [string]$MsiOut = 'out\release\AetherCore.msi',
    [string]$BundleOut = 'out\release\AetherCoreSetup.exe',
    [string]$WebView2Bootstrapper = 'out\prereqs\MicrosoftEdgeWebview2Setup.exe',
    [switch]$MsiOnly,
    [switch]$BundleOnly
)
$ErrorActionPreference = 'Stop'
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
    'UNINSTALL.txt'
)
# P37 Stage 1: the assets tree is packaged straight from the source tree (the embedded
# model alone is 1.07 GB; copying it into a payload dir per build buys nothing).
$assets = Join-Path $Root 'assets'
foreach ($rel in @('models\qwen2.5-1.5b-instruct-q4_k_m.gguf','models\models.manifest.json','models\licenses\Apache-2.0.txt','models\licenses\Qwen-GGUF-NOTICE.txt','vulndb\vulndb.json','vulndb\vulndb.manifest.json','vulndb\cis_map.json')) {
    if (-not (Test-Path (Join-Path $assets $rel))) { throw "Missing installer asset: assets\$rel" }
}
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

$msi = Join-Path $Root $MsiOut
$bundle = Join-Path $Root $BundleOut
New-Item -ItemType Directory -Force (Split-Path $msi -Parent) | Out-Null
New-Item -ItemType Directory -Force (Split-Path $bundle -Parent) | Out-Null
$productCode = Deterministic-ProductCode "AetherCore/$Version/x64"

if (-not $BundleOnly) {
    & dotnet tool run wix build installer\wix\Product.wxs -arch x64 -ext WixToolset.Util.wixext -o $msi `
        -d "PayloadDir=$Payload" -d "AssetsDir=$assets" -d "ProductVersion=$Version" -d "ProductCode=$productCode"
    if ($LASTEXITCODE -ne 0) { throw 'AetherCore MSI build failed.' }
    & dotnet tool run wix msi validate $msi
    if ($LASTEXITCODE -ne 0) { throw 'WiX MSI validation failed.' }
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
