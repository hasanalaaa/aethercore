$ErrorActionPreference = 'Continue'
# Read the pipe SD via LSAD/GetNamedPipeInfo alternative: use the SERVICE to dump its own pipe SD?
# Simpler: rebuild a debug-mode service momentarily? NO. Use accesschk-style approach via Win32:
# GetNamedPipeHandleState won't give SD. Use the fact that the SERVER handle is in the service process.
# Instead: use NtQuerySystemInformation? Too deep. Try opening with just SYNCHRONIZE|READ_DATA bits (0x0001|0x0020?) mapping:
# For pipes: GENERIC_READ=0x80000000 maps to FILE_READ_DATA|FILE_READ_ATTRIBUTES|READ_CONTROL...
# FILE_READ_DATA=0x0001, FILE_WRITE_DATA=0x0002, SYNCHRONIZE=0x00100000, READ_CONTROL=0x00020000
# Try each combination to find what SYSTEM CAN do:
$code = @'
using System;
using System.Runtime.InteropServices;
public static class PipeTry {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr CreateFileW(string name, uint access, uint share, IntPtr sa, uint disposition, uint flags, IntPtr template);
    [DllImport("kernel32.dll")]
    public static extern bool CloseHandle(IntPtr h);
}
'@
Add-Type -TypeDefinition $code
$pipe = '\\.\pipe\AetherCore.Maintenance.v7'
$combos = [ordered]@{
  'FILE_READ_DATA (0x1)'                    = [uint32]'0x00000001'
  'FILE_WRITE_DATA (0x2)'                   = [uint32]'0x00000002'
  'SYNCHRONIZE (0x100000)'                  = [uint32]'0x00100000'
  'READ_CONTROL (0x20000)'                  = [uint32]'0x00020000'
  'FILE_READ_ATTRIBUTES (0x80)'             = [uint32]'0x00000080'
  'FILE_READ_DATA|SYNCHRONIZE (0x100001)'   = [uint32]'0x00100001'
  '0x00120003 (client mask)'                = [uint32]'0x00120003'
  '0x00120081 (client mask + read attrs)'   = [uint32]'0x00120081'
  'GENERIC_READ (0x80000000)'               = [uint32]'0x80000000'
  'MAXIMUM_ALLOWED (0x02000000)'            = [uint32]'0x02000000'
}
foreach ($k in $combos.Keys) {
    $h = [PipeTry]::CreateFileW($pipe, $combos[$k], 0, [IntPtr]::Zero, 3, 0, [IntPtr]::Zero)
    if ($h -eq [IntPtr](-1)) {
        $err = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
        Write-Output ("$k -> DENIED (win32 $err)")
    } else {
        Write-Output ("$k -> OK")
        [PipeTry]::CloseHandle($h) | Out-Null
    }
}
