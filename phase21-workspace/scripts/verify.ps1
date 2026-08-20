[CmdletBinding()]
param()
$ErrorActionPreference='Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)

& cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) { throw 'rustfmt failed' }

& cargo test `
    -p aethercore-operation-engine `
    -p aethercore-persistence `
    -p aethercore-contracts `
    -p aethercore-diagnostics `
    -p aethercore-ipc `
    -p aethercore-security `
    -p aethercore-gpu-policy `
    -p aethercore-windows-pnp `
    -p aethercore-windows-update `
    -p aethercore-driver-hub `
    -p aethercore-restore-point `
    -p aethercore-driver-backup `
    -p aethercore-driver-install `
    -p aethercore-system-repair `
    -p aethercore-cleaner `
    -p aethercore-startup-manager `
    -p aethercore-hardware-telemetry `
    -p aethercore-crash-diagnostics `
    -p aethercore-diagnostic-engine
if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed' }

& pnpm --dir apps/ui check
if ($LASTEXITCODE -ne 0) { throw 'Svelte check failed' }
& pnpm --dir apps/ui build
if ($LASTEXITCODE -ne 0) { throw 'UI build failed' }
& cargo check --workspace
if ($LASTEXITCODE -ne 0) { throw 'Workspace compile check failed' }

$python = Get-Command python -ErrorAction SilentlyContinue
if ($python) {
    & python scripts/static_validate.py
    if ($LASTEXITCODE -ne 0) { throw 'Repository static validation failed' }
}

Write-Host 'AetherCore workspace verification passed.' -ForegroundColor Green
