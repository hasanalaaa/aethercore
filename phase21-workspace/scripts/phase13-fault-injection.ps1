[CmdletBinding()]
param([switch]$LiveReadOnly)
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $Root

Write-Host 'Phase 13 deterministic fault-injection suite' -ForegroundColor Cyan
$tests = @(
  @('aethercore-collector-runtime','fault_detail_is_utf8_bounded_before_crossing_process_boundaries'),
  @('aethercore-collector-runtime','watchdog_cancels_and_returns_timeout'),
  @('aethercore-collector-runtime','timed_out_provider_remains_quarantined_until_worker_exits'),
  @('aethercore-collector-runtime','successful_gated_provider_releases_before_result_is_visible'),
  @('aethercore-collector-runtime','panic_is_contained_as_internal_fault'),
  @('aethercore-collector-runtime','parent_cancellation_propagates_to_children_without_reverse_poisoning'),
  @('aethercore-collector-runtime','external_cancellation_interrupts_supervisor_before_watchdog_deadline'),
  @('aethercore-hardware-telemetry','nvme_parser_rejects_truncated_vendor_response'),
  @('aethercore-hardware-telemetry','protocol_window_rejects_overflow_and_truncation'),
  @('aethercore-hardware-telemetry','ata_driver_response_rejects_forged_returned_length'),
  @('aethercore-hardware-telemetry','ata_driver_response_rejects_inconsistent_declared_payload'),
  @('aethercore-hardware-telemetry','nvme_parser_accepts_vendor_tail_without_reading_past_standard_prefix'),
  @('aethercore-crash-diagnostics','structured_payload_classification_does_not_require_xml'),
  @('aethercore-crash-diagnostics','render_property_count_is_rejected_before_allocation_when_pathological'),
  @('aethercore-diagnostic-engine','provider_supervisor_panic_is_classified'),
  @('aethercore-diagnostic-engine','scan_runtime_failure_marks_collecting_snapshot_failed'),
  @('aethercore-diagnostic-engine','provider_panic_is_contained_and_persisted_as_typed_fault'),
  @('aethercore-diagnostic-engine','nested_provider_faults_are_preserved_in_the_diagnostic_snapshot')
)
foreach ($case in $tests) {
  & (Join-Path $PSScriptRoot 'invoke-cargo-test-case.ps1') -Package $case[0] -TestName $case[1]
  if ($LASTEXITCODE -ne 0) { throw "Fault injection failed: $($case[0])::$($case[1])" }
}

if ($LiveReadOnly) {
  Write-Host 'Running ignored read-only Windows collector probes...' -ForegroundColor Cyan
  & (Join-Path $PSScriptRoot 'invoke-cargo-test-case.ps1') -Package aethercore-hardware-telemetry -TestName live_storage_and_memory_collection_is_read_only -Ignored
  if ($LASTEXITCODE -ne 0) { throw 'Live hardware collector probe failed.' }
  & (Join-Path $PSScriptRoot 'invoke-cargo-test-case.ps1') -Package aethercore-crash-diagnostics -TestName live_event_and_minidump_collection_is_read_only -Ignored
  if ($LASTEXITCODE -ne 0) { throw 'Live Event Log/minidump collector probe failed.' }
}
Write-Host 'Phase 13 fault-injection suite passed.' -ForegroundColor Green
