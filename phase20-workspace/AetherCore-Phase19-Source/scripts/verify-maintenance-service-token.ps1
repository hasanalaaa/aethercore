#Requires -RunAsAdministrator
[CmdletBinding()]
param(
    [string]$ServiceName = 'AetherCoreMaintenance',
    [string]$OutputPath = 'out\ga-evidence\maintenance-service-token.json'
)
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $Root
if ($env:OS -ne 'Windows_NT') { throw 'Maintenance service token verification requires Windows.' }

$service = Get-CimInstance Win32_Service -Filter "Name='$ServiceName'"
if (-not $service) { throw "$ServiceName is not installed." }
if ($service.State -ne 'Running' -or [uint32]$service.ProcessId -eq 0) {
    throw "$ServiceName must be running before its process token can be verified."
}
if ($service.StartName -notin @('LocalSystem','Local System')) {
    throw "$ServiceName account drift: expected LocalSystem, got $($service.StartName)."
}

$sidType = (& "$env:SystemRoot\System32\sc.exe" qsidtype $ServiceName | Out-String)
if ($LASTEXITCODE -ne 0 -or $sidType -notmatch 'UNRESTRICTED') {
    throw "$ServiceName SID-type drift: expected SERVICE_SID_TYPE_UNRESTRICTED. sc.exe output: $sidType"
}

$expectedSid = ([System.Security.Principal.NTAccount]::new("NT SERVICE\$ServiceName")).Translate([System.Security.Principal.SecurityIdentifier]).Value

$native = @'
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;

public static class AetherCoreServiceTokenProbe {
    private const uint PROCESS_QUERY_LIMITED_INFORMATION = 0x1000;
    private const uint TOKEN_QUERY = 0x0008;
    private const int TokenGroups = 2;
    private const int TokenRestrictedSids = 11;

    [StructLayout(LayoutKind.Sequential)]
    private struct SID_AND_ATTRIBUTES {
        public IntPtr Sid;
        public uint Attributes;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct TOKEN_GROUPS_ONE {
        public uint GroupCount;
        public SID_AND_ATTRIBUTES Groups;
    }

    public sealed class GroupInfo {
        public string Sid { get; set; }
        public uint Attributes { get; set; }
    }

    public sealed class TokenEvidence {
        public GroupInfo[] Groups { get; set; }
        public GroupInfo[] RestrictedSids { get; set; }
    }

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern IntPtr OpenProcess(uint dwDesiredAccess, bool bInheritHandle, uint dwProcessId);

    [DllImport("advapi32.dll", SetLastError = true)]
    private static extern bool OpenProcessToken(IntPtr ProcessHandle, uint DesiredAccess, out IntPtr TokenHandle);

    [DllImport("advapi32.dll", SetLastError = true)]
    private static extern bool GetTokenInformation(IntPtr TokenHandle, int TokenInformationClass, IntPtr TokenInformation, uint TokenInformationLength, out uint ReturnLength);

    [DllImport("advapi32.dll", SetLastError = true)]
    private static extern bool ConvertSidToStringSid(IntPtr Sid, out IntPtr StringSid);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern IntPtr LocalFree(IntPtr hMem);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool CloseHandle(IntPtr hObject);

    private static void ThrowLast(string operation) {
        throw new Win32Exception(Marshal.GetLastWin32Error(), operation);
    }

    private static string SidString(IntPtr sid) {
        if (sid == IntPtr.Zero) throw new InvalidOperationException("Token contains a null SID pointer.");
        IntPtr text = IntPtr.Zero;
        if (!ConvertSidToStringSid(sid, out text)) ThrowLast("ConvertSidToStringSid failed");
        try {
            string value = Marshal.PtrToStringUni(text);
            if (value == null) throw new InvalidOperationException("ConvertSidToStringSid returned null text.");
            return value;
        } finally {
            if (text != IntPtr.Zero) LocalFree(text);
        }
    }

