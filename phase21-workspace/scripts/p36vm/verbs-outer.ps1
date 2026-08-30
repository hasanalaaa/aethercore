# Runs as SYSTEM. Exercises the four verbs under BOTH actual-token contexts by
# the proven one-shot Scheduled Task method: a runtime-random password that is
# never printed or persisted, RunLevel Limited for P36StandardUser and Highest
# for P36Admin. Copies both transcripts to the Mac share.
# Usage: verbs-outer.ps1 <label>
param([string]$Label = 'unlabeled')
$ErrorActionPreference = 'Stop'
$OutRoot = '\\Mac\Home\Documents\p36-stage\out'
$inner   = 'C:\AetherCore-P36\tools\verbs-inner.ps1'
$dir     = 'C:\Users\Public\p36'
New-Item -ItemType Directory -Force $dir | Out-Null

function New-RandomPassword {
    $rng = [Security.Cryptography.RandomNumberGenerator]::Create()
    $b = New-Object byte[] 32; $rng.GetBytes($b); [Convert]::ToBase64String($b)
}

function Invoke-AsUser([string]$User, [string]$Level, [string]$Ctx) {
    $pw = New-RandomPassword                       # never printed, never persisted
    & net user $User $pw | Out-Null
    if ($LASTEXITCODE -ne 0) { throw ("password reset failed for $User, exit $LASTEXITCODE") }
    $name = 'P36VerbRun_' + $Ctx
    Unregister-ScheduledTask -TaskName $name -Confirm:$false -EA SilentlyContinue
    # P36_CTX is passed through the command line because a one-shot task does
    # not inherit the caller's environment.
    $arg = '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -Command "$env:P36_CTX=''' + $Ctx + '''; & ''' + $inner + '''"'
    $act = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument $arg
    # Without these two switches a task registered this way sits permanently Queued.
    $set = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Minutes 5)
    Register-ScheduledTask -TaskName $name -Action $act -User $User -Password $pw -RunLevel $Level -Settings $set | Out-Null
    Start-ScheduledTask -TaskName $name
    $deadline = (Get-Date).AddSeconds(150)
    do { Start-Sleep -Seconds 2; $state = (Get-ScheduledTask -TaskName $name).State }
    while ($state -eq 'Running' -and (Get-Date) -lt $deadline)
    $info = Get-ScheduledTaskInfo -TaskName $name
    Write-Output ("$Ctx TASK_STATE=$state LAST_RESULT=" + $info.LastTaskResult)
    Unregister-ScheduledTask -TaskName $name -Confirm:$false
    $src = Join-Path $dir ("verbs-" + $Ctx + ".txt")
    if (Test-Path $src) {
        Copy-Item $src (Join-Path $OutRoot ("verbs-$Label-$Ctx.txt")) -Force
        Write-Output ("COPIED verbs-$Label-$Ctx.txt")
        Select-String -Path $src -Pattern '^(--- VERB|RESULT=|EXIT_CODE=)' | ForEach-Object { $_.Line }
    } else { Write-Output ("$Ctx NO_OUTPUT") }
}

Invoke-AsUser 'P36StandardUser' 'Limited' 'STD'
Invoke-AsUser 'P36Admin'        'Highest' 'ADMIN'
