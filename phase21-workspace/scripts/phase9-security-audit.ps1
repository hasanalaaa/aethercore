[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $Root

& (Join-Path $PSScriptRoot 'security-hardening-audit.ps1')
if ($LASTEXITCODE -ne 0) { throw 'Inherited Phase 8 hardening audit failed.' }

function Require-Marker([string]$File,[string]$Pattern,[string]$Label) {
    if (-not (Test-Path $File)) { throw "Missing Phase 9 security source: $File" }
    $text = Get-Content $File -Raw
    if ($text -notmatch $Pattern) { throw "Phase 9 invariant missing: $Label ($File)" }
}
function Reject-Tree([string]$Pattern,[string]$Label) {
    $files = Get-ChildItem apps,services,crates -Recurse -File | Where-Object { $_.Extension -in '.rs','.proto','.ts','.svelte' }
    foreach ($file in $files) {
        $text = Get-Content $file.FullName -Raw
        if ($text -match $Pattern) { throw "Retired/prototype surface detected: $Label ($($file.FullName))" }
    }
}

# Principal/session ownership is derived by the service from the connected named-pipe client.
Require-Marker 'crates/security/src/lib.rs' 'GetNamedPipeClientProcessId' 'named-pipe client PID is kernel-derived'
Require-Marker 'crates/security/src/lib.rs' 'GetNamedPipeClientSessionId' 'named-pipe client session is kernel-derived'
# The call moved into the RAII owner that guarantees the revert: `security/src/lib.rs:128`
# takes `ThreadImpersonation::named_pipe_client(pipe)`, and that type is the ONLY caller of
# `ImpersonateNamedPipeClient` in the workspace (`windows-foundation/src/lib.rs:320`), which
# is what makes a missed `RevertToSelf` impossible rather than merely discouraged. Asserting
# the raw Win32 name in `security` asserted a spelling; asserting BOTH ends asserts the
# chain. First evaluated ever by run 34778679667, after `DBT-P63-013` let this file parse.
Require-Marker 'crates/security/src/lib.rs' 'ThreadImpersonation::named_pipe_client\(pipe\)' 'principal token is read from the exact connected pipe client'
Require-Marker 'crates/windows-foundation/src/lib.rs' 'ImpersonateNamedPipeClient\(pipe\)' 'the impersonation owner is the only caller of ImpersonateNamedPipeClient'
Require-Marker 'crates/security/src/lib.rs' 'OpenThreadToken' 'impersonated client token is queried directly'
Require-Marker 'crates/security/src/lib.rs' 'RevertToSelf' 'service thread reverts client impersonation before request dispatch'
Require-Marker 'crates/security/src/lib.rs' 'TokenStatistics' 'logon AuthenticationId captured from client token'
Require-Marker 'crates/security/src/lib.rs' 'TokenUser' 'client user SID captured from token'
Require-Marker 'crates/security/src/lib.rs' 'authentication_id' 'principal contains logon authentication identity'
Require-Marker 'crates/security/src/lib.rs' 'binding_key' 'SID/logon/session binding key exists'
Require-Marker 'services/maintenance-service/src/server.rs' 'inspect_named_pipe_client' 'service derives principal once for the persistent pipe session'
# `DBT-P63-004`: the router is a module tree; its per-request preamble - version,
# request-id syntax, cancellation checkpoint, principal/session rebinding - is
# `router/dispatch.rs`, which is what this asserts.
Require-Marker 'services/maintenance-service/src/router/dispatch.rs' 'peer\.binding_key\(\)' 'router revalidates the session principal binding'
Require-Marker 'crates/operation-engine/src/lib.rs' 'get_plan_for_owner' 'plan reads are owner-scoped'
Require-Marker 'crates/persistence/migrations/0006_phase9_security.sql' 'owner_principal_key' 'plan owner persisted in safety ledger'
Require-Marker 'crates/diagnostic-engine/src/lib.rs' 'snapshot_for_owner' 'diagnostic live snapshot is principal-scoped'
Require-Marker 'crates/persistence/src/lib.rs' 'diagnostic_snapshots_for_owner' 'diagnostic history is principal-scoped'
Require-Marker 'crates/driver-install/tests/coordinator.rs' 'cross_user_start_cannot_fail_or_mutate_an_owned_plan' 'cross-user driver start is fail-closed'

# Authorization 2.0: the non-elevated shell passes a non-secret intent ID plus an allowlisted presentation locale to the broker. The locale carries no authorization authority.
Require-Marker 'crates/contracts/proto/operations.proto' 'BeginConsentIntentRequest' 'typed consent intent creation request'
Require-Marker 'crates/contracts/proto/operations.proto' 'ApproveConsentIntentRequest' 'typed broker approval request'
Require-Marker 'apps/consent-broker/src/main.rs' '--intent-id' 'broker accepts service-minted non-secret intent identifier'
Require-Marker 'apps/consent-broker/src/main.rs' '--locale' 'broker accepts only a display-locale selector in addition to the intent identifier'
Require-Marker 'apps/consent-broker/src/main.rs' 'locale != "en" && locale != "ar"' 'broker display locale is strictly allowlisted'
Require-Marker 'apps/desktop/src/main.rs' '--locale' 'desktop passes presentation locale separately from authorization intent'
Require-Marker 'apps/desktop/src/main.rs' 'BeginConsentIntent' 'desktop requests service-minted consent intent'
Require-Marker 'apps/desktop/src/main.rs' 'approve_plan_with_uac' 'production UAC command uses consent semantics'
Require-Marker 'crates/persistence/src/lib.rs' 'consume_consent_and_transition' 'consent consume and state transition share one transaction'
Require-Marker 'crates/persistence/src/lib.rs' 'TransactionBehavior::Immediate' 'authorization transaction takes immediate write lock'
Require-Marker 'crates/operation-engine/src/lib.rs' 'record\.approved_unix_ms\.is_some\(\)' 'approved intents cannot be presented/reapproved'
Require-Marker 'crates/system-repair/src/lib.rs' 'repair execution journal initialization failed after consent' 'repair releases fail-closed after consent if journal init fails'
Require-Marker 'crates/cleaner/src/lib.rs' 'cleanup execution journal initialization failed after consent' 'cleanup releases fail-closed after consent if journal init fails'
Require-Marker 'crates/startup-manager/src/lib.rs' 'startup execution journal initialization failed after consent' 'startup releases fail-closed after consent if journal init fails'
if ((Get-Content 'apps/consent-broker/src/main.rs' -Raw) -match '--challenge') { throw 'Retired authorization secret/challenge argument reintroduced into consent broker.' }
if ((Get-Content 'apps/desktop/src/main.rs' -Raw) -match '--challenge') { throw 'Retired authorization secret/challenge argument reintroduced into desktop UAC launch path.' }

# Exact cleanup identity remains tied to the opened file object through delete.
Require-Marker 'crates/cleaner/src/windows_impl.rs' 'GetFileInformationByHandleEx' 'file identity is collected by handle'
Require-Marker 'crates/cleaner/src/windows_impl.rs' 'FileIdInfo' 'FILE_ID_INFO is used'
Require-Marker 'crates/cleaner/src/windows_impl.rs' 'volume_serial_number' 'volume identity is frozen'
Require-Marker 'crates/cleaner/src/windows_impl.rs' 'file_id_128' '128-bit file identifier is frozen'
Require-Marker 'crates/cleaner/src/windows_impl.rs' 'SetFileInformationByHandle' 'deletion remains handle-based'
Require-Marker 'crates/cleaner/src/windows_impl.rs' 'replacement_with_same_path_and_shape_is_rejected_by_file_identity' 'replacement-race regression test exists'

# Dependency freeze must cover both lock bytes and every dependency/tool manifest that can alter resolution.
Require-Marker 'scripts/freeze-dependencies.ps1' "'package.json'" 'root pnpm tool pin is dependency-frozen'
Require-Marker 'scripts/freeze-dependencies.ps1' "'.config/dotnet-tools.json'" 'WiX tool manifest is dependency-frozen'
Require-Marker 'scripts/freeze-dependencies.ps1' "'nuget.config'" 'NuGet trust/source policy is dependency-frozen'
Require-Marker 'scripts/freeze-dependencies.ps1' 'manifest_baseline_sha256' 'freeze metadata authenticates the approved manifest baseline bytes'
Require-Marker 'scripts/freeze-dependencies.ps1' 'lock_baseline_sha256' 'freeze metadata authenticates the approved lock baseline bytes'

# Production sanitation: all retired Phase 1 authorization/demo paths are gone from product code/contracts.
Reject-Tree '(?i)create_demo_plan|advance_demo|Phase1Simulation|simulation_only' 'Phase 1 demo transition surface'
Reject-Tree '(?i)issue_challenge|grant_authorization|authorization_challenge|authorization_grant' 'retired challenge/grant authorization surface'
# PowerShell's escape character inside a double-quoted string is a BACKTICK, not a
# backslash, so the `\"` here ended the string and everything after it re-parsed into an
# unterminated single-quoted string: `The string is missing the terminator: '.` — a PARSE
# error, so this whole file never ran, and neither did anything `verify-phase9.ps1` gates.
# A single-quoted string needs no escape for `"` and doubles `''` for its own quote, so the
# regex below is now exactly `invoke\([\'"]authorize_plan[\'"]`. `DBT-P63-013`.
Reject-Tree 'invoke\([''"]authorize_plan[''"]' 'legacy desktop authorization command'

$contracts = Get-Content 'crates/contracts/src/lib.rs' -Raw
if ($contracts -notmatch 'PROTOCOL_VERSION:\s*u32\s*=\s*7') { throw 'Phase 9 requires IPC protocol version 7.' }

Write-Host 'Phase 9 principal, consent, exact-file-identity, and sanitation source audit passed.' -ForegroundColor Green
