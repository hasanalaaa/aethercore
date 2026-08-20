[CmdletBinding()]
param([switch]$LiveReadOnly)
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $Root

Write-Host 'Phase 14 — scheduler/preemption fault-injection regressions' -ForegroundColor Cyan

# Pure/deterministic safety primitives: publication linearization, watchdog cancellation,
# resource-governor cancellation, eligibility fail-closed policy, jitter/backoff and cadence.
& (Join-Path $PSScriptRoot 'invoke-cargo-test-case.ps1') -Package aethercore-collector-runtime -TestName revoked_commit_fence_rejects_late_publication
if($LASTEXITCODE -ne 0){throw 'CommitFence late-publication regression failed.'}
& (Join-Path $PSScriptRoot 'invoke-cargo-test-case.ps1') -Package aethercore-collector-runtime -TestName commit_and_revoke_are_linearized_by_one_boundary
if($LASTEXITCODE -ne 0){throw 'CommitFence linearization regression failed.'}
& (Join-Path $PSScriptRoot 'invoke-cargo-test-case.ps1') -Package aethercore-idle-scheduler -TestName preemption_is_fail_closed_for_activity_owner_session_and_mutation_changes
if($LASTEXITCODE -ne 0){throw 'Scheduler preemption policy regression failed.'}
& (Join-Path $PSScriptRoot 'invoke-cargo-test-case.ps1') -Package aethercore-idle-scheduler -TestName unknown_presentation_and_servicing_fail_closed
if($LASTEXITCODE -ne 0){throw 'Unknown eligibility fail-closed regression failed.'}
& (Join-Path $PSScriptRoot 'invoke-cargo-test-case.ps1') -Package aethercore-idle-scheduler -TestName cancellation_interrupts_governor_wait
if($LASTEXITCODE -ne 0){throw 'Scheduler governor cancellation regression failed.'}
& (Join-Path $PSScriptRoot 'invoke-cargo-test-case.ps1') -Package aethercore-idle-scheduler -TestName exponential_backoff_ceiling_is_capped
if($LASTEXITCODE -ne 0){throw 'Scheduler backoff regression failed.'}
& (Join-Path $PSScriptRoot 'invoke-cargo-test-case.ps1') -Package aethercore-persistence -TestName scheduler_cadence_is_durable_and_principal_scoped
if($LASTEXITCODE -ne 0){throw 'Principal-scoped durable cadence regression failed.'}

if($LiveReadOnly){
    Write-Host 'Running ignored read-only Windows eligibility probe...' -ForegroundColor DarkCyan
    & (Join-Path $PSScriptRoot 'invoke-cargo-test-case.ps1') -Package aethercore-idle-scheduler -TestName live_read_only_system_state_probe -Ignored -NoCapture
    if($LASTEXITCODE -ne 0){throw 'Live read-only Phase 14 eligibility probe failed.'}
}

Write-Host 'Phase 14 scheduler fault-injection regressions passed.' -ForegroundColor Green
