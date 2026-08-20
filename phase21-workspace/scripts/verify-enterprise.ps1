[CmdletBinding()]
param(
    [switch]$LiveTelemetry,[switch]$IncludePhase5Inventory,[switch]$LibFuzzer,[switch]$SkipOnlineSupplyChain,
    [switch]$UserShellPrivilegeCheck,[switch]$LiveReadOnlyFaultInjection,[switch]$LiveReadOnlySchedulerProbe,
    [switch]$RuntimeStress,[switch]$ExtendedSoak,[switch]$ReleasePackaging,[switch]$RequireSigning,[string]$UpdateTrustPath
)
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path;Set-Location $Root
Write-Host 'AetherCore Enterprise Convergence — definitive source/native verification gate' -ForegroundColor Cyan

$phase16=@{}
foreach($name in @('LiveTelemetry','IncludePhase5Inventory','LibFuzzer','SkipOnlineSupplyChain','UserShellPrivilegeCheck','LiveReadOnlyFaultInjection','LiveReadOnlySchedulerProbe')){
    if(Get-Variable -Name $name -ValueOnly){$phase16[$name]=$true}
}
& (Join-Path $PSScriptRoot 'verify-phase16.ps1') @phase16
if($LASTEXITCODE -ne 0){throw 'Inherited Phase 0-16 master verification failed.'}

& (Join-Path $PSScriptRoot 'enterprise-adversarial-audit.ps1')
if($LASTEXITCODE -ne 0){throw 'Enterprise adversarial architecture audit failed.'}

& cargo fmt --all -- --check
if($LASTEXITCODE -ne 0){throw 'Enterprise rustfmt gate failed.'}
& cargo clippy --workspace --all-targets --locked -- -D warnings
if($LASTEXITCODE -ne 0){throw 'Enterprise clippy -D warnings gate failed.'}
& cargo test --workspace --locked
if($LASTEXITCODE -ne 0){throw 'Enterprise full workspace regression gate failed.'}

foreach($case in @(
    @('aethercore-support-bundle','preview_reservations_count_against_global_quota_before_sanitization'),
    @('aethercore-support-bundle','preview_reservation_is_linearized_per_owner'),
    @('aethercore-support-bundle','preparation_reservations_count_against_global_quota_before_archive_io'),
    @('aethercore-support-bundle','preparation_reservation_is_linearized_per_owner'),
    @('aethercore-update-engine','observer_publication_never_runs_while_state_mutex_is_held'),
    @('aethercore-update-engine','upload_start_reservations_are_owner_scoped_not_global'),
    @('aethercore-update-engine','upload_records_use_independent_per_upload_mutexes'),
    @('aethercore-update-engine','staged_paths_are_owner_scoped_and_content_addressed'),
    @('aethercore-update-engine','stale_staging_cleanup_never_removes_protected_execution_artifact'),
    @('aethercore-operation-kernel','metrics_expose_backpressure_without_principal_content'),
    @('aethercore-operation-kernel','dropping_quiet_subscription_releases_registry_immediately'),
    @('aethercore-ipc','server_outbound_queue_saturates_fail_closed_without_blocking_request_workers'),
    @('aethercore-ipc','server_outbound_byte_budget_is_fail_closed_even_when_frame_slots_remain'),
    @('aethercore-ipc','client_outbound_queue_saturates_fail_closed_and_requests_transport_cancellation'),
    @('aethercore-ipc','byte_reservation_never_exceeds_limit_under_contention'),
    @('aethercore-ipc','disconnect_notification_is_at_most_once_across_reader_and_writer_paths'),
    @('aethercore-ipc','client_inflight_admission_matches_server_advertised_limit')
)){
    & (Join-Path $PSScriptRoot 'invoke-cargo-test-case.ps1') -Package $case[0] -TestName $case[1]
    if($LASTEXITCODE -ne 0){throw "Enterprise targeted regression failed: $($case[0])::$($case[1])"}
}

& pnpm --dir apps/ui check
if($LASTEXITCODE -ne 0){throw 'Enterprise strict Svelte/TypeScript gate failed.'}
& pnpm --dir apps/ui build
if($LASTEXITCODE -ne 0){throw 'Enterprise UI production build gate failed.'}

if($RuntimeStress -or $ExtendedSoak){
    $profile=if($ExtendedSoak){'release'}else{'standard'}
    & (Join-Path $PSScriptRoot 'enterprise-stress-matrix.ps1') -Profile $profile -LiveReadOnlyCollectors:$LiveReadOnlyFaultInjection -LibFuzzer:$LibFuzzer
    if($LASTEXITCODE -ne 0){throw 'Enterprise runtime stress matrix failed.'}
}

if($ReleasePackaging){
    Write-Host 'Re-running the authoritative Phase 16 packaging boundary only after Enterprise convergence gates passed.' -ForegroundColor Cyan
    $packageArgs=@{ReleasePackaging=$true;RequireSigning=[bool]$RequireSigning;SkipOnlineSupplyChain=[bool]$SkipOnlineSupplyChain}
    if($UpdateTrustPath){$packageArgs['UpdateTrustPath']=$UpdateTrustPath}
    & (Join-Path $PSScriptRoot 'verify-phase16.ps1') @packageArgs
    if($LASTEXITCODE -ne 0){throw 'Enterprise release packaging/signing boundary failed.'}
}
Write-Host 'AetherCore Enterprise convergence gate passed.' -ForegroundColor Green
Write-Host 'This gate does not by itself confer GA; verify-production.ps1 still requires signed lifecycle/matrix/extended-soak evidence.' -ForegroundColor DarkYellow
