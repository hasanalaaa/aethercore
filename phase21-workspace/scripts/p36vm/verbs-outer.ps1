# Runs as SYSTEM. Exercises the four verbs under BOTH actual-token contexts by
# the proven one-shot Scheduled Task method: a runtime-random password that is
# never printed or persisted, RunLevel Limited for P36StandardUser and Highest
# for P36Admin. Copies both transcripts to the Mac share.
# Usage: verbs-outer.ps1 <label>
param([string]$Label = 'unlabeled')
$ErrorActionPreference = 'Stop'
$OutRoot = '\\Mac\dev\p36-stage\out'
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
    $src = Join-Path $dir ("verbs-" + $Ctx + ".txt")
    # A transcript left by an EARLIER run is indistinguishable from this run's output
    # once it is copied to the Mac. Remove it before the task starts.
    Remove-Item $src -Force -ErrorAction SilentlyContinue
    Unregister-ScheduledTask -TaskName $name -Confirm:$false -EA SilentlyContinue
    # P36_CTX is passed through the command line because a one-shot task does
    # not inherit the caller's environment.
    $arg = '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -Command "$env:P36_CTX=''' + $Ctx + '''; & ''' + $inner + '''"'
    $act = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument $arg
    # Without these two switches a task registered this way sits permanently Queued.
    $set = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Minutes 5)
    Register-ScheduledTask -TaskName $name -Action $act -User $User -Password $pw -RunLevel $Level -Settings $set | Out-Null
    # -2s absorbs the second-granularity of LastRunTime; anything older than this is
    # necessarily a previous run.
    $t0 = (Get-Date).AddSeconds(-2)
    Start-ScheduledTask -TaskName $name
    # Wait through Queued as well as Running: a task that never leaves Queued (the
    # 2026-09-01 battery stall) must time out here, not fall straight through.
    $deadline = (Get-Date).AddSeconds(240)
    do { Start-Sleep -Seconds 3; $state = (Get-ScheduledTask -TaskName $name).State }
    while (($state -eq 'Running' -or $state -eq 'Queued') -and (Get-Date) -lt $deadline)
    $info = Get-ScheduledTaskInfo -TaskName $name
    $ran = ($info.LastRunTime -ne $null) -and ($info.LastRunTime -gt $t0)
    Write-Output ("$Ctx TASK_STATE=$state LAST_RESULT=" + $info.LastTaskResult +
                  " LAST_RUN=" + $info.LastRunTime + " RAN_THIS_INVOCATION=" + $ran)
    Unregister-ScheduledTask -TaskName $name -Confirm:$false
    # THE gate. Without it this harness copies whatever file happens to be on disk and
    # reports a PASS that belongs to an earlier run (Phase 39 18.5).
    if (-not $ran) {
        throw ("$Ctx STALE_RESULT: the task did not run this invocation " +
               "(state=$state LastRunTime=" + $info.LastRunTime + " started=$t0). " +
               "No transcript from this harness run exists; refusing to report one.")
    }
    if (Test-Path $src) {
        Copy-Item $src (Join-Path $OutRoot ("verbs-$Label-$Ctx.txt")) -Force
        Write-Output ("COPIED verbs-$Label-$Ctx.txt")
        Select-String -Path $src -Pattern '^(--- VERB|RESULT=|EXIT_CODE=)' | ForEach-Object { $_.Line }
    } else { Write-Output ("$Ctx NO_OUTPUT") }
}

Invoke-AsUser 'P36StandardUser' 'Limited' 'STD'
Invoke-AsUser 'P36Admin'        'Highest' 'ADMIN'
