[CmdletBinding()]
param(
    [string]$OutputPath = 'out\ga-evidence\ipc-pipe-security.json'
)
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $Root
if ($env:OS -ne 'Windows_NT') { throw 'Native IPC pipe security verification requires Windows.' }

$ServiceName = 'AetherCoreMaintenance'
$PipePath = '\\.\pipe\AetherCore.Maintenance.v7'
# FILE_READ_DATA | FILE_WRITE_DATA | READ_CONTROL | SYNCHRONIZE.
# FILE_CREATE_PIPE_INSTANCE / FILE_APPEND_DATA (0x4) is intentionally absent.
$RequiredClientMask = [uint32]0x00120003
$CreatePipeInstance = [uint32]0x00000004

$service = Get-CimInstance Win32_Service -Filter "Name='$ServiceName'"
if (-not $service -or $service.State -ne 'Running' -or -not $service.ProcessId) {
    throw "$ServiceName must be installed and running before IPC pipe security verification."
}
if ($service.StartName -notin @('LocalSystem', 'Local System')) {
    throw "$ServiceName account drift: expected LocalSystem, found '$($service.StartName)'."
}
if ($service.ServiceType -notmatch 'Own Process') {
    throw "$ServiceName service-type drift: expected an own-process service, found '$($service.ServiceType)'."
}
$serviceSid = (New-Object System.Security.Principal.NTAccount('NT SERVICE',$ServiceName)).Translate([System.Security.Principal.SecurityIdentifier]).Value
$serviceSidType = (& "$env:SystemRoot\System32\sc.exe" qsidtype $ServiceName | Out-String)
if ($LASTEXITCODE -ne 0 -or $serviceSidType -notmatch 'UNRESTRICTED') {
    throw "$ServiceName SID-type drift: expected SERVICE_SID_TYPE_UNRESTRICTED. sc.exe output: $serviceSidType"
}

