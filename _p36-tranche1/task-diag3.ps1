$ErrorActionPreference = 'Continue'
Write-Output '=== old probe task definitions (previous session names + inner script paths) ==='
foreach ($t in @('P36AdminContextProbe','P36StdContextProbe','P36StdProbe2')) {
    Write-Output "--- $t ---"
    schtasks /Query /TN $t /XML 2>&1 | Select-String -Pattern 'Command|Arguments|UserId|RunLevel' | ForEach-Object { Write-Output $_.Line.Trim() }
}
Write-Output '=== old inner probe scripts on disk? ==='
Get-ChildItem 'C:\AetherCore-P36\evidence' -Filter '*probe*' | ForEach-Object { Write-Output $_.Name }
Get-ChildItem 'C:\Users\Public' -Filter '*inner*' | ForEach-Object { Write-Output $_.Name }
Write-Output '=== handoff dir probe scripts (Mac-shared) ==='
Get-ChildItem '\\Mac\Home\Documents\AetherCore 2\_handoff\p36-codex-to-hermes' -Filter '*probe*' -ErrorAction SilentlyContinue | ForEach-Object { Write-Output $_.Name }
