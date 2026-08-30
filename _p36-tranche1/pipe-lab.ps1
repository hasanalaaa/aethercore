$ErrorActionPreference = 'Continue'
$code = @'
using System;
using System.Runtime.InteropServices;
public static class PipeLab {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr CreateNamedPipeW(string name, uint openMode, uint pipeMode, uint maxInst, uint outBuf, uint inBuf, uint defTimeout, IntPtr sa);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr CreateFileW(string name, uint access, uint share, IntPtr sa, uint disposition, uint flags, IntPtr template);
    [DllImport("kernel32.dll")]
    public static extern bool CloseHandle(IntPtr h);
    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern bool ConvertStringSecurityDescriptorToSecurityDescriptorW(string sddl, uint rev, out IntPtr sd, out uint size);
    [StructLayout(LayoutKind.Sequential)]
    public struct SECURITY_ATTRIBUTES {
        public uint nLength;
        public IntPtr lpSecurityDescriptor;
        public int bInheritHandle;
    }
}
'@
Add-Type -TypeDefinition $code

# production DACL copied from crates/ipc/src/windows_impl.rs with the LIVE service SID
# (owner left at creator default: setting owner=service-SID is owner-token-only by design)
$sid = (& sc.exe showsid AetherCoreMaintenance | Select-String 'S-1-5-80').ToString().Split(':')[-1].Trim()
Write-Output ("service SID: " + $sid)
$sddl = "D:P(A;;GA;;;$sid)(A;;0x00120003;;;AU)"
Write-Output ("test SDDL:   " + $sddl)
$sd = [IntPtr]::Zero; $sz = [uint32]0
$ok = [PipeLab]::ConvertStringSecurityDescriptorToSecurityDescriptorW($sddl, 1, [ref]$sd, [ref]$sz)
if (-not $ok) { Write-Output ("SDD convert failed: " + [Runtime.InteropServices.Marshal]::GetLastWin32Error()); exit }
$sa = New-Object PipeLab+SECURITY_ATTRIBUTES
$sa.nLength = [uint32][Runtime.InteropServices.Marshal]::SizeOf($sa)
$sa.lpSecurityDescriptor = $sd
$sa.bInheritHandle = 0
$ptr = [Runtime.InteropServices.Marshal]::AllocHGlobal([Runtime.InteropServices.Marshal]::SizeOf($sa))
[Runtime.InteropServices.Marshal]::StructureToPtr($sa, $ptr, $false)

# PIPE_TYPE_BYTE|READMODE_BYTE|WAIT|REJECT_REMOTE = 0x4|0x0|0x0|0x8 = 0xC? compute: PIPE_TYPE_BYTE=0x0? (BYTE=0), READMODE_BYTE=0x0, WAIT=0x0, REJECT_REMOTE_CLIENTS=0x8 -> 0x8
$pipeMode = [uint32]'0x00000008'
$openMode = [uint32]'0x00000003'  # PIPE_ACCESS_DUPLEX (no FIRST_INSTANCE)
$server = [PipeLab]::CreateNamedPipeW('\\.\pipe\P36Lab.v1', $openMode, $pipeMode, [uint32]255, [uint32]65536, [uint32]65536, [uint32]0, $ptr)
if ($server -eq [IntPtr](-1)) {
    Write-Output ("CreateNamedPipeW failed: " + [Runtime.InteropServices.Marshal]::GetLastWin32Error())
    exit
}
Write-Output 'test pipe created (server listening handle, not yet connected)'

# client opens with the production client mask + SQOS while server instance exists
$mask = [uint32]'0x00120003'
$sqos = [uint32]'0x00110000'
$h = [PipeLab]::CreateFileW('\\.\pipe\P36Lab.v1', $mask, 0, [IntPtr]::Zero, 3, $sqos, [IntPtr]::Zero)
if ($h -eq [IntPtr](-1)) {
    Write-Output ("client open DENIED win32=" + [Runtime.InteropServices.Marshal]::GetLastWin32Error())
} else {
    Write-Output 'client open OK on identically-SD test pipe'
    [PipeLab]::CloseHandle($h) | Out-Null
}
# cleanup: close server + delete
[PipeLab]::CloseHandle($server) | Out-Null
[System.IO.Directory]::GetFiles('\\.\pipe\') | Where-Object { $_ -like '*P36Lab*' } | ForEach-Object { Write-Output ("still present: " + $_) }
