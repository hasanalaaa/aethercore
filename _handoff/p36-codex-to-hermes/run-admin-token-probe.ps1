# P36 Admin token probe — outer driver (SYSTEM). One-shot task, RunLevel Highest, password never printed.
$ErrorActionPreference = 'Stop'
$evi = 'C:\AetherCore-P36\evidence'
$inner = Join-Path $evi 'admin-token-probe-inner.ps1'
$rng = [Security.Cryptography.RandomNumberGenerator]::Create()
$bytes = New-Object byte[] 32
$rng.GetBytes($bytes)
$pw = [Convert]::ToBase64String($bytes)
net user P36Admin $pw | Out-Null
if ($LASTEXITCODE -ne 0) { throw ('password reset failed, exit ' + $LASTEXITCODE) }
Write-Output 'PASSWORD_RESET=OK (value not shown)'
$name = 'P36AdminTokenProbe'
Unregister-ScheduledTask -TaskName $name -Confirm:$false -ErrorAction SilentlyContinue
$act = New-ScheduledTaskAction -Execute 'powershell.exe' `
    -Argument ('-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "' + $inner + '"')
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Minutes 5)
Register-ScheduledTask -TaskName $name -Action $act -User 'P36Admin' -Password $pw `
    -RunLevel Highest -Settings $settings | Out-Null
Write-Output 'TASK_REGISTERED=P36AdminTokenProbe RunLevel=Highest'
Start-ScheduledTask -TaskName $name
$deadline = (Get-Date).AddSeconds(90)
do {
    Start-Sleep -Seconds 2
    $state = (Get-ScheduledTask -TaskName $name).State
} while ($state -eq 'Running' -and (Get-Date) -lt $deadline)
$info = Get-ScheduledTaskInfo -TaskName $name
Write-Output ('TASK_STATE=' + $state)
Write-Output ('TASK_LASTRESULT=' + $info.LastTaskResult)
Unregister-ScheduledTask -TaskName $name -Confirm:$false
$src = $null
foreach ($c in @('C:\Users\P36Admin\AppData\Local\Temp\p36admin-token.txt','C:\Windows\Temp\p36admin-token.txt')) {
    if (Test-Path $c) { $src = $c; break }
}
if (-not $src) { throw 'probe output not found' }
Copy-Item $src (Join-Path $evi 'P36Admin-token-evidence.txt') -Force
Write-Output ('EVIDENCE_WRITTEN=C:\AetherCore-P36\evidence\P36Admin-token-evidence.txt (source: ' + $src + ')')
