$ErrorActionPreference = 'Continue'
# Admin probe attempt 3: RunLevel HIGHEST (elevated token as P36Admin is meant to be High)
$taskName = 'P36T1P2Admin3'
$evi = 'C:\AetherCore-P36\evidence'
$rng = [Security.Cryptography.RandomNumberGenerator]::Create()
$bytes = New-Object byte[] 32
$rng.GetBytes($bytes)
$pw = [Convert]::ToBase64String($bytes)
net user P36Admin $pw | Out-Null
Unregister-ScheduledTask -TaskName $taskName -Confirm:$false -ErrorAction SilentlyContinue
$act = New-ScheduledTaskAction -Execute 'powershell.exe' `
    -Argument '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File C:\Users\Public\probe2-inner.ps1 Admin'
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Minutes 3)
Register-ScheduledTask -TaskName $taskName -Action $act -User 'P36Admin' -Password $pw -RunLevel Highest -Settings $settings | Out-Null
Start-ScheduledTask -TaskName $taskName
$deadline = (Get-Date).AddSeconds(120)
do {
    Start-Sleep -Seconds 3
    $state = (Get-ScheduledTask -TaskName $taskName).State
} while ($state -eq 'Running' -and (Get-Date) -lt $deadline)
$info = Get-ScheduledTaskInfo -TaskName $taskName
Write-Output "TASK_STATE=$state LASTRESULT=$($info.LastTaskResult)"
Unregister-ScheduledTask -TaskName $taskName -Confirm:$false
$src = 'C:\Users\P36Admin\AppData\Local\Temp\p36-probe2-Admin.txt'
if (-not (Test-Path $src)) { $src = 'C:\Windows\Temp\p36-probe2-Admin.txt' }
if (-not (Test-Path $src)) { Write-Output 'NO_OUTPUT_3'; exit 1 }
Copy-Item $src (Join-Path $evi 'tranche1-userprobe2-Admin.txt') -Force
Get-Content (Join-Path $evi 'tranche1-userprobe2-Admin.txt')
