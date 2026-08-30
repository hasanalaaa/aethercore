$ErrorActionPreference = 'Continue'
$exe = 'C:\Program Files\AetherCore\aethercore-maintenance-service.exe'
Write-Output '=== current SDDL ==='
(Get-Acl $exe).Sddl | Write-Output
Write-Output '=== try /reset (restore inherited ACEs from dir) ==='
& icacls.exe $exe /reset 2>&1 | Out-String | Write-Output
Write-Output ("reset exit: " + $LASTEXITCODE)
(Get-Acl $exe).Sddl | Write-Output
Write-Output '=== owner check ==='
$acl = Get-Acl $exe
Write-Output ("Owner: " + $acl.Owner + "   Group: " + $acl.Group)
Write-Output '=== takeown then grant ==='
& takeown.exe /F $exe 2>&1 | Out-String | Write-Output
& icacls.exe $exe /grant:r "*S-1-5-18:F" "*S-1-5-32-544:F" "*S-1-5-32-545:RX" "NT SERVICE\AetherCoreMaintenance:(RX)" 2>&1 | Out-String | Write-Output
(Get-Acl $exe).Sddl | Write-Output
Write-Output '=== start attempt ==='
& sc.exe start AetherCoreMaintenance 2>&1 | Out-String | Write-Output
Start-Sleep 5
& sc.exe query AetherCoreMaintenance 2>&1 | Out-String | Write-Output
