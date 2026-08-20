[CmdletBinding()]
param([switch]$InstallPrerequisites,[switch]$RefreshDependencyFreeze)
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)

function Has($cmd) { return [bool](Get-Command $cmd -ErrorAction SilentlyContinue) }
function Install-Winget($id, $extra = @()) {
    if (-not (Has winget)) { throw 'winget is required for automated prerequisite installation.' }
    Write-Host "Installing $id..." -ForegroundColor Cyan
    & winget install --id $id --exact --accept-source-agreements --accept-package-agreements @extra
    if ($LASTEXITCODE -ne 0) { throw "winget failed installing $id" }
}
function Has-VCTools($vswhere) {
    if (-not (Test-Path $vswhere)) { return $false }
    $path = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    return [bool]$path
}
function Has-WebView2 {
    $keys = @(
        'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
        'HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
    )
    foreach ($key in $keys) {
        $runtime = Get-ItemProperty $key -ErrorAction SilentlyContinue
        if ($runtime -and $runtime.pv -and $runtime.pv -ne '0.0.0.0') { return $true }
    }
    return $false
}

if ($env:OS -ne 'Windows_NT') { throw 'AetherCore must be built on Windows.' }
$os = [Environment]::OSVersion.Version
if ($os.Build -lt 22621) { throw "AetherCore targets Windows 11 22H2 (build 22621) or newer. Detected build $($os.Build)." }
$arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
if ($arch -ne 'X64') { throw "AetherCore currently targets x64 Windows. Detected architecture: $arch." }

$missing = @()
if (-not (Has rustup)) { $missing += 'Rust' }
if (-not (Has node)) { $missing += 'Node.js' }
if (-not (Has pnpm)) { $missing += 'pnpm' }
if (-not (Has dotnet)) { $missing += '.NET SDK (WiX build tool host)' }
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (-not (Has-VCTools $vswhere)) { $missing += 'Visual C++ Build Tools' }
if (-not (Has-WebView2)) { $missing += 'WebView2 Runtime' }

if ($missing.Count -and -not $InstallPrerequisites) {
    Write-Host "Missing prerequisites: $($missing -join ', ')" -ForegroundColor Yellow
    Write-Host 'Re-run with -InstallPrerequisites to install supported dependencies.'
    exit 2
}

if ($InstallPrerequisites) {
    if (-not (Has rustup)) { Install-Winget 'Rustlang.Rustup' }
    if (-not (Has node)) { Install-Winget 'OpenJS.NodeJS.LTS' }
    if (-not (Has dotnet)) { Install-Winget 'Microsoft.DotNet.SDK.8' }
    if (-not (Has-VCTools $vswhere)) {
        Install-Winget 'Microsoft.VisualStudio.2022.BuildTools' @('--override', '--wait --quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended')
    }
    if (-not (Has-WebView2)) { Install-Winget 'Microsoft.EdgeWebView2Runtime' }
    $env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')
    if (-not (Has pnpm)) {
        & npm install --global pnpm@11.22.0
        if ($LASTEXITCODE -ne 0) { throw 'pnpm install failed' }
    }
}

$freezeEvidence = @('Cargo.lock','pnpm-lock.yaml','release\dependency-locks.sha256','release\dependency-manifests.sha256','release\dependency-freeze.json')
$freezeComplete = ($freezeEvidence | Where-Object { -not (Test-Path $_) }).Count -eq 0
if (-not $freezeComplete) {
    if (-not $RefreshDependencyFreeze) {
        throw 'Approved dependency freeze is absent. Review manifests, then re-run bootstrap.ps1 -RefreshDependencyFreeze on the trusted freeze workstation.'
    }
    Write-Host 'Explicitly refreshing the reviewed dependency freeze...' -ForegroundColor Yellow
    & .\scripts\freeze-dependencies.ps1 -Refresh
    if ($LASTEXITCODE -ne 0) { throw 'Dependency freeze refresh failed' }
} else {
    if ($RefreshDependencyFreeze) {
        & .\scripts\freeze-dependencies.ps1 -Refresh
    } else {
        & .\scripts\freeze-dependencies.ps1 -VerifyOnly
    }
    if ($LASTEXITCODE -ne 0) { throw 'Dependency freeze verification/refresh failed' }
}

& rustup toolchain install 1.97.1 --profile minimal --component rustfmt,clippy
if ($LASTEXITCODE -ne 0) { throw 'Rust toolchain installation failed' }
& rustup override set 1.97.1
if ($LASTEXITCODE -ne 0) { throw 'Rust toolchain override failed' }
& pnpm --dir apps/ui install --frozen-lockfile
if ($LASTEXITCODE -ne 0) { throw 'Frozen UI dependency installation failed' }
& pnpm --dir apps/ui check
if ($LASTEXITCODE -ne 0) { throw 'UI accessibility/type gate failed' }
& pnpm --dir apps/ui build
if ($LASTEXITCODE -ne 0) { throw 'UI build failed' }
& cargo build --workspace --locked
if ($LASTEXITCODE -ne 0) { throw 'Locked Cargo workspace build failed' }
& dotnet tool restore
if ($LASTEXITCODE -ne 0) { throw 'Pinned WiX tool restore failed' }
Write-Host 'AetherCore Phase 0-16 bootstrap prerequisites complete.' -ForegroundColor Green
Write-Host 'Run .\scripts\verify-phase16.ps1 for the current source/native production qualification gate.'
Write-Host 'Run .\scripts\run-dev.ps1 for local development.'
