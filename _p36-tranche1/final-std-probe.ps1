# P36 Tranche 1 - final StandardUser probe against the FINAL MSI install (clean run)
$ErrorActionPreference = 'Continue'
$taskName = 'P36T1FinalStd'
$evi = 'C:\AetherCore-P36\evidence'
$inner = 'C:\Users\Public\probe2-inner.ps1'
# fix the hardcoded label first
$c = Get-Content $inner -Raw
$c = $c.Replace("\$Label = 'StandardUser'", "param([string]`$Label); if (-not `\$Label) { `\$Label = 'StandardUser' }")
# simpler: rewrite first lines deterministically
$lines = Get-Content $inner
$lines[2] = 'if (-not $Label) { $Label = $args[0] }'
$lines[3] = 'if (-not $Label) { $Label = ''StandardUser'' }'
$lines | Set-Content 'C:\Users\Public\probe2-inner2.ps1' -Encoding ascii

$rng = [Security.Cryptography.RandomNumberGenerator]::Create()
$bytes = New-Object byte[] 32
$rng.GetBytes($bytes)
$pw = [Convert]::ToBase64String($bytes)
net user P36StandardUser $pw | Out-Null
Unregister-ScheduledTask -TaskName $taskName -Confirm:$false -ErrorAction SilentlyContinue
$act = New-ScheduledTaskAction -Execute 'powershell.exe' `
    -Argument '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File C:\Users\Public\probe2-inner2.ps1 StandardUser'
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Minutes 3)
Register-ScheduledTask -TaskName $taskName -Action $act -User 'P36StandardUser' -Password $pw -RunLevel Limited -Settings $settings | Out-Null
Start-ScheduledTask -TaskName $taskName
$deadline = (Get-Date).AddSeconds(120)
do {
    Start-Sleep -Seconds 3
    $state = (Get-ScheduledTask -TaskName $taskName).State
} while ($state -eq 'Running' -and (Get-Date) -lt $deadline)
$info = Get-ScheduledTaskInfo -TaskName $taskName
Write-Output "TASK_STATE=$state LASTRESULT=$($info.LastTaskResult)"
Unregister-ScheduledTask -TaskName $taskName -Confirm:$false
$src = 'C:\Users\P36StandardUser\AppData\Local\Temp\p36-probe2-StandardUser.txt'
if (-not (Test-Path $src)) { Write-Output 'NO OUTPUT'; exit 1 }
Copy-Item $src (Join-Path $evi 'tranche1-final-std-probe.txt') -Force
Get-Content (Join-Path $evi 'tranche1-final-std-probe.txt')
