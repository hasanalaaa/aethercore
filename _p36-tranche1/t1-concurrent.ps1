$ErrorActionPreference = 'Continue'
Write-Output '=== concurrent ipc_probe during doctor hang ==='
# Start doctor in a background process
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = 'C:\Program Files\AetherCore\aetherctl.exe'
$psi.Arguments = '--output json doctor'
$psi.UseShellExecute = $false
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$p = [System.Diagnostics.Process]::Start($psi)
Start-Sleep 3
Write-Output '--- second client: ipc_probe connect (tests listener responsiveness) ---'
$psi2 = New-Object System.Diagnostics.ProcessStartInfo
$psi2.FileName = 'C:\Program Files\AetherCore\ipc_probe.exe'
$psi2.UseShellExecute = $false
$psi2.RedirectStandardOutput = $true
$psi2.RedirectStandardError = $true
$sw2 = [System.Diagnostics.Stopwatch]::StartNew()
$p2 = [System.Diagnostics.Process]::Start($psi2)
$fin2 = $p2.WaitForExit(15000)
$sw2.Stop()
if ($fin2) {
    Write-Output ("ipc_probe exit=" + $p2.ExitCode + " dur=" + $sw2.ElapsedMilliseconds + "ms")
    Write-Output ($p2.StandardOutput.ReadToEnd().Substring(0, 200))
} else {
    try { $p2.Kill() } catch {}
    Write-Output ("ipc_probe TIMED OUT at " + $sw2.ElapsedMilliseconds + "ms - listener ALSO blocked")
}
# Now wait for doctor outcome
$fin = $p.WaitForExit(20000)
if ($fin) {
    Write-Output ("doctor eventually exit=" + $p.ExitCode)
    Write-Output ($p.StandardOutput.ReadToEnd().Substring(0, 300))
} else {
    try { $p.Kill() } catch {}
    Write-Output 'doctor still hung at +20s; killed'
}
