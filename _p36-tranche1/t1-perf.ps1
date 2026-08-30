$ErrorActionPreference = 'Continue'
Write-Output '=== C: simpler service request (perf snapshot) through the same client stack ==='
# This isolates: is it doctor-specific or request-path-wide?
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = 'C:\Program Files\AetherCore\aetherctl.exe'
$psi.Arguments = '--output json perf snapshot'
$psi.UseShellExecute = $false
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$p = [System.Diagnostics.Process]::Start($psi)
$fin = $p.WaitForExit(25000)
if ($fin) {
    $sw.Stop()
    $out = $p.StandardOutput.ReadToEnd()
    Write-Output ("perf snapshot exit=" + $p.ExitCode + " dur=" + $sw.ElapsedMilliseconds + "ms")
    Write-Output ("stdout(first 250): " + $out.Substring(0, [Math]::Min(250, $out.Length)))
} else {
    try { $p.Kill(); $p.WaitForExit(5000) | Out-Null } catch {}
    Write-Output ("perf snapshot HUNG at " + $sw.ElapsedMilliseconds + "ms")
}
Write-Output '=== D: care status (different request type) ==='
$psi2 = New-Object System.Diagnostics.ProcessStartInfo
$psi2.FileName = 'C:\Program Files\AetherCore\aetherctl.exe'
$psi2.Arguments = '--output json care status'
$psi2.UseShellExecute = $false
$psi2.RedirectStandardOutput = $true
$psi2.RedirectStandardError = $true
$sw2 = [System.Diagnostics.Stopwatch]::StartNew()
$p2 = [System.Diagnostics.Process]::Start($psi2)
$fin2 = $p2.WaitForExit(25000)
if ($fin2) {
    $sw2.Stop()
    $out2 = $p2.StandardOutput.ReadToEnd()
    Write-Output ("care status exit=" + $p2.ExitCode + " dur=" + $sw2.ElapsedMilliseconds + "ms")
    Write-Output ("stdout(first 250): " + $out.Substring(0, [Math]::Min(250, $out.Length)))
} else {
    try { $p2.Kill(); $p2.WaitForExit(5000) | Out-Null } catch {}
    Write-Output ("care status HUNG at " + $sw2.ElapsedMilliseconds + "ms")
}
Write-Output '=== service log during window ==='
Get-Content 'C:\ProgramData\AetherCore\logs\service.jsonl' -Tail 6 | ForEach-Object { Write-Output $_ }
