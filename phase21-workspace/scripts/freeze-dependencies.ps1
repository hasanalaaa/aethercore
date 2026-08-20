[CmdletBinding()]
param(
    [switch]$Refresh,
    [switch]$VerifyOnly
)
$ErrorActionPreference = 'Stop'
$Root = Split-Path $PSScriptRoot -Parent
Set-Location $Root
$lockBaseline = Join-Path $Root 'release\dependency-locks.sha256'
$manifestBaseline = Join-Path $Root 'release\dependency-manifests.sha256'
$freezeMetadata = Join-Path $Root 'release\dependency-freeze.json'
$freezeBlocker = Join-Path $Root 'release\dependency-freeze.blocker.json'

function Hash-Line([string]$Path) {
    $resolved = (Resolve-Path $Path).Path
    $hash = (Get-FileHash $resolved -Algorithm SHA256).Hash.ToLowerInvariant()
    $relative = [IO.Path]::GetRelativePath($Root, $resolved) -replace '\\','/'
    "$hash  $relative"
}

function Current-LockLines {
    @('Cargo.lock','pnpm-lock.yaml') | ForEach-Object {
        if (-not (Test-Path $_)) { throw "Missing dependency lockfile: $_" }
        Hash-Line $_
    }
}

function Current-ManifestLines {
    $paths = @('Cargo.toml','rust-toolchain.toml','package.json','pnpm-workspace.yaml','deny.toml','.cargo/config.toml','.config/dotnet-tools.json','nuget.config')
    $paths += @(Get-ChildItem apps,crates,services -Recurse -File -Filter Cargo.toml | ForEach-Object FullName)
    $paths += @(Get-ChildItem apps -Recurse -File -Filter package.json | Where-Object { $_.FullName -notmatch '[\\/]node_modules[\\/]' } | ForEach-Object FullName)
    $paths | Where-Object { Test-Path $_ } | ForEach-Object { (Resolve-Path $_).Path } | Sort-Object -Unique | ForEach-Object { Hash-Line $_ }
}

function Assert-Lines([string]$Baseline, [object[]]$Actual, [string]$Label) {
    if (-not (Test-Path $Baseline)) { throw "$Label baseline is missing: $Baseline" }
    $expected = @(Get-Content $Baseline | Where-Object { $_.Trim() })
    if (($expected -join "`n") -ne ($Actual -join "`n")) {
        throw "$Label hashes differ from the approved freeze baseline. Review dependency-manifest/lock changes and refresh only on the trusted dependency-freeze workstation."
    }
}

