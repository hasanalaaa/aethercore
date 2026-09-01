# Proves the freshness predicate verbs-outer.ps1 now gates on, without touching the
# product: register a harmless task, evaluate the predicate BEFORE it has ever run
# (must be False — this is the case that used to sail through as a PASS), then run it
# and evaluate again (must be True). Run as SYSTEM/administrator on the qualification VM.
$ErrorActionPreference = 'Stop'
$name = 'P40StaleTest'
Unregister-ScheduledTask -TaskName $name -Confirm:$false -EA SilentlyContinue
$act = New-ScheduledTaskAction -Execute 'cmd.exe' -Argument '/c exit 0'
$set = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Minutes 5)
Register-ScheduledTask -TaskName $name -Action $act -User 'SYSTEM' -RunLevel Highest -Settings $set | Out-Null

$failures = 0
$t0 = (Get-Date).AddSeconds(-2)
$info = Get-ScheduledTaskInfo -TaskName $name
$ran = ($null -ne $info.LastRunTime) -and ($info.LastRunTime -gt $t0)
Write-Output ("NEVER_RAN LastRunTime=" + $info.LastRunTime + " RAN=$ran WANT=False")
if ($ran) { $failures++ }

$t0 = (Get-Date).AddSeconds(-2)
Start-ScheduledTask -TaskName $name
$deadline = (Get-Date).AddSeconds(120)
do { Start-Sleep -Seconds 3; $state = (Get-ScheduledTask -TaskName $name).State }
while (($state -eq 'Running' -or $state -eq 'Queued') -and (Get-Date) -lt $deadline)
$info = Get-ScheduledTaskInfo -TaskName $name
$ran = ($null -ne $info.LastRunTime) -and ($info.LastRunTime -gt $t0)
Write-Output ("AFTER_RUN  LastRunTime=" + $info.LastRunTime + " RAN=$ran WANT=True")
if (-not $ran) { $failures++ }

Unregister-ScheduledTask -TaskName $name -Confirm:$false
Write-Output "STALETEST_FAILURES=$failures"
exit $(if ($failures -eq 0) { 0 } else { 1 })
