$ErrorActionPreference = 'Continue'
Write-Output '=== service state right now ==='
& sc.exe query AetherCoreMaintenance 2>&1 | Select-String 'STATE' | ForEach-Object { Write-Output $_.Line.Trim() }
Write-Output '=== restart if needed ==='
$q = (& sc.exe query AetherCoreMaintenance | Out-String)
if ($q -match 'STOPPED') {
    & sc.exe start AetherCoreMaintenance 2>&1 | Out-String | Write-Output
    Start-Sleep 6
}
& sc.exe query AetherCoreMaintenance 2>&1 | Select-String 'STATE' | ForEach-Object { Write-Output $_.Line.Trim() }
Write-Output '=== pipe present? ==='
[System.IO.Directory]::GetFiles('\\.\pipe\') | Where-Object { $_ -like '*Aether*' } | ForEach-Object { Write-Output $_ }
