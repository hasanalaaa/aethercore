$ErrorActionPreference = 'Continue'
Write-Output '=== search everywhere for the Admin probe2 output ==='
Get-ChildItem 'C:\Windows\Temp' -Filter 'p36*' -ErrorAction SilentlyContinue | ForEach-Object { Write-Output $_.FullName }
Get-ChildItem 'C:\Users\P36Admin' -Recurse -Filter 'p36*' -ErrorAction SilentlyContinue | ForEach-Object { Write-Output $_.FullName }
Write-Output '=== P36Admin profile exists? ==='
Write-Output ("profile dir: " + (Test-Path 'C:\Users\P36Admin'))
Get-ChildItem 'C:\Users\P36Admin\AppData\Local\Temp' -ErrorAction SilentlyContinue | Select-Object -First 5 | ForEach-Object { Write-Output $_.Name }
Write-Output '=== check the evidence copy task actually ran as admin (create marker file) ==='
$taskName = 'P36T1P2Admin4'
$rng = [Security.Cryptography.RandomNumberGenerator]::Create()
$bytes = New-Object byte[] 32
$rng.GetBytes($bytes)
$pw = [Convert]::ToBase64String($bytes)
net user P36Admin $pw | Out-Null
Unregister-ScheduledTask -TaskName $taskName -Confirm:$false -ErrorAction SilentlyContinue
$act = New-ScheduledTaskAction -Execute 'cmd.exe' -Argument '/c whoami > C:\Windows\Temp\p36-admin-whoami.txt 2>&1'
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Minutes 2)
Register-ScheduledTask -TaskName $taskName -Action $act -User 'P36Admin' -Password $pw -RunLevel Highest -Settings $settings | Out-Null
Start-ScheduledTask -TaskName $taskName
Start-Sleep 12
Unregister-ScheduledTask -TaskName $taskName -Confirm:$false
if (Test-Path 'C:\Windows\Temp\p36-admin-whoami.txt') { Get-Content 'C:\Windows\Temp\p36-admin-whoami.txt' } else { Write-Output 'MARKER NOT CREATED' }
