[CmdletBinding()]
param(
    [switch]$LiveTelemetry,
    [switch]$IncludePhase5Inventory
)
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path

# Preserve all native/security gates from Phase 0-6 first.
$phase6Args = @{}
if ($LiveTelemetry) { $phase6Args.LiveTelemetry = $true }
if ($IncludePhase5Inventory) { $phase6Args.IncludePhase5Inventory = $true }
& (Join-Path $PSScriptRoot 'verify-phase6.ps1') @phase6Args
if ($LASTEXITCODE -ne 0) { throw 'Phase 0-6 verification failed' }

Push-Location $Root
try {
    Write-Host 'Running Phase 7 strict Svelte accessibility/type diagnostics...' -ForegroundColor Cyan
    pnpm --dir apps/ui exec svelte-check --tsconfig ./tsconfig.json --threshold warning --fail-on-warnings
    if ($LASTEXITCODE -ne 0) { throw 'Phase 7 Svelte accessibility/type diagnostics failed' }

    Write-Host 'Building the production UI shell...' -ForegroundColor Cyan
    pnpm --dir apps/ui build
    if ($LASTEXITCODE -ne 0) { throw 'Phase 7 UI production build failed' }

    $python = Get-Command python -ErrorAction SilentlyContinue
    if ($python) {
        Write-Host 'Running Phase 7 design/accessibility/localization static invariants...' -ForegroundColor Cyan
        python scripts/static_validate.py
        if ($LASTEXITCODE -ne 0) { throw 'Phase 7 static validation failed' }
    }
}
finally { Pop-Location }

Write-Host 'Phase 7 verification complete.' -ForegroundColor Green
Write-Host 'Final visual/frame-pacing/High-DPI checks still require Windows 11 + WebView2 on physical monitors.' -ForegroundColor DarkYellow
