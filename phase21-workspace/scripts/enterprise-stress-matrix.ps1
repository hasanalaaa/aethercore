[CmdletBinding()]
param(
    [ValidateSet('standard','release')][string]$Profile='standard',
    [switch]$LiveReadOnlyCollectors,
    [switch]$LibFuzzer,
    [string]$EvidenceDirectory='out\ga-evidence'
)
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path;Set-Location $Root
if($env:OS -ne 'Windows_NT'){throw 'Enterprise runtime stress qualification requires Windows.'}
$matrix=Get-Content (Join-Path $Root 'release\enterprise-stress-matrix.json') -Raw|ConvertFrom-Json
$config=$matrix.runtime_profiles.$Profile
if(-not $config){throw "Unknown enterprise stress profile: $Profile"}
New-Item -ItemType Directory -Force (Join-Path $Root $EvidenceDirectory)|Out-Null

function Target([string]$Package,[string]$Test){
    & (Join-Path $PSScriptRoot 'invoke-cargo-test-case.ps1') -Package $Package -TestName $Test
    if($LASTEXITCODE -ne 0){throw "Enterprise targeted regression failed: $Package::$Test"}
}
Write-Host "AetherCore Enterprise stress matrix — $Profile" -ForegroundColor Cyan
Target 'aethercore-support-bundle' 'preview_reservations_count_against_global_quota_before_sanitization'
Target 'aethercore-support-bundle' 'preview_reservation_is_linearized_per_owner'
Target 'aethercore-support-bundle' 'preparation_reservations_count_against_global_quota_before_archive_io'
Target 'aethercore-support-bundle' 'preparation_reservation_is_linearized_per_owner'
Target 'aethercore-update-engine' 'observer_publication_never_runs_while_state_mutex_is_held'
Target 'aethercore-update-engine' 'upload_start_reservations_are_owner_scoped_not_global'
Target 'aethercore-update-engine' 'upload_records_use_independent_per_upload_mutexes'
Target 'aethercore-update-engine' 'staged_paths_are_owner_scoped_and_content_addressed'
Target 'aethercore-update-engine' 'stale_staging_cleanup_never_removes_protected_execution_artifact'
Target 'aethercore-operation-kernel' 'metrics_expose_backpressure_without_principal_content'
Target 'aethercore-operation-kernel' 'dropping_quiet_subscription_releases_registry_immediately'
Target 'aethercore-ipc' 'server_outbound_queue_saturates_fail_closed_without_blocking_request_workers'
Target 'aethercore-ipc' 'server_outbound_byte_budget_is_fail_closed_even_when_frame_slots_remain'
Target 'aethercore-ipc' 'client_outbound_queue_saturates_fail_closed_and_requests_transport_cancellation'
Target 'aethercore-ipc' 'byte_reservation_never_exceeds_limit_under_contention'
Target 'aethercore-ipc' 'disconnect_notification_is_at_most_once_across_reader_and_writer_paths'
Target 'aethercore-ipc' 'client_inflight_admission_matches_server_advertised_limit'

$serviceTokenOut=Join-Path $EvidenceDirectory 'maintenance-service-token.json'
& (Join-Path $PSScriptRoot 'verify-maintenance-service-token.ps1') -OutputPath $serviceTokenOut
if($LASTEXITCODE -ne 0){throw 'Native maintenance service token verification failed.'}

$ipcSecurityOut=Join-Path $EvidenceDirectory 'ipc-pipe-security.json'
& (Join-Path $PSScriptRoot 'verify-ipc-pipe-security.ps1') -OutputPath $ipcSecurityOut
if($LASTEXITCODE -ne 0){throw 'Native IPC pipe security verification failed.'}

$restarts=[int]$config.service_restart_cycles
$resilienceOut=Join-Path $EvidenceDirectory 'resilience.json'
& (Join-Path $PSScriptRoot 'phase16-resilience-matrix.ps1') -ServiceRestartCycles $restarts -LiveReadOnlyCollectors:$LiveReadOnlyCollectors -LibFuzzer:$LibFuzzer -OutputPath $resilienceOut
if($LASTEXITCODE -ne 0){throw 'Enterprise resilience matrix failed.'}

$soakProfile=[string]$config.soak_profile
$stressOut=Join-Path $EvidenceDirectory 'stress-soak.json'
& (Join-Path $PSScriptRoot 'phase16-stress-soak.ps1') -Profile $soakProfile -OutputPath $stressOut
if($LASTEXITCODE -ne 0){throw 'Enterprise stress/soak matrix failed.'}
$trendOut=Join-Path $EvidenceDirectory 'enterprise-resource-trend.json'
& python (Join-Path $PSScriptRoot 'enterprise-soak-analyze.py') --stress (Join-Path $Root $stressOut) --matrix (Join-Path $Root 'release\enterprise-stress-matrix.json') --profile $Profile --output (Join-Path $Root $trendOut)
if($LASTEXITCODE -ne 0){throw 'Enterprise sustained resource-trend analysis failed.'}

$version=(Get-ItemProperty 'HKLM:\Software\AetherCore' -Name InstallVersion -ErrorAction Stop).InstallVersion
$evidence=[ordered]@{
    schema='aethercore.enterprise-stress-evidence.v1';ok=$true;version=$version;profile=$Profile;
    targeted_regressions=17;maintenance_service_token=(Resolve-Path (Join-Path $Root $serviceTokenOut)).Path;ipc_pipe_security=(Resolve-Path (Join-Path $Root $ipcSecurityOut)).Path;
    resilience=(Resolve-Path (Join-Path $Root $resilienceOut)).Path;
    stress_soak=(Resolve-Path (Join-Path $Root $stressOut)).Path;
    resource_trend=(Resolve-Path (Join-Path $Root $trendOut)).Path;
    completed_utc=[DateTimeOffset]::UtcNow.ToString('o')
}
$enterpriseOut=Join-Path $Root (Join-Path $EvidenceDirectory 'enterprise-stress.json')
$evidence|ConvertTo-Json -Depth 6|Set-Content $enterpriseOut -Encoding utf8
Write-Host "Enterprise stress matrix passed. Evidence: $enterpriseOut" -ForegroundColor Green
