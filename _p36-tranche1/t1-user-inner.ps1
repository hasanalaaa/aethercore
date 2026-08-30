# P36 T1 - user-context aetherctl probe (runs AS the target user via scheduled task)
# Bounded 45s watchdog inside the user session; captures exact stdout/stderr/exit.
param([Parameter(Mandatory=$true)][string]$Label)
$ErrorActionPreference = 'Continue'
$outFile = Join-Path $env:TEMP ("t1-doctor-" + $Label + ".txt")
$cli = 'C:\Program Files\AetherCore\aetherctl.exe'
"=== T1 DOCTOR PROBE: $Label ===" | Out-File $outFile -Encoding utf8
"Context: $(whoami)  Time: $(Get-Date -Format o)" | Out-File $outFile -Append -Encoding utf8
(& whoami.exe /groups 2>&1 | Out-String) -split "`r?`n" |
    Where-Object { $_ -match 'Mandatory Label|S-1-5-32-544' } |
    ForEach-Object { $_.Trim() } |
    Out-File $outFile -Append -Encoding utf8

"--- aetherctl doctor (45s watchdog) ---" | Out-File $outFile -Append -Encoding utf8
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $cli
$psi.Arguments = '--output json doctor'
$psi.UseShellExecute = $false
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$p = [System.Diagnostics.Process]::Start($psi)
$finished = $p.WaitForExit(45000)
if ($finished) {
    $sw.Stop()
    $out = $p.StandardOutput.ReadToEnd()
    $err = $p.StandardError.ReadToEnd()
    "exit=$($p.ExitCode) timed_out=false duration_ms=$($sw.ElapsedMilliseconds)" | Out-File $outFile -Append -Encoding utf8
    "stdout:" | Out-File $outFile -Append -Encoding utf8
    $out | Out-File $outFile -Append -Encoding utf8
    "stderr:" | Out-File $outFile -Append -Encoding utf8
    $err | Out-File $outFile -Append -Encoding utf8
} else {
    $sw.Stop()
    try { $p.Kill(); $p.WaitForExit(5000) | Out-Null } catch {}
    "exit=null timed_out=true duration_ms=$($sw.ElapsedMilliseconds)" | Out-File $outFile -Append -Encoding utf8
    "stderr: KILLED by 45s watchdog" | Out-File $outFile -Append -Encoding utf8
}

"--- aetherctl service detect (offline probe, 30s watchdog) ---" | Out-File $outFile -Append -Encoding utf8
$psi2 = New-Object System.Diagnostics.ProcessStartInfo
$psi2.FileName = $cli
$psi2.Arguments = '--output json service detect'
$psi2.UseShellExecute = $false
$psi2.RedirectStandardOutput = $true
$psi2.RedirectStandardError = $true
$sw2 = [System.Diagnostics.Stopwatch]::StartNew()
$p2 = [System.Diagnostics.Process]::Start($psi2)
$finished2 = $p2.WaitForExit(30000)
if ($finished2) {
    $sw2.Stop()
    "exit=$($p2.ExitCode) timed_out=false duration_ms=$($sw2.ElapsedMilliseconds)" | Out-File $outFile -Append -Encoding utf8
    "stdout:" | Out-File $outFile -Append -Encoding utf8
    $p2.StandardOutput.ReadToEnd() | Out-File $outFile -Append -Encoding utf8
    "stderr:" | Out-File $outFile -Append -Encoding utf8
    $p2.StandardError.ReadToEnd() | Out-File $outFile -Append -Encoding utf8
} else {
    $sw2.Stop()
    try { $p2.Kill(); $p2.WaitForExit(5000) | Out-Null } catch {}
    "exit=null timed_out=true duration_ms=$($sw2.ElapsedMilliseconds) (service detect KILLED)" | Out-File $outFile -Append -Encoding utf8
}
"=== T1 PROBE END ===" | Out-File $outFile -Append -Encoding utf8
