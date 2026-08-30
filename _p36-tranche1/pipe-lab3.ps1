$ErrorActionPreference = 'Continue'
$code = @'
using System;
using System.Runtime.InteropServices;
public static class PipeLab3 {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr CreateNamedPipeW(string name, uint openMode, uint pipeMode, uint maxInst, uint outBuf, uint inBuf, uint defTimeout, IntPtr sa);
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

function New-LabPipe([string]$name, [string]$sddl) {
    $sd = [IntPtr]::Zero; $sz = [uint32]0
    $ok = [PipeLab3]::ConvertStringSecurityDescriptorToSecurityDescriptorW($sddl, 1, [ref]$sd, [ref]$sz)
    if (-not $ok) { Write-Output ("convert failed " + [Runtime.InteropServices.Marshal]::GetLastWin32Error()); return [IntPtr]::Zero }
    $sa = New-Object PipeLab3+SECURITY_ATTRIBUTES
    $sa.nLength = [uint32][Runtime.InteropServices.Marshal]::SizeOf($sa)
    $sa.lpSecurityDescriptor = $sd
    $sa.bInheritHandle = 0
    $ptr = [Runtime.InteropServices.Marshal]::AllocHGlobal([Runtime.InteropServices.Marshal]::SizeOf($sa))
    [Runtime.InteropServices.Marshal]::StructureToPtr($sa, $ptr, $false)
    $server = [PipeLab3]::CreateNamedPipeW($name, [uint32]'0x00000003', [uint32]'0x00000008', [uint32]255, [uint32]65536, [uint32]65536, [uint32]0, $ptr)
    if ($server -eq [IntPtr](-1)) { Write-Output ("CreateNamedPipeW failed " + [Runtime.InteropServices.Marshal]::GetLastWin32Error()); return [IntPtr]::Zero }
    return $server
}
function Get-Sd([IntPtr]$handle, [string]$label) {
    $dacl = [IntPtr]::Zero; $sd = [IntPtr]::Zero
    $rc = [PipeLab3]::GetSecurityInfo($handle, 1, 4, [IntPtr]::Zero, [IntPtr]::Zero, [ref]$dacl, [IntPtr]::Zero, [ref]$sd)
    if ($rc -ne 0) { Write-Output ("$label GetSecurityInfo rc=" + $rc); return }
    $str = [IntPtr]::Zero; $len = [uint32]0
    $ok = [PipeLab3]::ConvertSecurityDescriptorToStringSecurityDescriptorW($sd, 1, 4, [ref]$str, [ref]$len)
    if ($ok) {
        $sddl = [Runtime.InteropServices.Marshal]::PtrToStringUni($str, [int]$len)
        Write-Output ("$label DACL: " + $sddl)
    } else { Write-Output ("$label SD->SDDL failed") }
}

# pipe with the EXACT production AU ACE
$sid = (& sc.exe showsid AetherCoreMaintenance | Select-String 'S-1-5-80').ToString().Split(':')[-1].Trim()
$pB = New-LabPipe '\\.\pipe\P36LabD.v1' "D:P(A;;GA;;;$sid)(A;;0x00120003;;;AU)"
if ($pB -ne [IntPtr]::Zero) {
    Get-Sd $pB 'pipeD(prod AU specific)'
    [PipeLab3]::CloseHandle($pB) | Out-Null
}
# pipe with AU GA for comparison
$pC = New-LabPipe '\\.\pipe\P36LabE.v1' "D:P(A;;GA;;;$sid)(A;;GA;;;AU)"
if ($pC -ne [IntPtr]::Zero) {
    Get-Sd $pC 'pipeE(AU GA)'
    [PipeLab3]::CloseHandle($pC) | Out-Null
}
