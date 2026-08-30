$ErrorActionPreference = 'Continue'
# Simple StandardUser pipe probe: only the raw named-pipe open with the production client mask
# via P/Invoke (fast, no aetherctl state machine) + file/service denials. 60s budget total.
$Label = 'StandardUser'
$outFile = Join-Path $env:TEMP ("p36-probe2-" + $Label + ".txt")
"=== P36 PIPE PROBE v2: $Label ===" | Out-File $outFile -Encoding utf8
"Context: $(whoami)  Time: $(Get-Date -Format o)" | Out-File $outFile -Append -Encoding utf8
$code = @'
using System;
using System.Runtime.InteropServices;
public static class P36Pipe {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr CreateFileW(string name, uint access, uint share, IntPtr sa, uint disposition, uint flags, IntPtr template);
    [DllImport("kernel32.dll")]
    public static extern bool CloseHandle(IntPtr h);
}
'@
Add-Type -TypeDefinition $code
$mask = [uint32]'0x00120003'
$sqos = [uint32]'0x00110000'
$h = [P36Pipe]::CreateFileW('\\.\pipe\AetherCore.Maintenance.v7', $mask, 0, [IntPtr]::Zero, 3, $sqos, [IntPtr]::Zero)
if ($h -eq [IntPtr](-1)) {
    $err = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
    "PIPE_OPEN(exact production mask)=DENIED win32=$err" | Out-File $outFile -Append -Encoding utf8
} else {
    "PIPE_OPEN(exact production mask)=SUCCESS (client can reach service endpoint)" | Out-File $outFile -Append -Encoding utf8
    [P36Pipe]::CloseHandle($h) | Out-Null
}
# write-denial probes (same as before)
foreach ($t in @(
    @('SERVICE_EXE','C:\Program Files\AetherCore\aethercore-maintenance-service.exe'),
    @('TRUST_JSON','C:\Program Files\AetherCore\update-trust.json'),
    @('MUTATION_LOCK','C:\ProgramData\AetherCore\state\machine-mutation.lock'))) {
    try {
        $fs = [System.IO.File]::Open($t[1], 'Open', 'ReadWrite', 'None')
        $fs.Close()
        "$($t[0])_WRITE=SUCCESS (DEFECT)" | Out-File $outFile -Append -Encoding utf8
    } catch {
        "$($t[0])_WRITE=DENIED: $($_.Exception.Message)" | Out-File $outFile -Append -Encoding utf8
    }
}
$stop = & sc.exe stop AetherCoreMaintenance 2>&1 | Out-String
"SC_STOP exit=$LASTEXITCODE" | Out-File $outFile -Append -Encoding utf8
$stop | Out-File $outFile -Append -Encoding utf8
"=== PROBE2 END ===" | Out-File $outFile -Append -Encoding utf8
