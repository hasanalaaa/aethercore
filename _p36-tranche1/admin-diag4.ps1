$ErrorActionPreference = 'Continue'
# Diagnose why inline -Command task produced nothing: try the simplest possible -Command task
$taskName = 'P36T1P2Admin6'
$rng = [Security.Cryptography.RandomNumberGenerator]::Create()
$bytes = New-Object byte[] 32
$rng.GetBytes($bytes)
$pw = [Convert]::ToBase64String($bytes)
net user P36Admin $pw | Out-Null
Unregister-ScheduledTask -TaskName $taskName -Confirm:$false -ErrorAction SilentlyContinue
$act = New-ScheduledTaskAction -Execute 'powershell.exe' `
    -Argument '-NoProfile -Command "Get-Date | Out-File C:\Windows\Temp\p36-admin-cmdtest.txt -Force"'
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Minutes 2)
Register-ScheduledTask -TaskName $taskName -Action $act -User 'P36Admin' -Password $pw -RunLevel Highest -Settings $settings | Out-Null
Start-ScheduledTask -TaskName $taskName
Start-Sleep 15
$info = Get-ScheduledTaskInfo -TaskName $taskName
Write-Output "LASTRESULT=$($info.LastTaskResult)"
Unregister-ScheduledTask -TaskName $taskName -Confirm:$false
if (Test-Path 'C:\Windows\Temp\p36-admin-cmdtest.txt') { Write-Output 'CMD-TEST-OK'; Get-Content 'C:\Windows\Temp\p36-admin-cmdtest.txt' } else { Write-Output 'CMD-TEST-FAILED' }
