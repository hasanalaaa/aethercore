# P36 - exact exit code of the service binary in console mode + loader diagnosis
$ErrorActionPreference = 'Continue'
$diag = 'C:\AetherCore-P36\incoming\diag'
Write-Output '=== cmd direct run, echo errorlevel ==='
& cmd.exe /d /s /c "`"$diag\aethercore-maintenance-service.exe`" --console & echo EXITCODE=%errorlevel%" 2>&1 | Out-String | Write-Output
Write-Output '=== run with no args (dispatcher path) ==='
& cmd.exe /d /s /c "`"$diag\aethercore-maintenance-service.exe`" & echo EXITCODE=%errorlevel%" 2>&1 | Out-String | Write-Output
Write-Output '=== Application event log crashes (last 15 min) ==='
Get-WinEvent -FilterHashtable @{ LogName='Application'; StartTime=(Get-Date).AddMinutes(-15) } -ErrorAction SilentlyContinue |
    Where-Object { $_.Message -match 'aethercore' } |
    Select-Object -First 5 | ForEach-Object { Write-Output ("[$($_.Id)][$($_.ProviderName)] " + ($_.Message -replace "`r`n", ' ')) }
Write-Output '=== loader snap: try loading exe via powershell Start-Process and read ExitCode properly ==='
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = "$diag\aethercore-maintenance-service.exe"
$psi.Arguments = '--console'
$psi.UseShellExecute = $false
$psi.RedirectStandardError = $true
$psi.RedirectStandardOutput = $true
$proc = [System.Diagnostics.Process]::Start($psi)
$proc.WaitForExit(15000) | Out-Null
Write-Output ("HasExited=" + $proc.HasExited + "  ExitCode=" + $proc.ExitCode)
Write-Output ("stdout: " + ($proc.StandardOutput.ReadToEnd()))
Write-Output ("stderr: " + ($proc.StandardError.ReadToEnd()))
