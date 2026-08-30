param(
    [ValidateSet('all','deep-scan-ipc','drivers','storage-memory','whea-crash','windows-integrity','startup-cleanup','resource-failure','reconnect-backpressure','ui-a11y','performance','persistence-upgrade','remediation-plan')]
    [string]$Scenario = 'all'
)
$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'Phase 17 native qualification must run on Windows.' }

$repo = Split-Path -Parent $PSScriptRoot
Push-Location $repo
try {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw 'cargo is required for Phase 17 qualification.' }
    if (-not (Get-Command npm -ErrorAction SilentlyContinue)) { throw 'npm is required for Phase 17 UI qualification.' }

    cargo test -p aethercore-pc-intelligence
    cargo test -p aethercore-persistence phase17_intelligence_history_findings_and_sealed_plan_are_durable
    cargo test -p aethercore-maintenance-service
    cargo test -p aethercore-contracts

    Push-Location (Join-Path $repo 'apps/ui')
    try {
        npm run check
    } finally { Pop-Location }

    Write-Host "SOURCE TEST GATES PASSED. Native scenario '$Scenario' still requires operator evidence capture for the matching QUALIFICATION_DEBT item." -ForegroundColor Yellow
    Write-Host 'Do not convert this message into a native PASS without attaching real Windows evidence.' -ForegroundColor Yellow
} finally { Pop-Location }
