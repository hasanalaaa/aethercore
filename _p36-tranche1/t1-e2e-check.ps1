$ErrorActionPreference = 'Continue'
# Rerun the console service but as the REAL service session is stopped and pipe is free;
# error 0x8007051B says this SYSTEM console run could not set owner=NT SERVICE SID because
# the SCM-created service SID doesn't exist for a console process. This is exactly the
# "console mode can't own the pipe" dev-mode gap. Workaround for the diagnostic only:
# run the console service via a scheduled task so the service account + SID exist.
# SIMPLER: instead of console service, test the REAL installed service with an
# Admin-requested GetUpdateSnapshot (a different cheap request) to see if ALL requests stall.
Write-Output '=== restart real service ==='
& sc.exe start AetherCoreMaintenance 2>&1 | Out-Null
Start-Sleep 6
& sc.exe query AetherCoreMaintenance 2>&1 | Select-String 'STATE' | ForEach-Object { Write-Output $_.Line.Trim() }
Write-Output '=== F: GETUPDATE SNAPSHOT (light request, no providers) via aetherctl? no such CLI verb; use care status instead ==='
# G: check if the service responds to ANY request type - use the perf snapshot which is stateless
Write-Output '=== request round-trip probe with 3s timeout (distinguishes hang vs slow) ==='
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery\target\release\examples\ipc_request_probe.exe'
$psi.UseShellExecute = $false
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$p = [System.Diagnostics.Process]::Start($psi)
$fin = $p.WaitForExit(30000)
$sw.Stop()
if ($fin) {
    Write-Output ("probe exit=" + $p.ExitCode + " dur=" + $sw.ElapsedMilliseconds + "ms")
    $p.StandardOutput.ReadToEnd() -split "`r?`n" | ForEach-Object { Write-Output $_ }
} else {
    try { $p.Kill() } catch {}
    Write-Output ("probe TIMED OUT at " + $sw.ElapsedMilliseconds + "ms")
}
