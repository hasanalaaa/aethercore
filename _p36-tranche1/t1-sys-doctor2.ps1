$ErrorActionPreference = 'Continue'
Write-Output '=== SYSTEM doctor, clean bounded run ==='
$cli = 'C:\Program Files\AetherCore\aetherctl.exe'
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $cli
$psi.Arguments = '--output json doctor'
$psi.UseShellExecute = $false
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$p = [System.Diagnostics.Process]::Start($psi)
$fin = $p.WaitForExit(45000)
if ($fin) {
    $sw.Stop()
    $out = $p.StandardOutput.ReadToEnd()
    $err = $p.StandardError.ReadToEnd()
    Write-Output ("SYSTEM doctor exit=" + $p.ExitCode + " dur=" + $sw.ElapsedMilliseconds + "ms timed_out=false")
    Write-Output ("stdout: " + $out.Substring(0, [Math]::Min(600, $out.Length)))
    Write-Output ("stderr: " + $err.Substring(0, [Math]::Min(600, $err.Length)))
    [ordered]@{ label='SYSTEM-doctor'; exit_code=$p.ExitCode; timed_out=$false; duration_ms=$sw.ElapsedMilliseconds; stdout=$out; stderr=$err } |
        ConvertTo-Json -Depth 3 | Set-Content 'C:\AetherCore-P36\evidence\t1-doctor-system.json'
} else {
    try { $p.Kill(); $p.WaitForExit(5000) | Out-Null } catch {}
    Write-Output ("SYSTEM doctor TIMED_OUT after " + $sw.ElapsedMilliseconds + "ms - KILLED")
    "exit=null timed_out=true duration_ms=$($sw.ElapsedMilliseconds)" | Set-Content 'C:\AetherCore-P36\evidence\t1-doctor-system.json'
}
Write-Output '=== service log tail right after ==='
Get-Content 'C:\ProgramData\AetherCore\logs\service.jsonl' -Tail 4 | ForEach-Object { Write-Output $_ }