if ($VerifyOnly) {
    if (Test-Path $freezeBlocker) { throw 'Dependency freeze release blocker is still present. Refresh on the trusted workstation before verification can pass.' }
    Assert-Lines $lockBaseline @(Current-LockLines) 'Dependency lock'
    Assert-Lines $manifestBaseline @(Current-ManifestLines) 'Dependency manifest'
    if (-not (Test-Path $freezeMetadata)) { throw 'Dependency freeze metadata is missing.' }

    $metadata = Get-Content $freezeMetadata -Raw | ConvertFrom-Json
    if ($metadata.schema -ne 'aethercore.dependency-freeze.v1') { throw 'Unsupported dependency freeze metadata schema.' }
    $rustToolchain = (Get-Content 'rust-toolchain.toml' -Raw | Select-String -Pattern 'channel\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
    if ($metadata.rust_toolchain -ne $rustToolchain) { throw 'Dependency freeze Rust toolchain does not match rust-toolchain.toml.' }
    $packageManager = (Get-Content 'package.json' -Raw | ConvertFrom-Json).packageManager
    if ($packageManager -notmatch '^pnpm@(.+)$') { throw 'Root package.json must pin an exact pnpm packageManager.' }
    $expectedPnpm = $Matches[1]
    if ($metadata.pnpm -ne $expectedPnpm) { throw 'Dependency freeze pnpm version does not match root package.json.' }
    $currentPnpm = (& pnpm --version | Out-String).Trim()
    if ($LASTEXITCODE -ne 0 -or $currentPnpm -ne $expectedPnpm) { throw "Expected pnpm $expectedPnpm but found '$currentPnpm'." }

    $cargoLockHash = (Get-FileHash 'Cargo.lock' -Algorithm SHA256).Hash.ToLowerInvariant()
    $pnpmLockHash = (Get-FileHash 'pnpm-lock.yaml' -Algorithm SHA256).Hash.ToLowerInvariant()
    $manifestBaselineHash = (Get-FileHash $manifestBaseline -Algorithm SHA256).Hash.ToLowerInvariant()
    $lockBaselineHash = (Get-FileHash $lockBaseline -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($metadata.cargo_lock_sha256 -ne $cargoLockHash -or $metadata.pnpm_lock_sha256 -ne $pnpmLockHash) {
        throw 'Dependency freeze metadata does not match the approved lockfile bytes.'
    }
    if ($metadata.manifest_baseline_sha256 -ne $manifestBaselineHash -or $metadata.lock_baseline_sha256 -ne $lockBaselineHash) {
        throw 'Dependency freeze metadata does not match the approved baseline files.'
    }

    & cargo metadata --locked --format-version 1 *> $null
    if ($LASTEXITCODE -ne 0) { throw 'Cargo.lock does not resolve under --locked.' }
    Write-Host 'Dependency locks, manifests, tool pins, and freeze metadata match the approved Phase 9 baseline.' -ForegroundColor Green
    return
}

$freezeExists = (Test-Path 'Cargo.lock') -and (Test-Path 'pnpm-lock.yaml') -and (Test-Path $lockBaseline) -and (Test-Path $manifestBaseline) -and (Test-Path $freezeMetadata)
if ($freezeExists -and -not $Refresh) {
    & $PSCommandPath -VerifyOnly
    return
}
if (-not $Refresh) {
    throw 'Phase 9 dependency freeze is incomplete. Run freeze-dependencies.ps1 -Refresh only on the trusted dependency-freeze workstation after reviewing dependency changes.'
}

Write-Host 'Resolving dependency lockfiles from pinned manifests...' -ForegroundColor Cyan
& cargo generate-lockfile
if ($LASTEXITCODE -ne 0) { throw 'Cargo lockfile generation failed.' }
& pnpm --dir apps/ui install --lockfile-only
if ($LASTEXITCODE -ne 0) { throw 'pnpm lockfile generation failed.' }
& cargo metadata --locked --format-version 1 *> $null
if ($LASTEXITCODE -ne 0) { throw 'Generated Cargo.lock does not resolve under --locked.' }

New-Item -ItemType Directory -Force (Split-Path $lockBaseline -Parent) | Out-Null
@(Current-LockLines) | Set-Content $lockBaseline -Encoding ascii
@(Current-ManifestLines) | Set-Content $manifestBaseline -Encoding ascii

$rustToolchain = (Get-Content 'rust-toolchain.toml' -Raw | Select-String -Pattern 'channel\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
$packageManager = (Get-Content 'package.json' -Raw | ConvertFrom-Json).packageManager
if ($packageManager -notmatch '^pnpm@(.+)$') { throw 'Root package.json must pin an exact pnpm packageManager.' }
$expectedPnpm = $Matches[1]
$pnpmVersion = (& pnpm --version | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $pnpmVersion -ne $expectedPnpm) { throw "Dependency freeze must run with pnpm $expectedPnpm; found '$pnpmVersion'." }
$metadata = [ordered]@{
    schema = 'aethercore.dependency-freeze.v1'
    rust_toolchain = $rustToolchain
    pnpm = $pnpmVersion
    cargo_lock_sha256 = (Get-FileHash 'Cargo.lock' -Algorithm SHA256).Hash.ToLowerInvariant()
    pnpm_lock_sha256 = (Get-FileHash 'pnpm-lock.yaml' -Algorithm SHA256).Hash.ToLowerInvariant()
    manifest_baseline_sha256 = $null
    lock_baseline_sha256 = $null
}
$metadata.manifest_baseline_sha256 = (Get-FileHash $manifestBaseline -Algorithm SHA256).Hash.ToLowerInvariant()
$metadata.lock_baseline_sha256 = (Get-FileHash $lockBaseline -Algorithm SHA256).Hash.ToLowerInvariant()
$metadata | ConvertTo-Json -Depth 3 | Set-Content $freezeMetadata -Encoding utf8
if (Test-Path $freezeBlocker) { Remove-Item -LiteralPath $freezeBlocker -Force }
Write-Host 'Phase 9 dependency graph frozen. Review and commit lockfiles plus all release/dependency-* baseline files together.' -ForegroundColor Green
