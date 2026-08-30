# P36 probe2 driver (SYSTEM): runs the fast pipe/write probe as the target user
param([Parameter(Mandatory=$true)][string]$Label)
$ErrorActionPreference = 'Continue'
$taskName = "P36T1P2" + $Label
$inner = 'C:\Users\Public\probe2-inner.ps1'
$evi = 'C:\AetherCore-P36\evidence'
$rng = [Security.Cryptography.RandomNumberGenerator]::Create()
$bytes = New-Object byte[] 32
$rng.GetBytes($bytes)
$pw = [Convert]::ToBase64String($bytes)
net user "P36$Label" $pw | Out-Null
Unregister-ScheduledTask -TaskName $taskName -Confirm:$false -ErrorAction SilentlyContinue
$act = New-ScheduledTaskAction -Execute 'powershell.exe' `
    -Argument ('-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "' + $inner + '" ' + $Label)
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Minutes 3)
Register-ScheduledTask -TaskName $taskName -Action $act -User "P36$Label" -Password $pw -RunLevel Limited -Settings $settings | Out-Null
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
    "C:\Users\P36$Label\AppData\Local\Temp\p36-probe2-$Label.txt",
    "C:\Windows\Temp\p36-probe2-$Label.txt"
)
$src = $null
foreach ($c in $candidates) { if (Test-Path $c) { $src = $c; break } }
if (-not $src) { Write-Output "NO_OUTPUT"; exit 1 }
Copy-Item $src (Join-Path $evi "tranche1-userprobe2-$Label.txt") -Force
Get-Content (Join-Path $evi "tranche1-userprobe2-$Label.txt")
