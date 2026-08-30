[CmdletBinding()]
param()
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $Root

Write-Host 'AetherCore Zenith Recursive — targeted regression verification' -ForegroundColor Cyan
& (Join-Path $PSScriptRoot 'zenith-recursive-audit.ps1')
if($LASTEXITCODE -ne 0){throw 'Recursive source gate failed.'}

& cargo fmt --all -- --check
if($LASTEXITCODE -ne 0){throw 'cargo fmt check failed.'}
& cargo clippy --workspace --all-targets --locked -- -D warnings
if($LASTEXITCODE -ne 0){throw 'cargo clippy failed.'}
& cargo test --locked -p aethercore-operation-engine duplicated_plan_columns_cannot_diverge_from_hashed_material -- --nocapture
if($LASTEXITCODE -ne 0){throw 'Immutable plan coherence regression failed.'}
& cargo test --locked -p aethercore-operation-kernel inactive_owner_replay_state_is_bounded_and_reconnect_resets_safely -- --nocapture
if($LASTEXITCODE -ne 0){throw 'EventBus eviction/reset regression failed.'}
& cargo test --locked -p aethercore-operation-kernel active_owner_stream_is_never_evicted_under_retention_pressure -- --nocapture
if($LASTEXITCODE -ne 0){throw 'EventBus active-owner retention regression failed.'}
& cargo test --locked -p aethercore-maintenance-service global_request_slot_admission_is_strictly_bounded -- --nocapture
if($LASTEXITCODE -ne 0){throw 'Global request worker admission regression failed.'}
& cargo test --locked -p aethercore-operation-kernel lease_handoff_identity_is_exact -- --nocapture
if($LASTEXITCODE -ne 0){throw 'Mutation lease handoff identity regression failed.'}
& cargo test --locked -p aethercore-operation-kernel read_budget_lease_identity_is_exact -- --nocapture
if($LASTEXITCODE -ne 0){throw 'Read budget lease handoff identity regression failed.'}
& cargo test --locked -p aethercore-update-engine inactive_update_owner_state_cache_is_bounded -- --nocapture
if($LASTEXITCODE -ne 0){throw 'Update owner-state retention regression failed.'}
& cargo test --locked -p aethercore-update-engine mutation_critical_update_state_is_never_evicted -- --nocapture
if($LASTEXITCODE -ne 0){throw 'Update critical-state retention regression failed.'}
& cargo test --locked -p aethercore-ipc server_outbound_queue_saturates_fail_closed_without_blocking_request_workers -- --nocapture
if($LASTEXITCODE -ne 0){throw 'IPC server backpressure fail-closed regression failed.'}
& cargo test --locked -p aethercore-ipc client_outbound_queue_saturates_fail_closed_and_requests_transport_cancellation -- --nocapture
if($LASTEXITCODE -ne 0){throw 'IPC client cancellation-on-backpressure regression failed.'}

& (Join-Path $PSScriptRoot 'test-phase11-motion.ps1')
if($LASTEXITCODE -ne 0){throw 'Motion physics regression failed.'}
& (Join-Path $PSScriptRoot 'test-phase12-i18n.ps1')
if($LASTEXITCODE -ne 0){throw 'Localization regression failed.'}
& pnpm --dir apps/ui check
if($LASTEXITCODE -ne 0){throw 'Svelte static check failed.'}
& pnpm --dir apps/ui build
if($LASTEXITCODE -ne 0){throw 'UI production build failed.'}

Write-Host 'AetherCore Zenith Recursive targeted verification passed.' -ForegroundColor Green
Write-Host 'This targeted gate does not replace verify-zenith.ps1 or verify-production.ps1.' -ForegroundColor DarkYellow
