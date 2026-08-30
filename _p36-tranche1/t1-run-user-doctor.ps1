# P36 T1 - TASK 1 driver (SYSTEM): run doctor probes as P36Admin and P36StandardUser
# via the established one-shot Scheduled Task method; collect to evidence.
param([Parameter(Mandatory=$true)][string]$Label)
$ErrorActionPreference = 'Continue'
$taskName = "P36T1Doctor" + $Label
$inner = 'C:\Users\Public\t1-user-inner.ps1'
$evi = 'C:\AetherCore-P36\evidence'
$runLevel = if ($Label -eq 'Admin') { 'Highest' } else { 'Limited' }

$rng = [Security.Cryptography.RandomNumberGenerator]::Create()
$bytes = New-Object byte[] 32
$rng.GetBytes($bytes)
$pw = [Convert]::ToBase64String($bytes)
net user "P36$Label" $pw | Out-Null
Unregister-ScheduledTask -TaskName $taskName -Confirm:$false -ErrorAction SilentlyContinue
$act = New-ScheduledTaskAction -Execute 'powershell.exe' `
    -Argument ('-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File C:\Users\Public\t1-user-inner.ps1 ' + $Label)
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Minutes 4)
Register-ScheduledTask -TaskName $taskName -Action $act -User "P36$Label" -Password $pw -RunLevel $runLevel -Settings $settings | Out-Null
Write-Output "TASK_REGISTERED=$taskName RunLevel=$runLevel"

Start-ScheduledTask -TaskName $taskName
$deadline = (Get-Date).AddSeconds(150)
do {
    Start-Sleep -Seconds 3
    $state = (Get-ScheduledTask -TaskName $taskName).State
} while ($state -eq 'Running' -and (Get-Date) -lt $deadline)
$info = Get-ScheduledTaskInfo -TaskName $taskName
Write-Output "TASK_STATE=$state LASTRESULT=$($info.LastTaskResult)"
Unregister-ScheduledTask -TaskName $taskName -Confirm:$false

$src = "C:\Users\P36$Label\AppData\Local\Temp\t1-doctor-$Label.txt"
if (-not (Test-Path $src)) { $src = "C:\Windows\Temp\t1-doctor-$Label.txt" }
if (-not (Test-Path $src)) { Write-Output 'NO_OUTPUT'; exit 1 }
Copy-Item $src (Join-Path $evi "t1-doctor-$Label.txt") -Force
Get-Content (Join-Path $evi "t1-doctor-$Label.txt")
