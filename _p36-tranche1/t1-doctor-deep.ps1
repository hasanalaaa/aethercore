# P36 T1 Task1 - SYSTEM doctor diagnostic deep-run (30s+ capture) + concurrent request timing
# Establish whether the 10s deadline rejection is the diagnostic scan exceeding it, and
# whether the underlying provider work continues server-side after the client leaves.
$ErrorActionPreference = 'Continue'
$cli = 'C:\Program Files\AetherCore\aetherctl.exe'
$log = 'C:\ProgramData\AetherCore\logs\service.jsonl'
$sizeBefore = (Get-Item $log).Length
Write-Output ("log bytes before: " + $sizeBefore)

# Run doctor and wait up to 35s (longer than the 10s client deadline) to observe server behavior
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $cli
$psi.Arguments = '--output json --timeout-ms 30000 doctor'
$psi.UseShellExecute = $false
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$p = [System.Diagnostics.Process]::Start($psi)
$fin = $p.WaitForExit(35000)
$sw.Stop()
if ($fin) {
    $out = $p.StandardOutput.ReadToEnd()
    $err = $p.StandardError.ReadToEnd()
    Write-Output ("doctor --timeout-ms 30000: exit=" + $p.ExitCode + " dur=" + $sw.ElapsedMilliseconds + "ms")
    Write-Output ("stdout(first 400): " + $out.Substring(0, [Math]::Min(400, $out.Length)))
    if ($err) { Write-Output ("stderr(first 200): " + $err.Substring(0, [Math]::Min(200, $err.Length))) }
} else {
    try { $p.Kill(); $p.WaitForExit(5000) | Out-Null } catch {}
    Write-Output ("doctor TIMED OUT at 35s (client deadline 30s + margin) - killed")
}
Start-Sleep 3
Write-Output ("log bytes after: " + (Get-Item $log).Length)
Write-Output '=== new log entries ==='
$raw = Get-Content $log -Raw
$tail = $raw.Substring([Math]::Min($sizeBefore, $raw.Length))
$tail -split "`r?`n" | Where-Object { $_ } | Select-Object -Last 12 | ForEach-Object { Write-Output $_ }
