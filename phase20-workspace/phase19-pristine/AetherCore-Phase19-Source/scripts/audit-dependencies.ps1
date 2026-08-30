[CmdletBinding()]
param([string]$OutputDirectory = 'out\supply-chain')
$ErrorActionPreference = 'Stop'
$Root = Split-Path $PSScriptRoot -Parent
Set-Location $Root
if (-not (Test-Path 'Cargo.lock')) { throw 'Cargo.lock is required.' }
if (-not (Test-Path 'pnpm-lock.yaml')) { throw 'pnpm-lock.yaml is required.' }
$Out = Join-Path $Root $OutputDirectory
New-Item -ItemType Directory -Force $Out | Out-Null

$DenyVersion = '0.20.2'
$denyOk = $false
try {
    $v = (& cargo deny --version 2>$null | Out-String)
    if ($LASTEXITCODE -eq 0 -and $v -match [regex]::Escape($DenyVersion)) { $denyOk = $true }
} catch {}
if (-not $denyOk) {
    & cargo install --locked cargo-deny --version $DenyVersion
    if ($LASTEXITCODE -ne 0) { throw 'cargo-deny installation failed.' }
}

& cargo deny --locked check advisories licenses bans sources 2>&1 | Tee-Object -FilePath (Join-Path $Out 'cargo-deny.txt')
if ($LASTEXITCODE -ne 0) { throw 'cargo-deny rejected the Rust dependency graph.' }

$auditPath = Join-Path $Out 'pnpm-audit.json'
& pnpm --dir apps/ui audit --audit-level high --json | Out-File -FilePath $auditPath -Encoding utf8
if ($LASTEXITCODE -ne 0) { throw 'pnpm audit found a high or critical advisory.' }

$licensePath = Join-Path $Out 'pnpm-licenses.json'
& pnpm --dir apps/ui licenses list --json | Out-File -FilePath $licensePath -Encoding utf8
if ($LASTEXITCODE -ne 0) { throw 'pnpm license inventory failed.' }
$null = Get-Content $licensePath -Raw | ConvertFrom-Json

& cargo metadata --locked --format-version 1 | Out-File -FilePath (Join-Path $Out 'cargo-metadata.json') -Encoding utf8
if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed.' }
Write-Host "Dependency security and license audit passed. Reports: $Out" -ForegroundColor Green
