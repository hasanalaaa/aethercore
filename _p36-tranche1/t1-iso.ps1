$ErrorActionPreference = 'Continue'
Write-Output '=== A: does a second ipc_probe session CONNECT after first doctor client disconnects? ==='
# Test: single ipc_probe (fresh session) while NO doctor is running
& 'C:\Program Files\AetherCore\ipc_probe.exe' 2>&1 | Select-Object -First 1 | ForEach-Object { Write-Output ("standalone probe: " + $_) }
Write-Output '=== B: does doctor hang EVEN when service was just restarted (fresh session table)? ==='
& sc.exe stop AetherCoreMaintenance 2>&1 | Out-Null
Start-Sleep 4
& sc.exe start AetherCoreMaintenance 2>&1 | Out-Null
Start-Sleep 6
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = 'C:\Program Files\AetherCore\aetherctl.exe'
$psi.Arguments = '--output json doctor'
$psi.UseShellExecute = $false
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$p = [System.Diagnostics.Process]::Start($psi)
$fin = $p.WaitForExit(25000)
if ($fin) {
    $sw.Stop()
    $out = $p.StandardOutput.ReadToEnd()
    Write-Output ("fresh-restart doctor exit=" + $p.ExitCode + " dur=" + $sw.ElapsedMilliseconds + "ms")
    Write-Output ("stdout(first 250): " + $out.Substring(0, [Math]::Min(250, $out.Length)))
} else {
    try { $p.Kill(); $p.WaitForExit(5000) | Out-Null } catch {}
    Write-Output ("fresh-restart doctor HUNG at " + $sw.ElapsedMilliseconds + "ms")
}
