$ErrorActionPreference = 'Continue'
$pf = 'C:\Program Files\AetherCore'
Write-Output '=== forced cleanup: takeown + grant + delete ==='
& takeown.exe /F $pf /R /D Y 2>&1 | Select-Object -Last 3 | Out-String | Write-Output
& icacls.exe $pf /grant "*S-1-5-32-544:F" "*S-1-5-18:F" /T /C /Q 2>&1 | Select-Object -Last 3 | Out-String | Write-Output
Remove-Item $pf -Recurse -Force
Write-Output ("removed; now exists: " + (Test-Path $pf))
if (Test-Path $pf) { Get-ChildItem $pf -Recurse -Force | ForEach-Object { Write-Output $_.FullName } }
Write-Output 'FORCED-CLEANUP-DONE'
