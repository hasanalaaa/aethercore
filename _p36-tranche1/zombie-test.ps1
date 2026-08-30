$ErrorActionPreference = 'Continue'
Write-Output '=== aethercore processes right now ==='
Get-Process -Name 'aethercore-maintenance-service' -ErrorAction SilentlyContinue | ForEach-Object { Write-Output ("pid=" + $_.Id + " start=" + $_.StartTime + " path=" + $_.Path) }
Write-Output '=== pipe exists while service RUNNING? ==='
$exists = [System.IO.Directory]::GetFiles('\\.\pipe\') | Where-Object { $_ -like '*AetherCore*' }
Write-Output ($exists -join ', ')
Write-Output '=== stop service ==='
& sc.exe stop AetherCoreMaintenance 2>&1 | Out-String | Write-Output
Start-Sleep 5
Write-Output '=== processes after stop ==='
Get-Process -Name 'aethercore-maintenance-service' -ErrorAction SilentlyContinue | ForEach-Object { Write-Output ("pid=" + $_.Id + " start=" + $_.StartTime) }
if (-not (Get-Process -Name 'aethercore-maintenance-service' -ErrorAction SilentlyContinue)) { Write-Output '(no service processes)' }
Write-Output '=== pipe exists while service STOPPED? ==='
$exists2 = [System.IO.Directory]::GetFiles('\\.\pipe\') | Where-Object { $_ -like '*AetherCore*' }
if ($exists2) { Write-Output ("STILL PRESENT: " + ($exists2 -join ', ')) } else { Write-Output 'gone (service owned it)' }
Write-Output '=== restart service ==='
& sc.exe start AetherCoreMaintenance 2>&1 | Out-String | Write-Output
Start-Sleep 6
& sc.exe query AetherCoreMaintenance 2>&1 | Select-String 'STATE' | ForEach-Object { Write-Output $_.Line.Trim() }
$exists3 = [System.IO.Directory]::GetFiles('\\.\pipe\') | Where-Object { $_ -like '*AetherCore*' }
Write-Output ("pipe after restart: " + ($exists3 -join ', '))
Write-Output '=== immediate pipe open probe with client mask ==='
$code = @'
using System;
using System.Runtime.InteropServices;
public static class PipeTry2 {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr CreateFileW(string name, uint access, uint share, IntPtr sa, uint disposition, uint flags, IntPtr template);
    [DllImport("kernel32.dll")]
    public static extern bool CloseHandle(IntPtr h);
}
'@
Add-Type -TypeDefinition $code
$h = [PipeTry2]::CreateFileW('\\.\pipe\AetherCore.Maintenance.v7', [uint32]'0x00120003', 0, [IntPtr]::Zero, 3, [uint32]'0x00110000', [IntPtr]::Zero)
if ($h -eq [IntPtr](-1)) {
    Write-Output ("open FAILED win32=" + [Runtime.InteropServices.Marshal]::GetLastWin32Error())
} else {
    Write-Output 'open OK'
    [PipeTry2]::CloseHandle($h) | Out-Null
}
