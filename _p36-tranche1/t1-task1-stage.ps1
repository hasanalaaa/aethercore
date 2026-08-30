# P36 Tranche 1 Closure - TASK 1: bounded aetherctl doctor reproduction matrix
# Per-context: SYSTEM (reference), P36Admin, P36StandardUser
# Every invocation runs under a hard 45s watchdog (no indefinite hangs).
$ErrorActionPreference = 'Continue'
$evid = 'C:\AetherCore-P36\evidence'
$cli  = 'C:\Program Files\AetherCore\aetherctl.exe'
$svcLog = 'C:\ProgramData\AetherCore\logs\service.jsonl'
$inc = 'C:\AetherCore-P36\incoming'

# 0. stage the CLI next to the service (product layout; update desktop launcher parity)
Copy-Item 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery\target\release\aetherctl.exe' "$cli" -Force -ErrorAction SilentlyContinue
if (-not (Test-Path $cli)) { $cli = 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery\target\release\aetherctl.exe' }
Write-Output "cli=$cli"

function Log-Mark {
    $mark = "T1MARK-" + [Guid]::NewGuid().ToString('N').Substring(0,8)
    return $mark
}
function Invoke-Bounded([string]$label, [string]$exe, [string]$args, [string]$logFile) {
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $exe
    $psi.Arguments = $args
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
        [ordered]@{
            label = $label; exit_code = $p.ExitCode; timed_out = $false
            duration_ms = $sw.ElapsedMilliseconds
            stdout = $out; stderr = $err
        } | ConvertTo-Json -Depth 3 | Set-Content $logFile
        Write-Output ("[$label] exit=" + $p.ExitCode + " dur=" + $sw.ElapsedMilliseconds + "ms")
        if ($out) { Write-Output ("  stdout: " + ($out.Substring(0, [Math]::Min(400, $out.Length)) -replace "`r`n", ' | ')) }
        if ($err) { Write-Output ("  stderr: " + ($err.Substring(0, [Math]::Min(400, $err.Length)) -replace "`r`n", ' | ')) }
    } else {
        $sw.Stop()
        try { $p.Kill(); $p.WaitForExit(5000) | Out-Null } catch {}
        [ordered]@{
            label = $label; exit_code = $null; timed_out = $true
            duration_ms = $sw.ElapsedMilliseconds
            stdout = ''; stderr = 'KILLED: exceeded 45s watchdog'
        } | ConvertTo-Json -Depth 3 | Set-Content $logFile
        Write-Output ("[$label] TIMED OUT after " + $sw.ElapsedMilliseconds + "ms - KILLED")
    }
}

Write-Output '=== service state before probes ==='
& sc.exe query AetherCoreMaintenance 2>&1 | Select-String 'STATE' | ForEach-Object { Write-Output $_.Line.Trim() }

Write-Output '=== T1.1 SYSTEM reference (diagnostic only) ==='
Invoke-Bounded 'SYSTEM-doctor' $cli 'doctor' "$evid\t1-doctor-system.json"

Write-Output '=== T1.2 P36Admin ==='
Write-Output '(admin run handled by scheduled task driver - see next step)'

Write-Output '=== T1.3 P36StandardUser ==='
Write-Output '(standard user run handled by scheduled task driver - see next step)'
Write-Output 'MATRIX-STAGED'
