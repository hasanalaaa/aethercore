[CmdletBinding()]
param(
    [ValidatePattern('^\d+\.\d+\.\d+$')][string]$Version,
    [switch]$RequireSigning,
    [ValidateSet('CurrentUser','LocalMachine')][string]$CertificateStore = 'CurrentUser',
    [switch]$SkipOnlineSupplyChain,
    [string]$UpdateTrustPath
)
$ErrorActionPreference = 'Stop'
$Root = Split-Path $PSScriptRoot -Parent
Set-Location $Root
if ($env:OS -ne 'Windows_NT') { throw 'AetherCore production release builds must run on Windows.' }

if (-not $Version) {
    $cargoToml = Get-Content 'Cargo.toml' -Raw
    if ($cargoToml -notmatch '(?ms)\[workspace\.package\].*?version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"') {
        throw 'Unable to read workspace package version.'
    }
    $Version = $Matches[1]
}
if (-not (Test-Path 'Cargo.lock') -or -not (Test-Path 'pnpm-lock.yaml') -or -not (Test-Path 'release\dependency-locks.sha256') -or -not (Test-Path 'release\dependency-manifests.sha256') -or -not (Test-Path 'release\dependency-freeze.json')) {
    throw 'Release requires approved lockfiles plus release/dependency-locks.sha256, release/dependency-manifests.sha256, and release/dependency-freeze.json. Run freeze-dependencies.ps1 on the trusted freeze workstation first.'
}
& "$PSScriptRoot\freeze-dependencies.ps1" -VerifyOnly
if ($LASTEXITCODE -ne 0) { throw 'Dependency lock baseline verification failed.' }

if (-not $env:SOURCE_DATE_EPOCH) {
    try {
        $epoch = (& git log -1 --format=%ct 2>$null | Out-String).Trim()
        if ($LASTEXITCODE -eq 0 -and $epoch -match '^\d+$') { $env:SOURCE_DATE_EPOCH = $epoch }
    } catch {}
}
if (-not $env:SOURCE_DATE_EPOCH) {
    $epochFile = Join-Path $Root 'release\source-date-epoch.txt'
    if (-not (Test-Path $epochFile)) { throw 'SOURCE_DATE_EPOCH is required.' }
    $env:SOURCE_DATE_EPOCH = (Get-Content $epochFile -Raw).Trim()
}
if ($env:SOURCE_DATE_EPOCH -notmatch '^\d{9,12}$') { throw 'SOURCE_DATE_EPOCH is malformed.' }
$env:CARGO_INCREMENTAL = '0'
$env:TAURI_SIGNING_PRIVATE_KEY = ''

$ReleaseRoot = Join-Path $Root "out\release\$Version"
$Payload = Join-Path $ReleaseRoot 'payload'
$Artifacts = Join-Path $ReleaseRoot 'artifacts'
$Evidence = Join-Path $ReleaseRoot 'evidence'
$Prereqs = Join-Path $ReleaseRoot 'prereqs'
Remove-Item $ReleaseRoot -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $Payload,$Artifacts,$Evidence,$Prereqs | Out-Null

& pnpm --dir apps/ui install --frozen-lockfile
if ($LASTEXITCODE -ne 0) { throw 'Frozen UI dependency restore failed.' }
& pnpm --dir apps/ui check
if ($LASTEXITCODE -ne 0) { throw 'Svelte/TypeScript validation failed.' }
& pnpm --dir apps/ui build
if ($LASTEXITCODE -ne 0) { throw 'UI production build failed.' }

& cargo build --locked --release -p aethercore-maintenance-service -p aethercore-consent-broker -p aethercore-update-broker -p aethercore-install-hardener -p aetherctl
if ($LASTEXITCODE -ne 0) { throw 'Native privileged component build failed.' }
Push-Location (Join-Path $Root 'apps\desktop')
try {
    $tauri = Join-Path $Root 'apps\ui\node_modules\.bin\tauri.cmd'
    & $tauri build --no-bundle
    if ($LASTEXITCODE -ne 0) { throw 'Desktop Tauri build failed.' }
} finally { Pop-Location }

foreach ($name in @('aethercore-desktop.exe','aethercore-maintenance-service.exe','aethercore-consent-broker.exe','aethercore-update-broker.exe','aethercore-install-hardener.exe','aetherctl.exe')) {
    Copy-Item (Join-Path $Root "target\release\$name") (Join-Path $Payload $name) -Force
}

if ($RequireSigning -and -not $UpdateTrustPath) { throw 'Signed Phase 15 release packaging requires -UpdateTrustPath (or the protected AETHERCORE_UPDATE_TRUST_PATH passed by verify-phase15.ps1).' }
$trustSource = if ($UpdateTrustPath) { (Resolve-Path $UpdateTrustPath).Path } else { Join-Path $Root 'release\update-trust.template.json' }
& "$PSScriptRoot\validate-update-trust.ps1" -Path $trustSource -RequireEnabled:$RequireSigning
if ($LASTEXITCODE -ne 0) { throw 'Update trust validation failed.' }
Copy-Item $trustSource (Join-Path $Payload 'update-trust.json') -Force
# P37 Stage 2: the plain-language uninstall statement, installed beside the product.
Copy-Item (Join-Path $Root 'release\UNINSTALL.txt') (Join-Path $Payload 'UNINSTALL.txt') -Force

& "$PSScriptRoot\verify-pe-hardening.ps1" -Path (Get-ChildItem $Payload -Filter '*.exe' | Select-Object -ExpandProperty FullName)
if ($LASTEXITCODE -ne 0) { throw 'PE hardening verification failed.' }

