[CmdletBinding()]
param(
    [switch]$LiveDiscovery
)

$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

& (Join-Path $PSScriptRoot 'verify.ps1')

if ($LiveDiscovery) {
    if (-not $IsWindows) {
        throw 'Live Phase 2 discovery tests require Windows.'
    }

    Push-Location $Root
    try {
        Write-Host 'Running physical PnP smoke test...' -ForegroundColor Cyan
        cargo test -p aethercore-windows-pnp --test live_inventory -- --ignored --nocapture
        if ($LASTEXITCODE -ne 0) { throw 'Live PnP inventory smoke test failed' }

        Write-Host 'Running live WUA driver discovery smoke test...' -ForegroundColor Cyan
        cargo test -p aethercore-windows-update --test live_wua -- --ignored --nocapture
        if ($LASTEXITCODE -ne 0) { throw 'Live WUA discovery smoke test failed' }
    }
    finally {
        Pop-Location
    }
}

Write-Host 'Phase 2 verification complete.' -ForegroundColor Green
