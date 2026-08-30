[CmdletBinding()]
param(
    [switch]$NativeDoubleBuild,
    [string]$OutputPath = 'out\reproducibility-report.json'
)
$ErrorActionPreference = 'Stop'
$Root = Split-Path $PSScriptRoot -Parent
Set-Location $Root

function Require-Text($file, $pattern, $message) {
    if (-not (Test-Path $file)) { throw "Missing required file: $file" }
    $text = Get-Content $file -Raw
    if ($text -notmatch $pattern) { throw $message }
}

if (-not $env:SOURCE_DATE_EPOCH) { throw 'SOURCE_DATE_EPOCH must be set.' }
if ($env:SOURCE_DATE_EPOCH -notmatch '^\d{9,12}$') { throw 'SOURCE_DATE_EPOCH is malformed.' }
Require-Text 'Cargo.toml' 'codegen-units\s*=\s*1' 'Rust release profile must use one codegen unit.'
Require-Text 'Cargo.toml' 'incremental\s*=\s*false' 'Rust release profile must disable incremental compilation.'
Require-Text '.cargo/config.toml' '/Brepro' 'MSVC release linker must receive /Brepro.'
Require-Text '.config/dotnet-tools.json' '"version"\s*:\s*"6\.0\.2"' 'WiX must be pinned to 6.0.2.'
if (-not (Test-Path 'Cargo.lock') -or -not (Test-Path 'pnpm-lock.yaml')) { throw 'Both dependency lockfiles are required.' }

$inputs = @(
    'Cargo.lock','pnpm-lock.yaml','Cargo.toml','.cargo/config.toml','.config/dotnet-tools.json',
    'nuget.config','deny.toml','installer/wix/Product.wxs','installer/wix/Bundle.wxs','release/source-date-epoch.txt'
)
$inputHashes = @{}
foreach ($file in $inputs) { $inputHashes[$file] = (Get-FileHash $file -Algorithm SHA256).Hash.ToLowerInvariant() }

$native = [ordered]@{ attempted = $false; equal = $null; files = @{} }
if ($NativeDoubleBuild) {
    if ($env:OS -ne 'Windows_NT') { throw 'Native reproducibility double-build requires Windows.' }
    $native.attempted = $true
    $targets = @(
        'aethercore-maintenance-service.exe',
        'aethercore-consent-broker.exe',
        'aethercore-install-hardener.exe'
    )
    $dirs = @('out\repro\a','out\repro\b')
    foreach ($dir in $dirs) {
        Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue
        $env:CARGO_TARGET_DIR = Join-Path $Root $dir
        & cargo build --locked --release -p aethercore-maintenance-service -p aethercore-consent-broker -p aethercore-install-hardener
        if ($LASTEXITCODE -ne 0) { throw "Native reproducibility build failed: $dir" }
    }
    Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
    $allEqual = $true
    foreach ($name in $targets) {
        $a = Join-Path $Root "out\repro\a\release\$name"
        $b = Join-Path $Root "out\repro\b\release\$name"
        $ha = (Get-FileHash $a -Algorithm SHA256).Hash.ToLowerInvariant()
        $hb = (Get-FileHash $b -Algorithm SHA256).Hash.ToLowerInvariant()
        $same = $ha -eq $hb
        $native.files[$name] = @{ first = $ha; second = $hb; equal = $same }
        if (-not $same) { $allEqual = $false }
    }
    $native.equal = $allEqual
    if (-not $allEqual) { throw 'Native release binaries were not byte-for-byte reproducible.' }
}

$report = [ordered]@{
    schema = 'aethercore.reproducibility.v1'
    source_date_epoch = [int64]$env:SOURCE_DATE_EPOCH
    locked_inputs = $inputHashes
    rust_release_controls = @('codegen-units=1','incremental=false','/Brepro','/INCREMENTAL:NO')
    sbom_reproducibility = 'cargo-cyclonedx 0.5.9 consumes SOURCE_DATE_EPOCH; pnpm SBOM is generated from the committed lockfile.'
    native_double_build = $native
    msi_byte_for_byte = [ordered]@{
        claimed = $false
        status = 'known-upstream-limitation'
        reason = 'WiX currently emits varying MSI PackageCode and summary timestamps; AetherCore does not claim byte-identical MSI output until upstream resolves that limitation.'
        upstream = 'https://github.com/wixtoolset/issues/issues/8978'
    }
}
$out = Join-Path $Root $OutputPath
New-Item -ItemType Directory -Force (Split-Path $out -Parent) | Out-Null
$report | ConvertTo-Json -Depth 8 | Set-Content $out -Encoding utf8
Write-Host "Reproducibility controls verified: $out" -ForegroundColor Green
