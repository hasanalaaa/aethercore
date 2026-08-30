$ErrorActionPreference = 'Continue'
# -Command works for Admin; the earlier failure was quoting of nested quotes. Use -File with a
# wrapper script that writes to C:\Windows\Temp explicitly (Admin can write there).
$wrapper = 'C:\Users\Public\admin-probe-wrapper.ps1'
@'
& C:\Users\Public\probe2-inner.ps1 Admin
Copy-Item "$env:TEMP\p36-probe2-Admin.txt" 'C:\Windows\Temp\p36-probe2-Admin.txt' -Force -ErrorAction SilentlyContinue
'@ | Out-File $wrapper -Encoding utf8

$taskName = 'P36T1P2Admin7'
$evi = 'C:\AetherCore-P36\evidence'
$rng = [Security.Cryptography.RandomNumberGenerator]::Create()
$bytes = New-Object byte[] 32
$rng.GetBytes($bytes)
$pw = [Convert]::ToBase64String($bytes)
net user P36Admin $pw | Out-Null
Unregister-ScheduledTask -TaskName $taskName -Confirm:$false -ErrorAction SilentlyContinue
$act = New-ScheduledTaskAction -Execute 'powershell.exe' `
    -Argument '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File C:\Users\Public\admin-probe-wrapper.ps1'
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
if (Test-Path 'C:\Windows\Temp\p36-probe2-Admin.txt') {
    Copy-Item 'C:\Windows\Temp\p36-probe2-Admin.txt' (Join-Path $evi 'tranche1-userprobe2-Admin.txt') -Force
    Get-Content (Join-Path $evi 'tranche1-userprobe2-Admin.txt')
} else {
    Write-Output 'STILL NO OUTPUT'
}
