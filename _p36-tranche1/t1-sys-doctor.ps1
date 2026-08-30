$ErrorActionPreference = 'Continue'
Write-Output '=== T1.1 SYSTEM doctor (bounded, from staged evidence json) ==='
$e = Get-Content 'C:\AetherCore-P36\evidence\t1-doctor-system.json' -Raw | ConvertFrom-Json
Write-Output ("exit_code: " + $e.exit_code)
Write-Output ("timed_out: " + $e.timed_out)
Write-Output ("stderr: " + $e.stderr.Substring(0, [Math]::Min(200, $e.stderr.Length)))
Write-Output 'NOTE: the cli staged at Program Files is a COPY run; exit 2 = usage error because args were passed as one string to cmd wrongly. Re-run properly.'
# Proper run: quote-free single arg
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
if ($finished = $finished) {}
if ($finished) {
    $sw.Stop()
    Write-Output ("SYSTEM doctor exit=" + $p.ExitCode + " dur=" + $sw.ElapsedMilliseconds + "ms")
    Write-Output ("stdout: " + $p.StandardOutput.ReadToEnd())
    Write-Output ("stderr: " + $p.StandardError.ReadToEnd())
} else {
    try { $p.Kill() } catch {}
    Write-Output ("SYSTEM doctor TIMED OUT after " + $sw.ElapsedMilliseconds + "ms")
}
