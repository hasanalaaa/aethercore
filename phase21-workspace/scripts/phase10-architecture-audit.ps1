[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $Root

function Require-File([string]$File,[string]$Label) {
    if (-not (Test-Path $File -PathType Leaf)) { throw "Missing Phase 10 artifact: $Label ($File)" }
}
function Require-Marker([string]$File,[string]$Pattern,[string]$Label) {
    Require-File $File $Label
    $text = Get-Content $File -Raw
    if ($text -notmatch $Pattern) { throw "Phase 10 invariant missing: $Label ($File)" }
}
function Reject-Marker([string]$File,[string]$Pattern,[string]$Label) {
    Require-File $File $Label
    $text = Get-Content $File -Raw
    if ($text -match $Pattern) { throw "Forbidden Phase 10 pattern detected: $Label ($File)" }
}
# `DBT-P63-004`: `services/maintenance-service/src/router.rs` was one 1,868-line
# file and is now a module root beside `router/*.rs`. Everything below that asks a
# question OF the router asks it of the verbs, so it reads the tree. Sorted, root
# first, so the join is reproducible.
function Get-ModuleText([string]$File) {
    Require-File $File "module root $File"
    $text = Get-Content $File -Raw
    $dir = [IO.Path]::ChangeExtension($File, $null).TrimEnd('.')
    if (Test-Path $dir -PathType Container) {
        foreach ($child in (Get-ChildItem $dir -Filter '*.rs' -File | Sort-Object Name)) {
            $text = $text + "`n" + (Get-Content $child.FullName -Raw)
        }
    }
    return $text
}

Write-Host 'Auditing Operation Kernel decomposition...' -ForegroundColor Cyan
Require-File 'services/maintenance-service/src/errors.rs' 'typed service error classifier'
Require-Marker 'services/maintenance-service/src/errors.rs' 'impl From<MutationError>' 'mutation busy/identity typed error mapping'
Require-Marker 'services/maintenance-service/src/errors.rs' 'impl From<HubError>' 'driver domain typed error mapping'
Require-Marker 'services/maintenance-service/src/errors.rs' 'impl From<RepairError>' 'repair domain typed error mapping'
Require-Marker 'services/maintenance-service/src/errors.rs' 'impl From<CleanerError>' 'cleanup domain typed error mapping'
Require-Marker 'services/maintenance-service/src/errors.rs' 'impl From<StartupError>' 'startup domain typed error mapping'
Require-Marker 'services/maintenance-service/src/errors.rs' 'foreign_domain_snapshot_state_is_non_enumerable' 'foreign domain state is not enumerable through error detail'
@(
    'authorization.rs','cancellation.rs','event_bus.rs','mutation.rs','recovery.rs',
    'state_machine.rs','telemetry.rs','work_budget.rs'
) | ForEach-Object { Require-File (Join-Path 'crates/operation-kernel/src' $_) "kernel module $_" }
Require-Marker 'Cargo.toml' '"crates/operation-kernel"' 'operation-kernel workspace member'
Require-Marker 'crates/operation-kernel/src/lib.rs' 'pub struct OperationKernel' 'single service control-plane composition root'
Require-Marker 'crates/operation-kernel/src/lib.rs' 'EventBus::new\(' 'kernel event bus'
Require-Marker 'crates/operation-kernel/src/lib.rs' 'ProgressTelemetryStore::with_observer' 'telemetry-to-event-bus bridge'
Require-Marker 'crates/operation-kernel/src/mutation.rs' 'MutationWorkload::Update|Self::Update' 'update participates in global mutation exclusion'
Require-Marker 'crates/operation-kernel/src/mutation.rs' 'exactly_one_mutation_can_own_the_machine' 'global mutation exclusion regression test'
Require-Marker 'crates/operation-kernel/src/mutation.rs' 'mutation_lease_rejects_unscoped_identity' 'mutation lease cannot exist without owner/plan identity'
Require-Marker 'crates/operation-kernel/src/state_machine.rs' 'transition_for_owner' 'kernel state transitions remain principal-scoped'
Require-Marker 'crates/operation-kernel/src/mutation.rs' 'release_is_observable_and_raii_backed' 'RAII mutation release event regression test'
Require-Marker 'crates/operation-kernel/src/mutation.rs' 'cross_principal_contention_never_discloses_foreign_plan_identity' 'cross-principal mutation contention privacy'
Require-Marker 'crates/operation-kernel/src/mutation.rs' 'snapshot_for_owner' 'mutation supervisor snapshot is owner-scoped'
Require-Marker 'crates/operation-kernel/src/work_budget.rs' 'read_only_workloads_can_overlap_within_global_budget' 'concurrent read-only budget regression test'
Require-Marker 'crates/operation-kernel/src/cancellation.rs' 'duplicate_cancellation_ids_are_rejected' 'cancellation-token rebinding protection'
Require-Marker 'crates/operation-kernel/src/cancellation.rs' 'disconnect_cancels_only_the_owning_session_requests' 'session disconnect cancellation isolation'
Require-Marker 'crates/operation-kernel/src/event_bus.rs' 'sequence_is_monotonic_per_owner_and_does_not_leak_cross_user_activity' 'principal-local sequence privacy regression test'
Require-Marker 'crates/operation-kernel/src/event_bus.rs' 'sequence_from_previous_service_epoch_requires_reset' 'service-restart sequence reset regression test'
Require-Marker 'crates/operation-kernel/src/event_bus.rs' 'replay_window_exhaustion_is_explicit' 'bounded replay-window regression test'
Require-Marker 'crates/operation-kernel/src/event_bus.rs' 'subscription_install_has_no_publish_replay_race' 'atomic replay/subscription installation regression test'
Require-Marker 'crates/operation-kernel/src/event_bus.rs' 'bounded_subscriber_overflow_is_reported_as_lag_not_silent_loss' 'bounded subscriber lag is explicit rather than silently lossy'

Write-Host 'Auditing modular typed Protobuf contracts...' -ForegroundColor Cyan
$protoFiles = @('common.proto','operations.proto','drivers.proto','repair.proto','cleanup.proto','startup.proto','diagnostics.proto','events.proto')
foreach ($name in $protoFiles) { Require-File (Join-Path 'crates/contracts/proto' $name) "modular schema $name" }
Require-Marker 'crates/contracts/proto/common.proto' 'message ErrorInfo' 'typed error structure'
Require-Marker 'crates/contracts/proto/common.proto' 'enum OperationState' 'formal operation state enum'
Require-Marker 'crates/contracts/proto/common.proto' 'enum DiscoveryState' 'formal discovery state enum'
Require-Marker 'crates/contracts/proto/common.proto' 'enum RiskLevel' 'formal risk enum'
foreach ($domain in @('drivers','repair','cleanup','startup','diagnostics')) {
    Require-Marker "crates/contracts/proto/$domain.proto" 'state_code' "$domain typed state field"
}
Require-Marker 'crates/contracts/proto/events.proto' 'message ClientHello' 'persistent session handshake'
Require-Marker 'crates/contracts/proto/events.proto' 'message SessionRequest' 'deadline/cancellation request envelope'
Require-Marker 'crates/contracts/proto/events.proto' 'uint64 sequence = 1' 'event sequence numbers'
Require-Marker 'crates/contracts/proto/events.proto' 'STREAM_RESET_REASON_SEQUENCE_RESET' 'service restart stream reset'
Require-Marker 'crates/contracts/proto/events.proto' 'STREAM_RESET_REASON_SUBSCRIBER_LAGGED' 'slow-subscriber stream reset'
Require-Marker 'crates/contracts/proto/events.proto' 'MutationWorkloadKind' 'typed mutation workload event'
Require-Marker 'crates/contracts/proto/events.proto' 'message ProgressTelemetryEvent' 'typed transient progress event'
Require-Marker 'crates/contracts/build.rs' 'proto/events\.proto' 'prost compiles modular import graph'

Write-Host 'Auditing persistent authenticated IPC v7...' -ForegroundColor Cyan
Require-Marker 'crates/ipc/src/lib.rs' 'AetherCore\.Maintenance\.v7' 'v7 named-pipe endpoint'
Require-Marker 'crates/ipc/src/lib.rs' 'decode_client_frame_bytes' 'fuzzable production session-frame parser'
Require-Marker 'crates/contracts/src/lib.rs' 'MAX_CLIENT_SESSION_FRAME_BYTES' 'bounded client-to-service session frame limit'
Require-Marker 'crates/contracts/src/lib.rs' 'MAX_SERVER_SESSION_FRAME_BYTES' 'large server-to-client response/event frame limit'
Require-Marker 'crates/ipc/src/lib.rs' 'session_direction_limits_match_trust_and_payload_shape' 'directional session frame limit regression test'
Require-Marker 'crates/ipc/src/windows_impl.rs' 'pub struct SessionClient' 'long-lived desktop session client'
Require-Marker 'crates/ipc/src/windows_impl.rs' 'replay_after_sequence' 'client reconnect replay cursor'
Require-Marker 'crates/ipc/src/windows_impl.rs' 'on_stream_reset' 'stream reset callback'
Require-Marker 'crates/ipc/src/windows_impl.rs' 'duplicate in-flight request id' 'desktop-side correlation-id collision defense'
Require-Marker 'crates/ipc/src/windows_impl.rs' 'impl Drop for SessionClient' 'persistent/one-shot client shutdown lifecycle'
Require-Marker 'crates/ipc/src/windows_impl.rs' 'CancelSynchronousIo' 'persistent session teardown cancels blocked synchronous pipe I/O'
# `Require-Marker`'s pattern is a REGEX: `shutdown();` parses as `shutdown` + an EMPTY
# GROUP + `;`, which cannot match the literal `shutdown();` in the source. Escaped.
# `DBT-P63-014`.
Require-Marker 'crates/ipc/src/windows_impl.rs' 'self\.writer\.shutdown\(\);' 'SessionClient drop delegates to cancellation-first transport shutdown'
Require-Marker 'services/maintenance-service/src/server.rs' 'inspect_named_pipe_client' 'principal derived from exact accepted pipe client'
Require-Marker 'services/maintenance-service/src/server.rs' 'MAX_SESSIONS\s*:\s*usize\s*=\s*32' 'bounded authenticated sessions'
Require-Marker 'services/maintenance-service/src/server.rs' 'MAX_SESSIONS_PER_USER_SID\s*:\s*usize\s*=\s*4' 'one local user cannot monopolize persistent session capacity'
Require-Marker 'services/maintenance-service/src/server.rs' 'admit_user_session\(&peer\.user_sid\)' 'session quota is keyed by kernel-observed user SID'
Require-Marker 'services/maintenance-service/src/server.rs' 'MAX_INFLIGHT_PER_SESSION\s*:\s*usize\s*=\s*8' 'bounded in-flight requests per session'
Require-Marker 'services/maintenance-service/src/server.rs' '\.subscribe\(&owner, hello\.replay_after_sequence\)' 'principal-scoped replay subscription'
Require-Marker 'services/maintenance-service/src/server.rs' 'cancel_session\(&session_id\)' 'disconnect cancellation propagation'
Require-Marker 'services/maintenance-service/src/server.rs' 'deadline_unix_ms' 'server-enforced request deadlines'
Require-Marker 'services/maintenance-service/src/server.rs' 'SubscriberLagged' 'bounded backpressure reset path'
Require-Marker 'services/maintenance-service/src/server.rs' 'publish_hydration' 'ordered typed session hydration'
Require-Marker 'crates/contracts/proto/operations.proto' 'HydrateSessionRequest' 'explicit renderer/bootstrap stream hydration RPC'
Require-Marker 'apps/desktop/src/main.rs' 'Payload::HydrateSession' 'renderer reload hydration over persistent session'
# `App.svelte` is a five-line wrapper around `app/AppShell.svelte` now, and the session
# wiring moved to `platform/kernel-session.ts` + `platform/stream-state.ts`. These four
# asserted a file that no longer holds the behaviour; `resetStreamBackedState` was also
# renamed to `applyStreamReset`. `DBT-P63-014`.
Require-Marker 'apps/ui/src/platform/stream-state.ts' 'export function applyStreamReset' 'stream reset clears stale renderer state before hydration'
Require-Marker 'services/maintenance-service/src/server.rs' 'active_request_ids' 'per-session duplicate request correlation defense'
Require-Marker 'crates/operation-kernel/src/cancellation.rs' 'AlreadyRegistered' 'duplicate active cancellation identifier defense'
Require-Marker 'fuzz/fuzz_targets/ipc_frame.rs' 'decode_client_frame_bytes' 'v7 session parser libFuzzer coverage'

Write-Host 'Auditing machine mutation supervisor integration...' -ForegroundColor Cyan
$serviceControl = ((Get-ModuleText 'services/maintenance-service/src/router.rs') + "`n" + (Get-Content 'services/maintenance-service/src/streaming.rs' -Raw))
foreach ($workload in @('DriverInstall','SystemRepair','Cleanup','Startup')) {
    if ($serviceControl -notmatch "MutationWorkload::$workload") { throw "Service does not acquire global machine mutation lease for $workload." }
}
if ($serviceControl -notmatch 'durable_mutation_released') { throw 'Mutation lease lacks durable terminal-state fallback.' }
foreach ($read in @('DriverDiscovery','RepairAssessment','CleanupDiscovery','StartupDiscovery','Diagnostics')) {
    if ($serviceControl -notmatch "ReadWorkload::$read") { throw "Read-only resource budget is not wired for $read." }
}

Write-Host 'Auditing transient telemetry / durable journal split...' -ForegroundColor Cyan
Require-Marker 'crates/operation-kernel/src/telemetry.rs' 'pub struct ProgressTelemetryStore' 'in-memory transient telemetry store'
foreach ($domain in @('driver-install','system-repair','cleaner','startup-manager')) {
    Require-Marker "crates/$domain/src/lib.rs" 'telemetry\.publish' "$domain publishes mutation progress to transient plane"
    Require-Marker "crates/$domain/src/lib.rs" 'owner_principal_key' "$domain telemetry is principal-scoped"
}
Require-Marker 'crates/driver-install/src/lib.rs' 'percent_delta\s*>=\s*5' 'coalesced durable WUA progress threshold'
Require-Marker 'crates/driver-install/src/lib.rs' '>=\s*2_000' 'bounded durable WUA progress checkpoint interval'
Require-Marker 'crates/driver-install/src/lib.rs' 'get_for_owner\(owner_principal_key' 'driver live telemetry owner-scoped lookup'
Require-Marker 'crates/operation-kernel/src/telemetry.rs' 'transient_telemetry_is_scoped_by_owner_even_for_the_same_plan_id' 'transient telemetry owner isolation regression test'
foreach ($domain in @('driver-install','system-repair','cleaner','startup-manager')) {
    Require-Marker "crates/$domain/src/lib.rs" 'clear_for_owner' "$domain clears transient telemetry with owner scope"
}
Require-Marker 'crates/persistence/migrations/0007_phase10_kernel.sql' 'NOT persisted' 'journal explicitly excludes frame-rate telemetry'
Require-Marker 'crates/persistence/src/lib.rs' '0007_phase10_kernel\.sql' 'Phase 10 migration registered'

Write-Host 'Auditing renderer polling elimination and reconnect ordering...' -ForegroundColor Cyan
$ui = Get-ChildItem 'apps/ui/src' -Recurse -File | ForEach-Object { Get-Content $_.FullName -Raw }
$uiText = $ui -join "`n"
if ($uiText -match '\bsetInterval\s*\(' -or $uiText -match '\bclearInterval\s*\(') {
    throw 'Renderer polling timer remains after streaming migration.'
}
Require-Marker 'apps/ui/src/platform/kernel-session.ts' 'aethercore://kernel-event' 'renderer consumes live kernel event stream'
Require-Marker 'apps/ui/src/platform/kernel-session.ts' 'start_ipc_session' 'renderer explicitly starts session after listener registration'
Require-Marker 'apps/ui/src/platform/kernel-session.ts' 'stream-reset|streamReset' 'renderer consumes explicit stream resets'
Require-Marker 'apps/desktop/src/main.rs' 'SESSION_DESIRED' 'background reconnect waits for renderer readiness'
Require-Marker 'apps/desktop/src/main.rs' 'LAST_EVENT_SEQUENCE\.store\(reset\.current_sequence' 'service-restart sequence reset can move cursor backward'
Require-Marker 'apps/desktop/src/main.rs' 'CONNECT_LOCK' 'desktop reconnect serialization'
Require-Marker 'apps/desktop/src/main.rs' 'invalidate_session_if_current' 'stale failed session cannot erase a replacement'
Require-Marker 'apps/desktop/src/main.rs' 'Arc::ptr_eq' 'session invalidation is identity-checked'

Write-Host 'Auditing service decomposition and production sanitation...' -ForegroundColor Cyan
foreach ($name in @('composition.rs','errors.rs','router.rs','protocol.rs','streaming.rs','server.rs')) { Require-File (Join-Path 'services/maintenance-service/src' $name) "service module $name" }
$mainLines = (Get-Content 'services/maintenance-service/src/main.rs').Count
if ($mainLines -ge 220) { throw "maintenance-service/main.rs remains monolithic ($mainLines lines)." }
$routerLines = (Get-Content 'services/maintenance-service/src/router.rs').Count
if ($routerLines -ge 220) { throw "maintenance-service/router.rs remains monolithic ($routerLines lines)." }
# The loophole decomposition opens: the dispatcher must not reappear one directory
# down. `DBT-P63-004`.
$routerModules = @(Get-ChildItem 'services/maintenance-service/src/router' -Filter '*.rs' -File)
if ($routerModules.Count -lt 2) { throw 'maintenance-service/router is not decomposed into domain modules.' }
foreach ($module in $routerModules) {
    $moduleLines = (Get-Content $module.FullName).Count
    if ($moduleLines -ge 260) { throw "maintenance-service/router/$($module.Name) is monolithic ($moduleLines lines)." }
}
Reject-Marker 'crates/ipc/src/lib.rs' 'AetherCore\.Maintenance\.v1' 'obsolete connection-per-request pipe endpoint'
Reject-Marker 'apps/ui/src/App.svelte' 'setInterval\s*\(' 'legacy renderer polling loop'
# P75: (?m). Get-Content -Raw is one string, so a bare ^ matched only its first character and
# a definition on any later line passed.
Reject-Marker 'crates/contracts/proto/aethercore.proto' '(?m)^\s*(message|enum|service)\s+' 'aggregator must not regain monolithic definitions'

Write-Host "Phase 10 architecture/source audit passed. main.rs=$mainLines lines; router.rs=$routerLines lines + $($routerModules.Count) domain modules." -ForegroundColor Green
