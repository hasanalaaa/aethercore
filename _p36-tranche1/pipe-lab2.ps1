$ErrorActionPreference = 'Continue'
$code = @'
using System;
using System.Runtime.InteropServices;
public static class PipeLab2 {
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

function New-LabPipe($name, $sddl) {
    $sd = [IntPtr]::Zero; $sz = [uint32]0
    $ok = [PipeLab2]::ConvertStringSecurityDescriptorToSecurityDescriptorW($sddl, 1, [ref]$sd, [ref]$sz)
    if (-not $ok) { Write-Output ("SDD convert failed: " + [Runtime.InteropServices.Marshal]::GetLastWin32Error()); return $null }
    $sa = New-Object PipeLab2+SECURITY_ATTRIBUTES
    $sa.nLength = [uint32][Runtime.InteropServices.Marshal]::SizeOf($sa)
    $sa.lpSecurityDescriptor = $sd
    $sa.bInheritHandle = 0
    $ptr = [Runtime.InteropServices.Marshal]::AllocHGlobal([Runtime.InteropServices.Marshal]::SizeOf($sa))
    [Runtime.InteropServices.Marshal]::StructureToPtr($sa, $ptr, $false)
    $server = [PipeLab2]::CreateNamedPipeW($name, [uint32]'0x00000003', [uint32]'0x00000008', [uint32]255, [uint32]65536, [uint32]65536, [uint32]0, $ptr)
    if ($server -eq [IntPtr](-1)) { Write-Output ("CreateNamedPipeW $name failed: " + [Runtime.InteropServices.Marshal]::GetLastWin32Error()); return $null }
    return $server
}
function Try-Open($name, $mask, $label) {
    $h = [PipeLab2]::CreateFileW($name, $mask, 0, [IntPtr]::Zero, 3, [uint32]'0x00110000', [IntPtr]::Zero)
    if ($h -eq [IntPtr](-1)) { Write-Output ("  $label -> DENIED win32=" + [Runtime.InteropServices.Marshal]::GetLastWin32Error()) }
    else { Write-Output ("  $label -> OK"); [PipeLab2]::CloseHandle($h) | Out-Null }
}

# PIPE A: dev SDDL (SY GA, BA GA, AU client mask)
$devSddl = 'D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;0x00120003;;;AU)'
$pA = New-LabPipe '\\.\pipe\P36LabA.v1' $devSddl
Write-Output ("pipe A (dev sddl): " + $(if ($pA) { 'created' } else { 'FAILED' }))

# PIPE B: production DACL (service SID GA, AU client mask) - NO SYSTEM/BA grant
$sid = (& sc.exe showsid AetherCoreMaintenance | Select-String 'S-1-5-80').ToString().Split(':')[-1].Trim()
$prodSddl = "D:P(A;;GA;;;$sid)(A;;0x00120003;;;AU)"
$pB = New-LabPipe '\\.\pipe\P36LabB.v1' $prodSddl
Write-Output ("pipe B (prod dacl): " + $(if ($pB) { 'created' } else { 'FAILED' }))

# PIPE C: production DACL + extra AU GA (should definitely allow if AU works at all)
$prodC = "D:P(A;;GA;;;$sid)(A;;GA;;;AU)"
$pC = New-LabPipe '\\.\pipe\P36LabC.v1' $prodC
Write-Output ("pipe C (AU GA):     " + $(if ($pC) { 'created' } else { 'FAILED' }))

Write-Output '--- client opens (SYSTEM context) ---'
if ($pA) {
    Try-Open '\\.\pipe\P36LabA.v1' ([uint32]'0x00120003') 'A exact mask'
    Try-Open '\\.\pipe\P36LabA.v1' ([uint32]'0x80000000') 'A GENERIC_READ'
}
if ($pB) {
    Try-Open '\\.\pipe\P36LabB.v1' ([uint32]'0x00120003') 'B exact mask'
    Try-Open '\\.\pipe\P36LabB.v1' ([uint32]'0x80000000') 'B GENERIC_READ'
}
if ($pC) {
    Try-Open '\\.\pipe\P36LabC.v1' ([uint32]'0x00120003') 'C exact mask'
    Try-Open '\\.\pipe\P36LabC.v1' ([uint32]'0x80000000') 'C GENERIC_READ'
}
foreach ($h in @($pA, $pB, $pC)) { if ($h) { [PipeLab2]::CloseHandle($h) | Out-Null } }
