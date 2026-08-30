$ErrorActionPreference = 'Continue'
$cli = 'C:\Program Files\AetherCore\ipc_probe.exe'
Write-Output '=== FINAL post-install pipe verification (SYSTEM client, exact mask) ==='
& $cli 2>&1 | Select-Object -First 2 | Out-String | Write-Output
Write-Output '=== Service SID / SD final check ==='
& sc.exe showsid AetherCoreMaintenance 2>&1 | Select-String 'SERVICE SID|STATUS' | ForEach-Object { Write-Output $_.Line.Trim() }
& sc.exe qsidtype AetherCoreMaintenance 2>&1 | Select-String 'SERVICE_SID_TYPE' | ForEach-Object { Write-Output $_.Line.Trim() }
Write-Output '=== event log errors in last 10 min (should be none for AetherCore) ==='
$ev = Get-WinEvent -FilterHashtable @{ LogName='System'; StartTime=(Get-Date).AddMinutes(-10) } -ErrorAction SilentlyContinue |
    Where-Object { $_.Message -match 'AetherCore' -and $_.LevelDisplayName -match 'Error|Warning' }
if ($ev) { $ev | Select-Object -First 4 | ForEach-Object { Write-Output ("[$($_.Id)] $($_.Message)") } } else { Write-Output 'NO AetherCore errors/warnings' }
