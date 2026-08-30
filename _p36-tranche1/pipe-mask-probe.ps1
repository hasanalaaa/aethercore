$ErrorActionPreference = 'Continue'
# Probe: open the pipe with JUST the exact client access mask via P/Invoke CreateFileW
$code = @'
using System;
using System.Runtime.InteropServices;
public static class PipeOpener {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr CreateFileW(string name, uint access, uint share, IntPtr sa, uint disposition, uint flags, IntPtr template);
    [DllImport("kernel32.dll")]
    public static extern bool CloseHandle(IntPtr h);
    [DllImport("kernel32.dll")]
    public static extern uint GetLastError();
}
'@
Add-Type -TypeDefinition $code
$pipe = '\\.\pipe\AetherCore.Maintenance.v7'
$mask = [uint32]('0x00120003')
# SECURITY_SQOS_PRESENT|SECURITY_IDENTIFICATION = 0x00010000|0x00020000? SECURITY_IDENTIFICATION=0
# SQOS flags go in dwFlagsAndAttributes high byte: SECURITY_SQOS_PRESENT (0x00100000)? Actually 0x00100000? Official: SECURITY_SQOS_PRESENT = 0x00100000, SECURITY_IDENTIFICATION = 0x00020000? No:
# SECURITY_SQOS_PRESENT 0x00100000; SECURITY_IDENTIFICATION 0x00000000 in the QoS field (SECURITY_IDENTIFICATION == 1? no, SECURITY_ANONYMOUS=0, IDENTIFICATION=1, IMPERSONATION=2, DELEGATION=3) - QoS is bits 16-19? Actually SECURITY_SQOS_PRESENT=0x00100000, QoS value shifted: SECURITY_IDENTIFICATION<<16? SECURITY_IDENTIFICATION=1 -> 0x00010000.
$flags = [uint32]('0x00120000')  # SECURITY_SQOS_PRESENT | (SECURITY_IDENTIFICATION(1) << 16) = 0x00110000? let me compute: 0x00100000 | 0x00010000 = 0x00110000
$flags = [uint32]('0x00110000')
$handle = [PipeOpener]::CreateFileW($pipe, $mask, 0, [IntPtr]::Zero, 3, $flags, [IntPtr]::Zero)  # 3 = OPEN_EXISTING
if ($handle -eq [IntPtr](-1)) {
    $err = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
    Write-Output ("OPEN with exact client mask FAILED win32error=" + $err)
} else {
    Write-Output 'OPEN with exact client mask = SUCCESS'
    [PipeOpener]::CloseHandle($handle) | Out-Null
}
# Also try with generic read
$handle2 = [PipeOpener]::CreateFileW($pipe, [uint32]('0x80000000'), 0, [IntPtr]::Zero, 3, $flags, [IntPtr]::Zero)
if ($handle2 -eq [IntPtr](-1)) {
    $err2 = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
    Write-Output ("OPEN with GENERIC_READ FAILED win32error=" + $err2)
} else {
    Write-Output 'OPEN with GENERIC_READ = SUCCESS'
    [PipeOpener]::CloseHandle($handle2) | Out-Null
}
