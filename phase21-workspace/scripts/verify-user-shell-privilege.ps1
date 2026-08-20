[CmdletBinding()]
param(
    [string]$InstallDirectory = (Join-Path $env:ProgramW6432 'AetherCore')
)
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'User-shell privilege verification requires Windows.' }

$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Run this privilege check from a normal, non-elevated user session. It intentionally refuses an elevated PowerShell host.'
}

$InstallDirectory = (Resolve-Path $InstallDirectory).Path
$desktop = Join-Path $InstallDirectory 'aethercore-desktop.exe'
$broker = Join-Path $InstallDirectory 'aethercore-consent-broker.exe'
foreach ($file in @($desktop,$broker)) {
    if (-not (Test-Path $file -PathType Leaf)) { throw "Installed artifact is missing: $file" }
}

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class AetherCoreTokenProbe {
    const uint TOKEN_QUERY = 0x0008;
    const int TokenElevation = 20;
    [StructLayout(LayoutKind.Sequential)]
    public struct TOKEN_ELEVATION { public int TokenIsElevated; }
    [DllImport("advapi32.dll", SetLastError=true)]
    static extern bool OpenProcessToken(IntPtr ProcessHandle, uint DesiredAccess, out IntPtr TokenHandle);
    [DllImport("advapi32.dll", SetLastError=true)]
    static extern bool GetTokenInformation(IntPtr TokenHandle, int TokenInformationClass, out TOKEN_ELEVATION TokenInformation, int TokenInformationLength, out int ReturnLength);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern bool CloseHandle(IntPtr hObject);
    public static bool IsElevated(IntPtr processHandle) {
        IntPtr token;
        if (!OpenProcessToken(processHandle, TOKEN_QUERY, out token)) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
        try {
            TOKEN_ELEVATION elevation;
            int returned;
            int size = Marshal.SizeOf<TOKEN_ELEVATION>();
            if (!GetTokenInformation(token, TokenElevation, out elevation, size, out returned)) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
            return elevation.TokenIsElevated != 0;
        } finally { CloseHandle(token); }
    }
}
'@

$process = $null
try {
    $process = Start-Process -FilePath $desktop -PassThru
    Start-Sleep -Milliseconds 800
    if ($process.HasExited) { throw "Desktop shell exited before token verification (exit code $($process.ExitCode))." }
    if ([AetherCoreTokenProbe]::IsElevated($process.Handle)) {
        throw 'Desktop shell unexpectedly owns an elevated token.'
    }

    $svc = Get-CimInstance Win32_Service -Filter "Name='AetherCoreMaintenance'"
    if (-not $svc) { throw 'AetherCoreMaintenance service is not installed.' }
    if ($svc.StartName -notin @('LocalSystem','Local System')) { throw "Maintenance service account drift: $($svc.StartName)" }

    Write-Host 'Privilege separation passed: desktop runs non-elevated; privileged maintenance remains in the LocalSystem service/UAC broker boundary.' -ForegroundColor Green
} finally {
    if ($process -and -not $process.HasExited) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
    }
}
