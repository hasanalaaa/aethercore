$ErrorActionPreference = 'Continue'
$code = @'
using System;
using System.Runtime.InteropServices;
public static class PipeLab4 {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr CreateNamedPipeW(string name, uint openMode, uint pipeMode, uint maxInst, uint outBuf, uint inBuf, uint defTimeout, IntPtr sa);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr CreateFileW(string name, uint access, uint share, IntPtr sa, uint disposition, uint flags, IntPtr template);
    [DllImport("kernel32.dll")]
    public static extern bool CloseHandle(IntPtr h);
    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern bool ConvertStringSecurityDescriptorToSecurityDescriptorW(string sddl, uint rev, out IntPtr sd, out uint size);
    [DllImport("advapi32.dll", SetLastError = true)]
    public static extern uint GetSecurityInfo(IntPtr handle, int objectType, uint info, IntPtr owner, IntPtr group, out IntPtr dacl, IntPtr sacl, out IntPtr sd);
    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern bool ConvertSecurityDescriptorToStringSecurityDescriptorW(IntPtr sd, uint rev, uint info, out IntPtr str, out uint len);
    [StructLayout(LayoutKind.Sequential)]
    public struct SECURITY_ATTRIBUTES {
        public uint nLength;
        public IntPtr lpSecurityDescriptor;
        public int bInheritHandle;
    }
}
'@
Add-Type -TypeDefinition $code
$sid = (& sc.exe showsid AetherCoreMaintenance | Select-String 'S-1-5-80').ToString().Split(':')[-1].Trim()

function New-LabPipe([string]$name, [string]$sddl) {
    $sd = [IntPtr]::Zero; $sz = [uint32]0
    $ok = [PipeLab4]::ConvertStringSecurityDescriptorToSecurityDescriptorW($sddl, 1, [ref]$sd, [ref]$sz)
    if (-not $ok) { Write-Output ("convert failed " + [Runtime.InteropServices.Marshal]::GetLastWin32Error()); return [IntPtr]::Zero }
    $sa = New-Object PipeLab4+SECURITY_ATTRIBUTES
    $sa.nLength = [uint32][Runtime.InteropServices.Marshal]::SizeOf($sa)
    $sa.lpSecurityDescriptor = $sd
    $sa.bInheritHandle = 0
    $ptr = [Runtime.InteropServices.Marshal]::AllocHGlobal([Runtime.InteropServices.Marshal]::SizeOf($sa))
    [Runtime.InteropServices.Marshal]::StructureToPtr($sa, $ptr, $false)
    return [PipeLab4]::CreateNamedPipeW($name, [uint32]'0x00000003', [uint32]'0x00000008', [uint32]255, [uint32]65536, [uint32]65536, [uint32]0, $ptr)
}
function Show-Dacl([IntPtr]$handle, [string]$label) {
    $dacl = [IntPtr]::Zero; $sd2 = [IntPtr]::Zero
    $rc = [PipeLab4]::GetSecurityInfo($handle, 1, 4, [IntPtr]::Zero, [IntPtr]::Zero, [ref]$dacl, [IntPtr]::Zero, [ref]$sd2)
    if ($rc -ne 0) { Write-Output ("$label GetSecurityInfo rc=" + $rc); return }
    $str = [IntPtr]::Zero; $len = [uint32]0
    $ok = [PipeLab4]::ConvertSecurityDescriptorToStringSecurityDescriptorW($sd2, 1, 4, [ref]$str, [ref]$len)
    if ($ok) { $s = [Runtime.InteropServices.Marshal]::PtrToStringUni($str, [int]$len); Write-Output ("$label DACL: " + ($s -replace "`0", '')) }
}
function Try-Open([string]$name, [uint32]$mask, [string]$label) {
    $h = [PipeLab4]::CreateFileW($name, $mask, 0, [IntPtr]::Zero, 3, [uint32]'0x00110000', [IntPtr]::Zero)
    if ($h -eq [IntPtr](-1)) { Write-Output ("  $label -> DENIED win32=" + [Runtime.InteropServices.Marshal]::GetLastWin32Error()); return $false }
    Write-Output ("  $label -> OK"); [PipeLab4]::CloseHandle($h) | Out-Null; return $true
}

# CANDIDATE FIX: FR (FILE_GENERIC_READ, includes SYNCHRONIZE+READ_CONTROL) + hex 0x2 (WRITE_DATA only)
# => AU effective: 0x12008B; client mask 0x00120003 fully covered; NO create-instance bit (0x4)
$cand = "D:P(A;;GA;;;$sid)(A;;FR;;;AU)(A;;0x00000002;;;AU)"
Write-Output ("candidate SDDL: " + $cand)
$h1 = New-LabPipe '\\.\pipe\P36LabF.v1' $cand
if ($h1 -ne [IntPtr]::Zero) {
    Show-Dacl $h1 'pipeF'
    Try-Open '\\.\pipe\P36LabF.v1' ([uint32]'0x00120003') 'F exact client mask'
    Try-Open '\\.\pipe\P36LabF.v1' ([uint32]'0x00120007') 'F mask WITH create-instance (must be DENIED)'
    [PipeLab4]::CloseHandle($h1) | Out-Null
}
