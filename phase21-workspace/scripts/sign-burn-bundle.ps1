[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$BundlePath,
    [string]$CertificateThumbprint = $env:AETHERCORE_CODESIGN_THUMBPRINT,
    [ValidateSet('CurrentUser','LocalMachine')][string]$CertificateStore = 'CurrentUser',
    [string]$TimestampUrl = $env:AETHERCORE_TIMESTAMP_URL,
    [switch]$RequireSigning
)
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $Root
if ($env:OS -ne 'Windows_NT') { throw 'Burn signing is Windows-only.' }

$bundle = (Resolve-Path $BundlePath).Path
if (-not $CertificateThumbprint) {
    if ($RequireSigning) { throw 'Burn signing is required but AETHERCORE_CODESIGN_THUMBPRINT was not configured.' }
    Write-Warning 'Code-signing certificate is not configured; leaving the Burn bundle unsigned.'
    return
}
if (-not $TimestampUrl) { throw 'A timestamp URL is required whenever Burn signing is enabled.' }
if ($TimestampUrl -notmatch '^https://') { throw 'Timestamp URL must use HTTPS.' }
if (-not (Get-Command dotnet -ErrorAction SilentlyContinue)) { throw '.NET SDK is required for the pinned WiX tool.' }

& dotnet tool restore
if ($LASTEXITCODE -ne 0) { throw 'Pinned WiX tool restore failed before Burn signing.' }
$WixVersion = (& dotnet tool run wix --version | Out-String).Trim()
if ($WixVersion -notmatch '^6\.0\.2') { throw "Unexpected WiX version: $WixVersion (expected 6.0.2)" }

$work = Join-Path ([IO.Path]::GetTempPath()) ("aethercore-burn-sign-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
try {
    $engine = Join-Path $work 'burn-engine.exe'
    $reattached = Join-Path $work 'reattached-bundle.exe'

    # WiX requires Burn bundles to be signed in two pieces: the cached engine and the final bundle.
    & dotnet tool run wix burn detach $bundle -engine $engine
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path $engine)) { throw 'Failed to detach the Burn engine.' }

    & (Join-Path $PSScriptRoot 'sign-artifacts.ps1') -Path $engine `
        -CertificateThumbprint $CertificateThumbprint -CertificateStore $CertificateStore `
        -TimestampUrl $TimestampUrl -RequireSigning:$RequireSigning
    if ($LASTEXITCODE -ne 0) { throw 'Burn engine signing failed.' }

    & dotnet tool run wix burn reattach $bundle -engine $engine -o $reattached
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path $reattached)) { throw 'Failed to reattach the signed Burn engine.' }

    Move-Item -Force $reattached $bundle
    & (Join-Path $PSScriptRoot 'sign-artifacts.ps1') -Path $bundle `
        -CertificateThumbprint $CertificateThumbprint -CertificateStore $CertificateStore `
        -TimestampUrl $TimestampUrl -RequireSigning:$RequireSigning
    if ($LASTEXITCODE -ne 0) { throw 'Final Burn bundle signing failed.' }

    Write-Host "Burn engine and full bundle signed: $bundle" -ForegroundColor Green
}
finally {
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}
