# P36 T1 - service-side doctor observation: log a marker before/after each doctor request.
# Correlate client timestamps with service JSON logs per the workstream instruction.
$ErrorActionPreference = 'Continue'
$cli = 'C:\Program Files\AetherCore\aetherctl.exe'
$log = 'C:\ProgramData\AetherCore\logs\service.jsonl'
$before = (Get-Item $log).Length
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $cli
$psi.Arguments = '--output json doctor'
$psi.UseShellExecute = $false
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$p = [System.Diagnostics.Process]::Start($psi)
$fin = $p.WaitForExit(45000)
$sw.Stop()
if ($fin) {
    Write-Output ("doctor exit=" + $p.ExitCode + " dur=" + $sw.ElapsedMilliseconds + "ms")
    $out = $p.StandardOutput.ReadToEnd()
    Write-Output ("stdout(first 300): " + $out.Substring(0, [Math]::Min(300, $out.Length)))
} else {
    try { $p.Kill(); $p.WaitForExit(5000) | Out-Null } catch {}
    Write-Output ("doctor TIMED OUT " + $sw.ElapsedMilliseconds + "ms")
}
Start-Sleep 2
Write-Output '=== service log entries during the doctor window ==='
# print entries newer than 60s ago
$cutoff = (Get-Date).ToUniversalTime().AddSeconds(-70)
Get-Content $log | ForEach-Object {
    if ($_ -match '"timestamp":"([^"]+)"') {
        $t = [datetime]::Parse($Matches[1].Replace('Z',''))
        if ($t -ge (Get-Date).AddSeconds(-70).ToUniversalTime()) { Write-Output $_ }
    }
}