if (-not $SkipOnlineSupplyChain) {
    & "$PSScriptRoot\audit-dependencies.ps1" -OutputDirectory ([IO.Path]::GetRelativePath($Root,(Join-Path $Evidence 'supply-chain')))
    if ($LASTEXITCODE -ne 0) { throw 'Dependency audit failed.' }
}
& "$PSScriptRoot\generate-sbom.ps1" -OutputDirectory ([IO.Path]::GetRelativePath($Root,(Join-Path $Evidence 'sbom')))
if ($LASTEXITCODE -ne 0) { throw 'SBOM generation failed.' }
& "$PSScriptRoot\verify-reproducible.ps1" -OutputPath ([IO.Path]::GetRelativePath($Root,(Join-Path $Evidence 'reproducibility.json')))
if ($LASTEXITCODE -ne 0) { throw 'Reproducibility control verification failed.' }

& "$PSScriptRoot\sign-artifacts.ps1" -Path (Get-ChildItem $Payload -Filter '*.exe' | Select-Object -ExpandProperty FullName) -CertificateStore $CertificateStore -RequireSigning:$RequireSigning
if ($LASTEXITCODE -ne 0) { throw 'Payload signing failed.' }

$webview = Join-Path $Prereqs 'MicrosoftEdgeWebview2Setup.exe'
& "$PSScriptRoot\fetch-webview2.ps1" -OutputPath ([IO.Path]::GetRelativePath($Root,$webview))
if ($LASTEXITCODE -ne 0) { throw 'WebView2 prerequisite acquisition failed.' }

$msi = Join-Path $Artifacts "AetherCore-$Version-x64.msi"
$bundle = Join-Path $Artifacts "AetherCoreSetup-$Version-x64.exe"
& "$PSScriptRoot\build-installer.ps1" -PayloadDir $Payload -Version $Version -MsiOut ([IO.Path]::GetRelativePath($Root,$msi)) -BundleOut ([IO.Path]::GetRelativePath($Root,$bundle)) -WebView2Bootstrapper ([IO.Path]::GetRelativePath($Root,$webview)) -MsiOnly
if ($LASTEXITCODE -ne 0) { throw 'MSI packaging failed.' }
& "$PSScriptRoot\sign-artifacts.ps1" -Path $msi -CertificateStore $CertificateStore -RequireSigning:$RequireSigning
if ($LASTEXITCODE -ne 0) { throw 'MSI signing failed.' }

& "$PSScriptRoot\build-installer.ps1" -PayloadDir $Payload -Version $Version -MsiOut ([IO.Path]::GetRelativePath($Root,$msi)) -BundleOut ([IO.Path]::GetRelativePath($Root,$bundle)) -WebView2Bootstrapper ([IO.Path]::GetRelativePath($Root,$webview)) -BundleOnly
if ($LASTEXITCODE -ne 0) { throw 'Burn bundle packaging failed.' }
& "$PSScriptRoot\sign-burn-bundle.ps1" -BundlePath $bundle -CertificateStore $CertificateStore -RequireSigning:$RequireSigning
if ($LASTEXITCODE -ne 0) { throw 'Burn engine/final bundle signing failed.' }

$sourceCommit = $null
try {
    $candidate = (& git rev-parse HEAD 2>$null | Out-String).Trim()
    if ($LASTEXITCODE -eq 0 -and $candidate -match '^[0-9a-fA-F]{40,64}$') { $sourceCommit = $candidate.ToLowerInvariant() }
} catch {}
$webviewHash = (Get-FileHash $webview -Algorithm SHA256).Hash.ToLowerInvariant()
$signerSubject = $null
$signerThumbprint = $null
if ($RequireSigning) {
    $bundleSig = Get-AuthenticodeSignature $bundle
    if ($bundleSig.Status -ne 'Valid' -or -not $bundleSig.SignerCertificate) { throw 'Signed release bundle failed final Authenticode inspection.' }
    $signerSubject = $bundleSig.SignerCertificate.Subject
    $signerThumbprint = $bundleSig.SignerCertificate.Thumbprint
}

$metadata = [ordered]@{
    schema = 'aethercore.release.v1'
    version = $Version
    architecture = 'x64'
    minimum_windows_build = 22621
    source_date_epoch = [int64]$env:SOURCE_DATE_EPOCH
    source_commit = $sourceCommit
    protocol_version = 7
    signing_required = [bool]$RequireSigning
    signer_subject = $signerSubject
    signer_thumbprint = $signerThumbprint
    wix = '6.0.2'
    cargo_cyclonedx = '0.5.9'
    cargo_deny = '0.20.2'
    pnpm = '11.22.0'
    webview2_bootstrapper_sha256 = $webviewHash
    msi_byte_reproducible_claim = $false
}
$metadataPath = Join-Path $ReleaseRoot 'RELEASE-METADATA.json'
$metadata | ConvertTo-Json -Depth 4 | Set-Content $metadataPath -Encoding utf8

$hashFile = Join-Path $ReleaseRoot 'SHA256SUMS.txt'
$metadataHash = (Get-FileHash $metadataPath -Algorithm SHA256).Hash.ToLowerInvariant()
$hashLines = @(
    @($Payload,$Artifacts,$Evidence,$Prereqs) | ForEach-Object {
        Get-ChildItem $_ -Recurse -File | Sort-Object FullName | ForEach-Object {
            $hash = (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
            $rel = [IO.Path]::GetRelativePath($ReleaseRoot,$_.FullName) -replace '\\','/'
            "$hash  $rel"
        }
    }
    "$metadataHash  RELEASE-METADATA.json"
)
$hashLines | Set-Content $hashFile -Encoding ascii
Write-Host "AetherCore production release candidate built: $ReleaseRoot" -ForegroundColor Green
