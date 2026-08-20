[CmdletBinding()]
param([string]$OutputDirectory = 'out\sbom')
$ErrorActionPreference = 'Stop'
$Root = Split-Path $PSScriptRoot -Parent
Set-Location $Root
if (-not $env:SOURCE_DATE_EPOCH) { throw 'SOURCE_DATE_EPOCH is required for deterministic SBOM metadata.' }
if (-not (Test-Path 'Cargo.lock')) { throw 'Cargo.lock is required.' }
if (-not (Test-Path 'pnpm-lock.yaml')) { throw 'pnpm-lock.yaml is required.' }

$Out = Join-Path $Root $OutputDirectory
$RustOut = Join-Path $Out 'rust'
New-Item -ItemType Directory -Force $RustOut | Out-Null
Get-ChildItem $RustOut -File -ErrorAction SilentlyContinue | Remove-Item -Force

$CycloneDxVersion = '0.5.9'
$installed = $false
try {
    $v = (& cargo cyclonedx --version 2>$null | Out-String)
    if ($LASTEXITCODE -eq 0 -and $v -match [regex]::Escape($CycloneDxVersion)) { $installed = $true }
} catch {}
if (-not $installed) {
    & cargo install --locked cargo-cyclonedx --version $CycloneDxVersion
    if ($LASTEXITCODE -ne 0) { throw 'cargo-cyclonedx installation failed.' }
}

# cargo-cyclonedx emits one SBOM per workspace package when run at the workspace root.
$before = @(Get-ChildItem $Root -Recurse -File -Filter '*.cdx.json' |
    Where-Object { $_.FullName -notlike "$(Join-Path $Root 'target')*" -and $_.FullName -notlike "$Out*" } |
    Select-Object -ExpandProperty FullName)
& cargo cyclonedx --format json
if ($LASTEXITCODE -ne 0) { throw 'Rust CycloneDX generation failed.' }
$after = @(Get-ChildItem $Root -Recurse -File -Filter '*.cdx.json' |
    Where-Object { $_.FullName -notlike "$(Join-Path $Root 'target')*" -and $_.FullName -notlike "$Out*" })
$newFiles = @($after | Where-Object { $before -notcontains $_.FullName })
if ($newFiles.Count -eq 0) {
    # Some cargo-cyclonedx versions overwrite existing package SBOMs; collect all workspace output.
    $newFiles = $after
}
foreach ($f in $newFiles) {
    $relative = [IO.Path]::GetRelativePath($Root, $f.DirectoryName) -replace '[\\/:]','_'
    $name = if ($relative -eq '.') { $f.Name } else { "$relative-$($f.Name)" }
    Move-Item $f.FullName (Join-Path $RustOut $name) -Force
}
if ((Get-ChildItem $RustOut -File).Count -eq 0) { throw 'No Rust SBOM files were produced.' }

$UiOut = Join-Path $Out 'ui.cdx.json'
& pnpm --dir apps/ui sbom --sbom-format cyclonedx --sbom-spec-version 1.7 --lockfile-only --sbom-type application --sbom-supplier AetherCore --out $UiOut
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $UiOut)) { throw 'pnpm CycloneDX generation failed.' }

Get-ChildItem $Out -Recurse -File | ForEach-Object {
    $null = Get-Content $_.FullName -Raw | ConvertFrom-Json
}
Write-Host "SBOMs generated in $Out" -ForegroundColor Green
