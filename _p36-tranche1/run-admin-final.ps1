$ErrorActionPreference = 'Continue'
# Admin probe FINAL: label passed correctly (run-probe2.ps1 passes -Label Admin but inner
# hardcodes; fixed here by inlining label override). RunLevel Highest.
$wrapper = 'C:\Users\Public\admin-probe-wrapper3.ps1'
@'
"wrapper3-start $(Get-Date -Format o)" | Out-File 'C:\Windows\Temp\p36-wrap3.txt' -Force
& C:\Users\Public\probe2-inner.ps1 Admin
Copy-Item "$env:TEMP\p36-probe2-StandardUser.txt" 'C:\Windows\Temp\p36-admin-final.txt' -Force -ErrorAction SilentlyContinue
"done exists=$(Test-Path 'C:\Windows\Temp\p36-admin-final.txt')" | Out-File 'C:\Windows\Temp\p36-wrap3.txt' -Append -Force
'@ | Out-File $wrapper -Encoding ascii

$taskName = 'P36T1P2Admin9'
$evi = 'C:\AetherCore-P36\evidence'
$rng = [Security.Cryptography.RandomNumberGenerator]::Create()
$bytes = New-Object byte[] 32
$rng.GetBytes($bytes)
$pw = [Convert]::ToBase64String($bytes)
net user P36Admin $pw | Out-Null
Unregister-ScheduledTask -TaskName $taskName -Confirm:$false -ErrorAction SilentlyContinue
$act = New-ScheduledTaskAction -Execute 'powershell.exe' `
    -Argument '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File C:\Users\Public\admin-probe-wrapper3.ps1'
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Minutes 3)
Register-ScheduledTask -TaskName $taskName -Action $act -User 'P36Admin' -Password $pw -RunLevel Highest -Settings $settings | Out-Null
Start-ScheduledTask -TaskName $taskName
$deadline = (Get-Date).AddSeconds(150)
do {
    Start-Sleep -Seconds 3
    $state = (Get-ScheduledTask -TaskName $taskName).State
} while ($state -eq 'Running' -and (Get-Date) -lt $deadline)
$info = Get-ScheduledTaskInfo -TaskName $taskName
Write-Output "TASK_STATE=$state LASTRESULT=$($info.LastTaskResult)"
Unregister-ScheduledTask -TaskName $taskName -Confirm:$false
if (Test-Path 'C:\Windows\Temp\p36-admin-final.txt') {
    Copy-Item 'C:\Windows\Temp\p36-admin-final.txt' (Join-Path $evi 'tranche1-userprobe2-Admin.txt') -Force
    Write-Output '=== ADMIN PROBE RESULT ==='
    Get-Content (Join-Path $evi 'tranche1-userprobe2-Admin.txt')
} else {
    Write-Output 'NO FINAL OUTPUT'
    if (Test-Path 'C:\Windows\Temp\p36-wrap3.txt') { Get-Content 'C:\Windows\Temp\p36-wrap3.txt' }
}
