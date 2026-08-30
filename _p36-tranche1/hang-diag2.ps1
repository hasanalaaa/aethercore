$ErrorActionPreference = 'Continue'
Write-Output '=== did the driver/inner powershells finish after kill? ==='
Get-Process -Name powershell -ErrorAction SilentlyContinue | ForEach-Object { Write-Output ("pid=" + $_.Id + " start=" + $_.StartTime) }
Write-Output '=== is P36StandardUser probe marked complete now? ==='
$raw = Get-Content 'C:\AetherCore-P36\evidence\tranche1-userprobe-StandardUser.txt' -Raw
if ($raw -match 'PROBE END') { Write-Output 'COMPLETE' } else { Write-Output 'INCOMPLETE - driver stuck too' }
