$ErrorActionPreference = 'Continue'
$code = @'
using System;
using System.Runtime.InteropServices;
public static class PipeSd {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr CreateFileW(string name, uint access, uint share, IntPtr sa, uint disposition, uint flags, IntPtr template);
    [DllImport("kernel32.dll")]
    public static extern bool CloseHandle(IntPtr h);
    [DllImport("advapi32.dll", SetLastError = true)]
    public static extern uint GetSecurityInfo(IntPtr handle, int objectType, uint info, IntPtr owner, IntPtr group, out IntPtr dacl, IntPtr sacl, out IntPtr sd);
    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern bool ConvertSecurityDescriptorToStringSecurityDescriptorW(IntPtr sd, uint rev, uint info, out IntPtr str, out uint len);
}
'@
Add-Type -TypeDefinition $code
$pipe = '\\.\pipe\AetherCore.Maintenance.v7'
$READ_CONTROL = [uint32]'0x00020000'
$handle = [PipeSd]::CreateFileW($pipe, $READ_CONTROL, 0, [IntPtr]::Zero, 3, 0, [IntPtr]::Zero)
if ($handle -eq [IntPtr](-1)) {
    $err = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
    Write-Output ("READ_CONTROL open FAILED win32error=" + $err)
    exit
}
Write-Output 'READ_CONTROL open OK - dumping real pipe SD'
$dacl = [IntPtr]::Zero; $sd = [IntPtr]::Zero
$rc = [PipeSd]::GetSecurityInfo($handle, 1, 4, [IntPtr]::Zero, [IntPtr]::Zero, [ref]$dacl, [IntPtr]::Zero, [ref]$sd)  # SE_FILE_OBJECT=1, DACL_SECURITY_INFORMATION=4
Write-Output ("GetSecurityInfo rc=" + $rc)
$str = [IntPtr]::Zero; $len = [uint32]0
$ok = [PipeSd]::ConvertSecurityDescriptorToStringSecurityDescriptorW($sd, 1, 4, [ref]$str, [ref]$len)
if ($ok) {
    $sddl = [Runtime.InteropServices.Marshal]::PtrToStringUni($str, [int]$len)
    Write-Output ("pipe DACL SDDL: " + $sddl)
} else {
    Write-Output ("ConvertSD failed win32error=" + [Runtime.InteropServices.Marshal]::GetLastWin32Error())
}
[PipeSd]::CloseHandle($handle) | Out-Null
