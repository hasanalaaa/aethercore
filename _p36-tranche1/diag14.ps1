$ErrorActionPreference = 'Continue'
$exe = 'C:\Program Files\AetherCore\aethercore-maintenance-service.exe'
Write-Output '=== Get-Acl SDDL of service exe (empty-DACL state) ==='
(Get-Acl $exe).Sddl | Write-Output
Write-Output '=== apply hardener grant line to the exe manually ==='
& icacls.exe $exe /grant:r "*S-1-5-18:(OI)(CI)F" "*S-1-5-32-544:(OI)(CI)F" "*S-1-5-32-545:(OI)(CI)RX" "NT SERVICE\AetherCoreMaintenance:(OI)(CI)RX" 2>&1 | Out-String | Write-Output
Write-Output ("icacls exit: " + $LASTEXITCODE)
Write-Output '=== exe ACL after manual grant ==='
& icacls.exe $exe 2>&1 | Out-String | Write-Output
(Get-Acl $exe).Sddl | Write-Output
Write-Output '=== try service start now ==='
& sc.exe start AetherCoreMaintenance 2>&1 | Out-String | Write-Output
Start-Sleep 5
& sc.exe query AetherCoreMaintenance 2>&1 | Out-String | Write-Output
[System.IO.Directory]::GetFiles('\\.\pipe\') | Where-Object { $_ -like '*aether*' } | ForEach-Object { Write-Output $_ }
