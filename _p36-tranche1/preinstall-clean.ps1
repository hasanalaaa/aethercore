$ErrorActionPreference = 'Continue'
Write-Output '=== residual dir contents (service deleted, product entry absent; dir from failed sim) ==='
Get-ChildItem 'C:\Program Files\AetherCore' -Force -ErrorAction SilentlyContinue | ForEach-Object { Write-Output $_.Name }
Remove-Item 'C:\Program Files\AetherCore' -Recurse -Force -ErrorAction SilentlyContinue
Write-Output ("removed; now exists: " + (Test-Path 'C:\Program Files\AetherCore'))
Write-Output '=== ProgramData state: reset to pre-install (keep machine-mutation.lock) ==='
Get-ChildItem 'C:\ProgramData\AetherCore' -Recurse -Force -ErrorAction SilentlyContinue | ForEach-Object { Write-Output $_.FullName }
Remove-Item 'C:\ProgramData\AetherCore\logs' -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item 'C:\ProgramData\AetherCore\state\aethercore.db' -Force -ErrorAction SilentlyContinue
Get-ChildItem 'C:\ProgramData\AetherCore' -Recurse -Force -ErrorAction SilentlyContinue | ForEach-Object { Write-Output $_.FullName }
Write-Output 'PREINSTALL-CLEAN'
