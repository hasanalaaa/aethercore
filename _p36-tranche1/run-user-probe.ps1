# P36 Tranche 1 - probe driver (SYSTEM): proven Scheduled-Task method (Codex-established)
param([Parameter(Mandatory=$true)][string]$Label)
$ErrorActionPreference = 'Continue'
$taskName = "P36T1Probe" + $Label
$inner = 'C:\Users\Public\user-probe-inner.ps1'
$evi = 'C:\AetherCore-P36\evidence'

# 1. fresh random password (never printed)
$rng = [Security.Cryptography.RandomNumberGenerator]::Create()
$bytes = New-Object byte[] 32
$rng.GetBytes($bytes)
$pw = [Convert]::ToBase64String($bytes)
net user "P36$Label" $pw | Out-Null
if ($LASTEXITCODE -ne 0) { Write-Output "PASSWORD_RESET_FAILED exit=$LASTEXITCODE"; exit 1 }
Write-Output "PASSWORD_RESET=OK (value not shown)"

# 2. one-shot task under the real account, RunLevel Limited
Unregister-ScheduledTask -TaskName $taskName -Confirm:$false -ErrorAction SilentlyContinue
$act = New-ScheduledTaskAction -Execute 'powershell.exe' `
    -Argument ('-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "' + $inner + '" ' + $Label)
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Minutes 6)
Register-ScheduledTask -TaskName $taskName -Action $act -User "P36$Label" -Password $pw `
    -RunLevel Limited -Settings $settings | Out-Null
Write-Output "TASK_REGISTERED=$taskName RunLevel=Limited"

# 3. run + wait (bounded)
Start-ScheduledTask -TaskName $taskName
$deadline = (Get-Date).AddSeconds(240)
do {
    Start-Sleep -Seconds 3
    $state = (Get-ScheduledTask -TaskName $taskName).State
} while ($state -eq 'Running' -and (Get-Date) -lt $deadline)
$info = Get-ScheduledTaskInfo -TaskName $taskName
Write-Output "TASK_STATE=$state"
Write-Output "TASK_LASTRESULT=$($info.LastTaskResult)"
Unregister-ScheduledTask -TaskName $taskName -Confirm:$false

# 4. collect output from user temp (or Windows temp) -> evidence
$candidates = @(
    "C:\Users\P36$Label\AppData\Local\Temp\p36-probe-$Label.txt",
    "C:\Windows\Temp\p36-probe-$Label.txt"
)
$src = $null
foreach ($c in $candidates) { if (Test-Path $c) { $src = $c; break } }
if (-not $src) { Write-Output "NO_PROBE_OUTPUT_FOUND (candidates checked: $($candidates -join ', '))"; exit 1 }
Copy-Item $src (Join-Path $evi "tranche1-userprobe-$Label.txt") -Force
Write-Output "EVIDENCE_WRITTEN=$evi\tranche1-userprobe-$Label.txt (source: $src)"
Get-Content (Join-Path $evi "tranche1-userprobe-$Label.txt")
