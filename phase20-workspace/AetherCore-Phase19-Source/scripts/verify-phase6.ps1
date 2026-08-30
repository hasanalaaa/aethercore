[CmdletBinding()]
param(
    [switch]$LiveTelemetry,
    [switch]$IncludePhase5Inventory
)
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
& (Join-Path $PSScriptRoot 'verify.ps1')
if ($LASTEXITCODE -ne 0) { throw 'Base Phase 0-6 verification failed' }
Push-Location $Root
try {
    Write-Host 'Running Phase 6 hardware/crash diagnostic rule tests...' -ForegroundColor Cyan
    cargo test -p aethercore-hardware-telemetry -p aethercore-crash-diagnostics -p aethercore-diagnostic-engine -- --nocapture
    if ($LASTEXITCODE -ne 0) { throw 'Phase 6 diagnostic tests failed' }
    cargo test -p aethercore-persistence diagnostic_snapshot_is_durable -- --nocapture
    if ($LASTEXITCODE -ne 0) { throw 'Diagnostic persistence durability test failed' }
    if ($LiveTelemetry) {
        if ($env:OS -ne 'Windows_NT') { throw 'Live Phase 6 telemetry requires Windows.' }
        Write-Host 'Running read-only storage/memory and event/minidump collectors. No repair or mutation is performed...' -ForegroundColor Yellow
        cargo test -p aethercore-hardware-telemetry live_storage_and_memory_collection_is_read_only -- --ignored --nocapture
        if ($LASTEXITCODE -ne 0) { throw 'Live hardware telemetry probe failed' }
        cargo test -p aethercore-crash-diagnostics live_event_and_minidump_collection_is_read_only -- --ignored --nocapture
        if ($LASTEXITCODE -ne 0) { throw 'Live crash diagnostics probe failed' }
    }
    if ($IncludePhase5Inventory) {
        & (Join-Path $PSScriptRoot 'verify-phase5.ps1') -LiveInventory
        if ($LASTEXITCODE -ne 0) { throw 'Phase 5 read-only inventory probe failed' }
    }
}
finally { Pop-Location }
Write-Host 'Phase 6 verification complete.' -ForegroundColor Green
Write-Host 'LiveTelemetry is observation-only: storage SMART/reliability, memory pressure, WHEA/Event Log and minidump metadata.' -ForegroundColor DarkYellow