    private static GroupInfo[] ReadGroups(IntPtr token, int infoClass) {
        uint required = 0;
        bool sized = GetTokenInformation(token, infoClass, IntPtr.Zero, 0, out required);
        int firstError = Marshal.GetLastWin32Error();
        if (!sized && firstError != 122) throw new Win32Exception(firstError, "GetTokenInformation size query failed");
        if (required == 0) throw new InvalidOperationException("GetTokenInformation returned a zero-sized group buffer.");
        if (required > 1024 * 1024) throw new InvalidOperationException("Token group buffer exceeds 1 MiB safety bound.");

        IntPtr buffer = Marshal.AllocHGlobal(checked((int)required));
        try {
            uint actual;
            if (!GetTokenInformation(token, infoClass, buffer, required, out actual)) ThrowLast("GetTokenInformation failed");
            if (actual > required) throw new InvalidOperationException("Token information length grew beyond allocated buffer.");

            uint count = unchecked((uint)Marshal.ReadInt32(buffer));
            if (count > 16384) throw new InvalidOperationException("Token group count exceeds safety bound.");
            int entrySize = Marshal.SizeOf(typeof(SID_AND_ATTRIBUTES));
            int groupOffset = checked((int)Marshal.OffsetOf(typeof(TOKEN_GROUPS_ONE), "Groups"));
            long requiredEntries = checked((long)groupOffset + checked((long)count * entrySize));
            if (requiredEntries > actual) throw new InvalidOperationException("TOKEN_GROUPS entries exceed returned buffer length.");

            var result = new List<GroupInfo>(checked((int)count));
            for (uint i = 0; i < count; i++) {
                IntPtr entryPtr = IntPtr.Add(buffer, checked(groupOffset + checked((int)i * entrySize)));
                var entry = (SID_AND_ATTRIBUTES)Marshal.PtrToStructure(entryPtr, typeof(SID_AND_ATTRIBUTES));
                result.Add(new GroupInfo { Sid = SidString(entry.Sid), Attributes = entry.Attributes });
            }
            return result.ToArray();
        } finally {
            Marshal.FreeHGlobal(buffer);
        }
    }

    public static TokenEvidence Inspect(uint processId) {
        IntPtr process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, processId);
        if (process == IntPtr.Zero) ThrowLast("OpenProcess failed");
        IntPtr token = IntPtr.Zero;
        try {
            if (!OpenProcessToken(process, TOKEN_QUERY, out token)) ThrowLast("OpenProcessToken failed");
            return new TokenEvidence {
                Groups = ReadGroups(token, TokenGroups),
                RestrictedSids = ReadGroups(token, TokenRestrictedSids)
            };
        } finally {
            if (token != IntPtr.Zero) CloseHandle(token);
            CloseHandle(process);
        }
    }
}
'@

if (-not ('AetherCoreServiceTokenProbe' -as [type])) { Add-Type -TypeDefinition $native -Language CSharp }
$token = [AetherCoreServiceTokenProbe]::Inspect([uint32]$service.ProcessId)
$serviceGroup = @($token.Groups | Where-Object { $_.Sid -eq $expectedSid })
if ($serviceGroup.Count -ne 1) {
    throw "Expected service SID $expectedSid exactly once in process token; found $($serviceGroup.Count)."
}

$SE_GROUP_ENABLED_BY_DEFAULT = [uint32]0x00000002
$SE_GROUP_OWNER = [uint32]0x00000008
$attrs = [uint32]$serviceGroup[0].Attributes
if (($attrs -band $SE_GROUP_ENABLED_BY_DEFAULT) -eq 0) { throw 'Service SID is not marked SE_GROUP_ENABLED_BY_DEFAULT.' }
if (($attrs -band $SE_GROUP_OWNER) -eq 0) { throw 'Service SID is not marked SE_GROUP_OWNER.' }
if (@($token.RestrictedSids).Count -ne 0) {
    $restricted = @($token.RestrictedSids | ForEach-Object { $_.Sid }) -join ', '
    throw "Maintenance service token unexpectedly contains restricting SIDs: $restricted"
}

$out = Join-Path $Root $OutputPath
New-Item -ItemType Directory -Force (Split-Path $out -Parent) | Out-Null
$evidence = [ordered]@{
    schema = 'aethercore.maintenance-service-token.v1'
    ok = $true
    service = $ServiceName
    process_id = [uint32]$service.ProcessId
    account = $service.StartName
    service_sid_type = 'UNRESTRICTED'
    service_sid = $expectedSid
    service_sid_attributes = ('0x{0:X8}' -f $attrs)
    service_sid_enabled_by_default = $true
    service_sid_owner_capable = $true
    restricting_sid_count = 0
    token_group_count = @($token.Groups).Count
    completed_utc = [DateTimeOffset]::UtcNow.ToString('o')
}
$evidence | ConvertTo-Json -Depth 5 | Set-Content $out -Encoding utf8
Write-Host "Maintenance service token verification passed. Evidence: $out" -ForegroundColor Green
