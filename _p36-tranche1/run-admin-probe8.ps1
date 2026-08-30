$ErrorActionPreference = 'Continue'
Write-Output '=== check wrapper actually ran: leave breadcrumbs ==='
$wrapper = 'C:\Users\Public\admin-probe-wrapper2.ps1'
@'
"wrapper-start $(Get-Date -Format o)" | Out-File 'C:\Windows\Temp\p36-wrap-breadcrumb.txt' -Force
try {
    & C:\Users\Public\probe2-inner.ps1 Admin
    "inner-ok" | Out-File 'C:\Windows\Temp\p36-wrap-breadcrumb.txt' -Append -Force
} catch {
    "inner-err: $_" | Out-File 'C:\Windows\Temp\p36-wrap-breadcrumb.txt' -Append -Force
}
"temp-is: $env:TEMP" | Out-File 'C:\Windows\Temp\p36-wrap-breadcrumb.txt' -Append -Force
Copy-Item "$env:TEMP\p36-probe2-Admin.txt" 'C:\Windows\Temp\p36-probe2-Admin.txt' -Force -ErrorAction SilentlyContinue
"copy-done exists=$(Test-Path 'C:\Windows\Temp\p36-probe2-Admin.txt')" | Out-File 'C:\Windows\Temp\p36-wrap-breadcrumb.txt' -Append -Force
'@ | Out-File $wrapper -Encoding ascii

$taskName = 'P36T1P2Admin8'
$evi = 'C:\AetherCore-P36\evidence'
$rng = [Security.Cryptography.RandomNumberGenerator]::Create()
$bytes = New-Object byte[] 32
$rng.GetBytes($bytes)
$pw = [Convert]::ToBase64String($bytes)
net user P36Admin $pw | Out-Null
Unregister-ScheduledTask -TaskName $taskName -Confirm:$false -ErrorAction SilentlyContinue
$act = New-ScheduledTaskAction -Execute 'powershell.exe' `
    -Argument '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File C:\Users\Public\admin-probe-wrapper2.ps1'
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
Write-Output '=== breadcrumbs ==='
if (Test-Path 'C:\Windows\Temp\p36-wrap-breadcrumb.txt') { Get-Content 'C:\Windows\Temp\p36-wrap-breadcrumb.txt' } else { Write-Output 'NO BREADCRUMB - wrapper never ran' }
if (Test-Path 'C:\Windows\Temp\p36-probe2-Admin.txt') {
    Copy-Item 'C:\Windows\Temp\p36-probe2-Admin.txt' (Join-Path $evi 'tranche1-userprobe2-Admin.txt') -Force
    Write-Output '=== probe output ==='
    Get-Content (Join-Path $evi 'tranche1-userprobe2-Admin.txt')
}