if (-not ('AetherCore.NativePipeSecurity' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Security.AccessControl;
using System.Security.Principal;
using Microsoft.Win32.SafeHandles;

namespace AetherCore {
    public sealed class PipeSecurityResult {
        public string OwnerSid { get; set; } = String.Empty;
        public int AuthenticatedUsersAllowMask { get; set; }
        public int AuthenticatedUsersDenyMask { get; set; }
        public int TrustedServiceAllowMask { get; set; }
        public int AdministratorsAllowMask { get; set; }
        public bool TrustedServiceCanCreatePipeInstance { get; set; }
        public bool DaclProtected { get; set; }
        public int AllowAceCount { get; set; }
        public int AuthenticatedUsersAllowAceCount { get; set; }
        public int TrustedServiceAllowAceCount { get; set; }
        public string OffendingCreateInstanceSids { get; set; } = String.Empty;
        public string UnexpectedAllowSids { get; set; } = String.Empty;
        public string UnexpectedDenySids { get; set; } = String.Empty;
        public string UnexpectedAceTypes { get; set; } = String.Empty;
    }

    public static class NativePipeSecurity {
        // Exact production client rights: FILE_READ_DATA | FILE_WRITE_DATA | READ_CONTROL | SYNCHRONIZE.
        private const uint PIPE_CLIENT_ACCESS = 0x00120003;
        private const uint SECURITY_IDENTIFICATION = 0x00010000;
        private const uint SECURITY_SQOS_PRESENT = 0x00100000;
        private const uint PIPE_SECURITY_QOS = SECURITY_IDENTIFICATION | SECURITY_SQOS_PRESENT;
        private const uint OPEN_EXISTING = 3;
        // This deliberately mirrors the Rust runtime endpoint-authentication contract.
        // A failure here is a qualification failure; do not silently substitute another object type.
        private const int SE_FILE_OBJECT = 1;
        private const uint OWNER_SECURITY_INFORMATION = 0x00000001;
        private const uint DACL_SECURITY_INFORMATION = 0x00000004;
        private const int FILE_CREATE_PIPE_INSTANCE = 0x00000004;
        private const int GENERIC_ALL = 0x10000000;
        private const int GENERIC_WRITE = 0x40000000;

        private static bool GrantsPipeCreateInstance(int mask) {
            return (mask & FILE_CREATE_PIPE_INSTANCE) != 0 ||
                   (mask & GENERIC_ALL) != 0 ||
                   (mask & GENERIC_WRITE) != 0;
        }

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern SafeFileHandle CreateFileW(
            string lpFileName,
            uint dwDesiredAccess,
            uint dwShareMode,
            IntPtr lpSecurityAttributes,
            uint dwCreationDisposition,
            uint dwFlagsAndAttributes,
            IntPtr hTemplateFile);

        [DllImport("advapi32.dll", SetLastError = true)]
        private static extern uint GetSecurityInfo(
            SafeFileHandle handle,
            int objectType,
            uint securityInfo,
            out IntPtr owner,
            out IntPtr group,
            out IntPtr dacl,
            out IntPtr sacl,
            out IntPtr securityDescriptor);

        [DllImport("advapi32.dll")]
        private static extern uint GetSecurityDescriptorLength(IntPtr securityDescriptor);

        [DllImport("kernel32.dll")]
        private static extern IntPtr LocalFree(IntPtr memory);

        public static PipeSecurityResult Inspect(string pipePath, string trustedServiceSidText) {
            using (SafeFileHandle pipe = CreateFileW(
                pipePath,
                PIPE_CLIENT_ACCESS,
                0,
                IntPtr.Zero,
                OPEN_EXISTING,
                PIPE_SECURITY_QOS,
                IntPtr.Zero)) {
                if (pipe.IsInvalid) {
                    throw new Win32Exception(Marshal.GetLastWin32Error(), "Open AetherCore named pipe with exact production client rights");
                }

                IntPtr owner, group, dacl, sacl, securityDescriptor;
                uint error = GetSecurityInfo(
                    pipe,
                    SE_FILE_OBJECT,
                    OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                    out owner,
                    out group,
                    out dacl,
                    out sacl,
                    out securityDescriptor);
                if (error != 0 || securityDescriptor == IntPtr.Zero) {
                    throw new Win32Exception((int)error, "GetSecurityInfo(named pipe owner/DACL)");
                }

                try {
                    uint length = GetSecurityDescriptorLength(securityDescriptor);
                    if (length == 0 || length > Int32.MaxValue) {
                        throw new InvalidOperationException("Named-pipe security descriptor length is invalid.");
                    }
                    byte[] bytes = new byte[(int)length];
                    Marshal.Copy(securityDescriptor, bytes, 0, (int)length);
                    RawSecurityDescriptor raw = new RawSecurityDescriptor(bytes, 0);
                    if (raw.Owner == null) {
                        throw new InvalidOperationException("Named pipe has no owner SID.");
                    }
                    if (raw.DiscretionaryAcl == null) {
                        throw new InvalidOperationException("Named pipe has no DACL.");
                    }

                    SecurityIdentifier authenticatedUsers = new SecurityIdentifier(WellKnownSidType.AuthenticatedUserSid, null);
                    SecurityIdentifier trustedService = new SecurityIdentifier(trustedServiceSidText);
                    SecurityIdentifier administrators = new SecurityIdentifier(WellKnownSidType.BuiltinAdministratorsSid, null);
                    int auAllow = 0;
                    int auDeny = 0;
                    int trustedServiceAllow = 0;
                    int administratorsAllow = 0;
                    int allowAceCount = 0;
                    int authenticatedUsersAllowAceCount = 0;
                    int trustedServiceAllowAceCount = 0;
                    List<string> offenders = new List<string>();
                    List<string> unexpectedAllows = new List<string>();
                    List<string> unexpectedDenies = new List<string>();
                    List<string> unexpectedAceTypes = new List<string>();

                    foreach (GenericAce genericAce in raw.DiscretionaryAcl) {
                        CommonAce ace = genericAce as CommonAce;
                        if (ace == null) {
                            unexpectedAceTypes.Add(genericAce.AceType.ToString());
                            continue;
                        }
                        if (ace.AceFlags != AceFlags.None || ace.IsCallback || ace.OpaqueLength != 0) {
                            unexpectedAceTypes.Add(String.Format("{0}/flags={1}/callback={2}/opaque={3}", ace.AceType, ace.AceFlags, ace.IsCallback, ace.OpaqueLength));
                            continue;
                        }
                        if (ace.AceQualifier == AceQualifier.AccessDenied) {
                            if (ace.SecurityIdentifier.Equals(authenticatedUsers)) { auDeny |= ace.AccessMask; }
                            unexpectedDenies.Add(ace.SecurityIdentifier.Value);
                            continue;
                        }
                        if (ace.AceQualifier != AceQualifier.AccessAllowed) {
                            unexpectedAceTypes.Add(ace.AceType.ToString());
                            continue;
                        }
                        allowAceCount++;
                        if (ace.SecurityIdentifier.Equals(authenticatedUsers)) {
                            auAllow |= ace.AccessMask;
                            authenticatedUsersAllowAceCount++;
                        }
                        if (ace.SecurityIdentifier.Equals(trustedService)) {
                            trustedServiceAllow |= ace.AccessMask;
                            trustedServiceAllowAceCount++;
                        }
                        if (ace.SecurityIdentifier.Equals(administrators)) {
                            administratorsAllow |= ace.AccessMask;
                        }
                        if (GrantsPipeCreateInstance(ace.AccessMask) &&
                            !ace.SecurityIdentifier.Equals(trustedService)) {
                            offenders.Add(ace.SecurityIdentifier.Value);
                        }
                        if (!ace.SecurityIdentifier.Equals(authenticatedUsers) &&
                            !ace.SecurityIdentifier.Equals(trustedService)) {
                            unexpectedAllows.Add(ace.SecurityIdentifier.Value);
                        }
                    }

                    return new PipeSecurityResult {
                        OwnerSid = raw.Owner.Value,
                        AuthenticatedUsersAllowMask = auAllow,
                        AuthenticatedUsersDenyMask = auDeny,
                        TrustedServiceAllowMask = trustedServiceAllow,
                        AdministratorsAllowMask = administratorsAllow,
                        TrustedServiceCanCreatePipeInstance = GrantsPipeCreateInstance(trustedServiceAllow),
                        DaclProtected = (raw.ControlFlags & ControlFlags.DiscretionaryAclProtected) != 0,
                        AllowAceCount = allowAceCount,
                        AuthenticatedUsersAllowAceCount = authenticatedUsersAllowAceCount,
                        TrustedServiceAllowAceCount = trustedServiceAllowAceCount,
                        OffendingCreateInstanceSids = String.Join(",", offenders.ToArray()),
                        UnexpectedAllowSids = String.Join(",", unexpectedAllows.ToArray()),
                        UnexpectedDenySids = String.Join(",", unexpectedDenies.ToArray()),
                        UnexpectedAceTypes = String.Join(",", unexpectedAceTypes.ToArray())
                    };
                }
                finally {
                    LocalFree(securityDescriptor);
                }
            }
        }
    }
}
'@
}

$result = [AetherCore.NativePipeSecurity]::Inspect($PipePath, $serviceSid)
$servicePid = [uint32]$service.ProcessId
$allowMask = [uint32]$result.AuthenticatedUsersAllowMask
$denyMask = [uint32]$result.AuthenticatedUsersDenyMask
$serviceAllowMask = [uint32]$result.TrustedServiceAllowMask
$administratorsAllowMask = [uint32]$result.AdministratorsAllowMask

if ($result.OwnerSid -ne $serviceSid) {
    throw "Named-pipe owner drift: expected trusted service SID $serviceSid, found $($result.OwnerSid)."
}
if ($allowMask -ne $RequiredClientMask) {
    throw ('Authenticated Users pipe allow mask drift: expected exactly 0x{0:X8}, found 0x{1:X8}.' -f $RequiredClientMask, $allowMask)
}
if ($denyMask -ne 0) {
    throw ('Authenticated Users pipe deny mask drift: expected 0x00000000, found 0x{0:X8}.' -f $denyMask)
}
if (-not $result.TrustedServiceCanCreatePipeInstance) {
    throw ('Trusted service SID cannot create successor pipe instances. mask=0x{0:X8}' -f $serviceAllowMask)
}
if ($administratorsAllowMask -ne 0) {
    throw ('Builtin Administrators must not have a named-pipe allow ACE. mask=0x{0:X8}' -f $administratorsAllowMask)
}
if ($result.AllowAceCount -ne 2 -or $result.AuthenticatedUsersAllowAceCount -ne 1 -or $result.TrustedServiceAllowAceCount -ne 1) {
    throw "Named-pipe DACL ACE cardinality drift: expected exactly one AU allow and one trusted-service allow; total=$($result.AllowAceCount) au=$($result.AuthenticatedUsersAllowAceCount) service=$($result.TrustedServiceAllowAceCount)."
}
if (-not $result.DaclProtected) {
    throw 'Named-pipe DACL is not protected from inheritance.'
}
if (($allowMask -band $CreatePipeInstance) -ne 0) {
    throw ('Authenticated Users can create named-pipe server instances. mask=0x{0:X8}' -f $allowMask)
}
if (-not [string]::IsNullOrWhiteSpace($result.OffendingCreateInstanceSids)) {
    throw "Unauthorized pipe-create-instance ACE(s) detected: $($result.OffendingCreateInstanceSids)"
}
if (-not [string]::IsNullOrWhiteSpace($result.UnexpectedAllowSids)) {
    throw "Unexpected named-pipe allow ACE trustee(s): $($result.UnexpectedAllowSids)"
}
if (-not [string]::IsNullOrWhiteSpace($result.UnexpectedDenySids)) {
    throw "Unexpected named-pipe deny ACE trustee(s): $($result.UnexpectedDenySids)"
}
if (-not [string]::IsNullOrWhiteSpace($result.UnexpectedAceTypes)) {
    throw "Unrecognized named-pipe ACE type(s): $($result.UnexpectedAceTypes)"
}

$evidence = [ordered]@{
    schema = 'aethercore.ipc-pipe-security.v4'
    ok = $true
    pipe = $PipePath
    service = $ServiceName
    service_pid = $servicePid
    service_account = [string]$service.StartName
    service_type = [string]$service.ServiceType
    service_sid_type = 'UNRESTRICTED'
    trusted_service_sid = [string]$serviceSid
    pipe_owner_sid = [string]$result.OwnerSid
    trusted_service_allow_mask = ('0x{0:X8}' -f $serviceAllowMask)
    trusted_service_can_create_pipe_instance = [bool]$result.TrustedServiceCanCreatePipeInstance
    builtin_administrators_allow_mask = ('0x{0:X8}' -f $administratorsAllowMask)
    authenticated_users_allow_mask = ('0x{0:X8}' -f $allowMask)
    authenticated_users_deny_mask = ('0x{0:X8}' -f $denyMask)
    authenticated_users_create_pipe_instance = $false
    dacl_protected = [bool]$result.DaclProtected
    allow_ace_count = [int]$result.AllowAceCount
    authenticated_users_allow_ace_count = [int]$result.AuthenticatedUsersAllowAceCount
    trusted_service_allow_ace_count = [int]$result.TrustedServiceAllowAceCount
    unexpected_allow_sids = @()
    unexpected_deny_sids = @()
    unexpected_ace_types = @()
    unauthorized_create_instance_sids = @()
    client_sqos = 'SECURITY_IDENTIFICATION'
    completed_utc = [DateTimeOffset]::UtcNow.ToString('o')
}
$absoluteOutput = Join-Path $Root $OutputPath
New-Item -ItemType Directory -Force (Split-Path -Parent $absoluteOutput) | Out-Null
$evidence | ConvertTo-Json -Depth 6 | Set-Content $absoluteOutput -Encoding utf8
Write-Host "Native IPC pipe security verification passed. Evidence: $absoluteOutput" -ForegroundColor Green
