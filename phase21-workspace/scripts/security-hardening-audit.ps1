[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$Root = Split-Path $PSScriptRoot -Parent
Set-Location $Root

function Require-Marker([string]$File,[string]$Pattern,[string]$Label) {
    if (-not (Test-Path $File)) { throw "Missing security source: $File" }
    $text = Get-Content $File -Raw
    if ($text -notmatch $Pattern) { throw "Security invariant missing: $Label ($File)" }
}
function Reject-Marker([string]$File,[string]$Pattern,[string]$Label) {
    if (-not (Test-Path $File)) { throw "Missing security source: $File" }
    $text = Get-Content $File -Raw
    if ($text -match $Pattern) { throw "Forbidden security pattern detected: $Label ($File)" }
}

# IPC: local-only named pipe, framed allocation caps, bounded IDs and an exposed parser for fuzzing.
Require-Marker 'crates/ipc/src/windows_impl.rs' 'PIPE_REJECT_REMOTE_CLIENTS' 'remote named-pipe clients rejected'
Require-Marker 'crates/ipc/src/lib.rs' 'MAX_REQUEST_FRAME_BYTES' 'request allocation ceiling enforced'
Require-Marker 'crates/ipc/src/lib.rs' 'decode_client_frame_bytes' 'production v7 client-frame parser available to fuzz target'
Require-Marker 'crates/contracts/src/lib.rs' 'MAX_REQUEST_ID_BYTES\s*:\s*usize\s*=\s*128' 'request ID size ceiling'
# `DBT-P63-004`: the request-id check runs in the dispatcher's preamble, now
# `router/dispatch.rs`.
Require-Marker 'services/maintenance-service/src/router/dispatch.rs' 'is_safe_request_id' 'service validates request ID syntax before dispatch'

# Consent: elevated broker *and* exact executable identity; no path-prefix authorization.
Require-Marker 'crates/security/src/lib.rs' 'peer\.elevated' 'broker elevation required'
Require-Marker 'crates/security/src/lib.rs' 'is_expected_broker' 'exact broker executable binding required'

# Destructive file operations: reparse-point refusal + final path by handle before mutation.
Require-Marker 'crates/cleaner/src/windows_impl.rs' 'FILE_ATTRIBUTE_REPARSE_POINT' 'reparse points rejected'
Require-Marker 'crates/cleaner/src/windows_impl.rs' 'GetFinalPathNameByHandleW' 'final path resolved by handle'
Require-Marker 'crates/cleaner/src/windows_impl.rs' 'FILE_FLAG_OPEN_REPARSE_POINT' 'reparse target itself opened without following it'

# Installer hardener is a fixed-purpose helper, not a general privileged command runner.
Require-Marker 'apps/install-hardener/src/main.rs' 'mode == OsStr::new\("apply"\)' 'single literal hardener verb'
Require-Marker 'apps/install-hardener/src/main.rs' 'System32' 'fixed System32 tool roots'
Require-Marker 'apps/install-hardener/src/main.rs' 'sidtype.*unrestricted' 'service-specific SID policy compatible with broad maintenance mutations'
Require-Marker 'apps/install-hardener/src/main.rs' 'reset_acl_tree' 'repair removes stale explicit ACL drift before allowlist'
Require-Marker 'apps/install-hardener/src/main.rs' 'Component::ParentDir' 'installer rejects parent traversal syntax'
Require-Marker 'apps/install-hardener/src/main.rs' 'Prefix::Disk|Prefix::VerbatimDisk' 'installer roots are local drive paths, not UNC/device paths'
Require-Marker 'apps/install-hardener/src/main.rs' 'FILE_ATTRIBUTE_REPARSE_POINT' 'installer ACL trees reject reparse points'
Require-Marker 'apps/install-hardener/src/main.rs' 'symlink_metadata' 'installer reparse checks inspect links without following them'
Require-Marker 'apps/install-hardener/src/main.rs' '"/L"' 'icacls operates on symbolic links rather than following their destinations'
Reject-Marker 'apps/install-hardener/src/main.rs' 'cmd\.exe|powershell\.exe|pwsh\.exe' 'no shell execution in privileged MSI helper'
Require-Marker 'installer/wix/Product.wxs' 'ExeCommand="apply"' 'MSI passes only literal hardener verb'
Require-Marker 'installer/wix/Product.wxs' 'Impersonate="no"' 'hardener deferred under installer service context'
Require-Marker 'installer/wix/Product.wxs' 'After="InstallServices"' 'hardener sequenced only after service creation'
Reject-Marker 'installer/wix/Product.wxs' '<ServiceConfig\b' 'service SID and delayed-auto delegated to fixed hardener rather than MSI ServiceConfig'

# Tauri capability remains minimal; shell/process plugins are not granted to web content.
Require-Marker 'apps/desktop/capabilities/default.json' '"core:default"' 'minimal Tauri core permission set'
Reject-Marker 'apps/desktop/capabilities/default.json' 'shell:|process:|fs:|http:' 'no broad Tauri shell/process/fs/http capability'

# There must be no command-string service API or network listener introduced by packaging.
$proto = (Get-ChildItem 'crates/contracts/proto' -Filter '*.proto' -File | Sort-Object Name | ForEach-Object { Get-Content $_.FullName -Raw }) -join "`n"
if ($proto -match '(?i)execute[_ ]?(command|shell)|powershell|cmdline|raw[_ ]?command') {
    throw 'Forbidden arbitrary command surface detected in Protobuf contract.'
}
$rust = Get-ChildItem apps,services,crates -Recurse -File -Filter '*.rs' | ForEach-Object { Get-Content $_.FullName -Raw }
if (($rust -join "`n") -match 'TcpListener|UdpSocket') { throw 'Unexpected network listener/socket introduced in native product code.' }

Write-Host 'Security hardening source audit passed through Phase 10.' -ForegroundColor Green
