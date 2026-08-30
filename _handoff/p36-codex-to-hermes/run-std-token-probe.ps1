# P36 StandardUser token probe — outer driver (runs as SYSTEM via prlctl exec)
# Establishes the proven one-shot Scheduled Task method: reset password (random,
# runtime-only, NEVER printed/persisted), register one-shot task with Password
# logon type + RunLevel Limited under the ACTUAL P36StandardUser, run, collect.
$ErrorActionPreference = 'Stop'
$evi = 'C:\AetherCore-P36\evidence'
$inner = Join-Path $evi 'std-token-probe-inner.ps1'

# 1. Reset P36StandardUser password to a fresh random value (never printed)
$rng = [Security.Cryptography.RandomNumberGenerator]::Create()
$bytes = New-Object byte[] 32
$rng.GetBytes($bytes)
$pw = [Convert]::ToBase64String($bytes)
net user P36StandardUser $pw | Out-Null
if ($LASTEXITCODE -ne 0) { throw ('password reset failed, exit ' + $LASTEXITCODE) }
Write-Output 'PASSWORD_RESET=OK (value not shown)'

# 2. Register one-shot task under P36StandardUser, RunLevel Limited
$name = 'P36StdTokenProbe'
Unregister-ScheduledTask -TaskName $name -Confirm:$false -ErrorAction SilentlyContinue
$act = New-ScheduledTaskAction -Execute 'powershell.exe' `
    -Argument ('-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "' + $inner + '"')
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Minutes 5)
Register-ScheduledTask -TaskName $name -Action $act -User 'P36StandardUser' -Password $pw `
    -RunLevel Limited -Settings $settings | Out-Null
Write-Output 'TASK_REGISTERED=P36StdTokenProbe RunLevel=Limited'

# 3. Run and wait (bounded 90 s)
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

# 4. Locate output (user TEMP may be profile temp or Windows temp) and copy to evidence
$candidates = @(
    'C:\Users\P36StandardUser\AppData\Local\Temp\p36std-token.txt',
    'C:\Windows\Temp\p36std-token.txt'
)
$src = $null
foreach ($c in $candidates) { if (Test-Path $c) { $src = $c; break } }
if (-not $src) { throw 'probe output not found in either temp location' }
Copy-Item $src (Join-Path $evi 'P36StandardUser-token-groups.txt') -Force
Write-Output ('EVIDENCE_WRITTEN=C:\AetherCore-P36\evidence\P36StandardUser-token-groups.txt (source: ' + $src + ')')
