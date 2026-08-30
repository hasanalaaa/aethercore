$ErrorActionPreference = 'Continue'
$evi = 'C:\AetherCore-P36\evidence'
Write-Output '=== rerun Admin probe with output to shared temp (findable) ==='
# copy inner probe output to C:\Windows\Temp via a wrapper: use cmd /c with redirect
$taskName = 'P36T1P2Admin2'
$rng = [Security.Cryptography.RandomNumberGenerator]::Create()
$bytes = New-Object byte[] 32
$rng.GetBytes($bytes)
$pw = [Convert]::ToBase64String($bytes)
net user P36Admin $pw | Out-Null
Unregister-ScheduledTask -TaskName $taskName -Confirm:$false -ErrorAction SilentlyContinue
$act = New-ScheduledTaskAction -Execute 'powershell.exe' `
    -Argument '-NoProfile -ExecutionPolicy Bypass -Command "& C:\Users\Public\probe2-inner.ps1 Admin; Copy-Item $env:TEMP\p36-probe2-Admin.txt C:\Windows\Temp\ -Force -ErrorAction SilentlyContinue"'
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Minutes 3)
Register-ScheduledTask -TaskName $taskName -Action $act -User 'P36Admin' -Password $pw -RunLevel Limited -Settings $settings | Out-Null
Start-ScheduledTask -TaskName $taskName
$deadline = (Get-Date).AddSeconds(120)
do {
    Start-Sleep -Seconds 3
    $state = (Get-ScheduledTask -TaskName $taskName).State
} while ($state -eq 'Running' -and (Get-Date) -lt $deadline)
$info = Get-ScheduledTaskInfo -TaskName $taskName
Write-Output "TASK_STATE=$state LASTRESULT=$($info.LastTaskResult)"
Unregister-ScheduledTask -TaskName $taskName -Confirm:$false
$candidates = @(
    'C:\Windows\Temp\p36-probe2-Admin.txt',
    'C:\Users\P36Admin\AppData\Local\Temp\p36-probe2-Admin.txt'
)
$src = $null
foreach ($c in $candidates) { if (Test-Path $c) { $src = $c; break } }
if (-not $src) { Write-Output 'NO_OUTPUT_AGAIN'; exit 1 }
Copy-Item $src (Join-Path $evi 'tranche1-userprobe2-Admin.txt') -Force
Get-Content (Join-Path $evi 'tranche1-userprobe2-Admin.txt')
