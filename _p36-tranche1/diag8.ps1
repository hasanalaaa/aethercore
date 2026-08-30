$ErrorActionPreference = 'Continue'
$diag = 'C:\AetherCore-P36\incoming\diag'
Write-Output '=== create AetherCoreMaintenance from the REAL payload exe (with libomp beside it), full MSI config ==='
& sc.exe create AetherCoreMaintenance binPath= "`"$diag\aethercore-maintenance-service.exe`"" type= own start= auto error= normal DisplayName= "AetherCore Maintenance Service" 2>&1 | Out-String | Write-Output
& sc.exe sidtype AetherCoreMaintenance unrestricted 2>&1 | Out-String | Write-Output
& sc.exe config AetherCoreMaintenance start= delayed-auto obj= LocalSystem 2>&1 | Out-String | Write-Output
& sc.exe sdset AetherCoreMaintenance "D:(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWLOCRRC;;;AU)" 2>&1 | Out-String | Write-Output
Write-Output '--- start ---'
& sc.exe start AetherCoreMaintenance 2>&1 | Out-String | Write-Output
Start-Sleep 6
& sc.exe query AetherCoreMaintenance 2>&1 | Out-String | Write-Output
Get-Service AetherCoreMaintenance -ErrorAction SilentlyContinue | Format-List Name,Status,StartType | Out-String | Write-Output
Write-Output '--- pipes ---'
[System.IO.Directory]::GetFiles('\\.\pipe\') | Where-Object { $_ -like '*aether*' } | ForEach-Object { Write-Output $_ }
Write-Output '--- service.jsonl tail ---'
if (Test-Path 'C:\ProgramData\AetherCore\logs\service.jsonl') { Get-Content 'C:\ProgramData\AetherCore\logs\service.jsonl' -Tail 8 | ForEach-Object { Write-Output $_ } } else { Write-Output 'none' }
