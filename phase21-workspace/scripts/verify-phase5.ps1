[CmdletBinding()]
param(
    [switch]$LiveInventory,
    [switch]$IncludePhase4Probes
)

$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

& (Join-Path $PSScriptRoot 'verify.ps1')
if ($LASTEXITCODE -ne 0) { throw 'Base Phase 0-5 verification failed' }

Push-Location $Root
try {
    Write-Host 'Running Phase 5 passive-default and reversible startup-manager tests...' -ForegroundColor Cyan
    cargo test -p aethercore-startup-manager -- --nocapture
    if ($LASTEXITCODE -ne 0) { throw 'Startup Manager tests failed' }

    Write-Host 'Re-running persistence recovery/journal tests for startup change records...' -ForegroundColor Cyan
    cargo test -p aethercore-persistence startup_change_record_is_durable_and_queryable -- --nocapture
    if ($LASTEXITCODE -ne 0) { throw 'Startup persistence durability test failed' }

    if ($LiveInventory) {
        if ($env:OS -ne 'Windows_NT') { throw 'Live Phase 5 inventory requires Windows.' }
        Write-Host 'Running read-only native startup/service inventory. No item is disabled or restored...' -ForegroundColor Yellow
        cargo test -p aethercore-startup-manager live_inventory_is_read_only -- --ignored --nocapture
        if ($LASTEXITCODE -ne 0) { throw 'Live Startup Manager inventory failed' }
    }

    if ($IncludePhase4Probes) {
        & (Join-Path $PSScriptRoot 'verify-phase4.ps1') -LiveCleanupScan
        if ($LASTEXITCODE -ne 0) { throw 'Phase 4 read-only probes failed' }
    }
}
finally {
    Pop-Location
}

Write-Host 'Phase 5 verification complete.' -ForegroundColor Green
Write-Host 'LiveInventory is observation-only. Real startup/service changes must use the immutable service-plan -> UAC -> durable evidence -> mutation -> verification path.' -ForegroundColor DarkYellow
