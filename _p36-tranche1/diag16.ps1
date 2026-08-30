Write-Output '=== latest install log: hardener & start services ==='
Get-Content -LiteralPath 'C:\AetherCore-P36\logs\tranche1-msi-install2.log' |
    Select-String -Pattern 'Error 19|Access is denied|did not respond|HardenInstalledSecurity|StartServices|Executing op: ServiceInstall|service failed' |
    Select-Object -Last 14 |
    ForEach-Object { Write-Output $_.Line }
Write-Output '=== service exe ACL right now (post-install state) ==='
& icacls.exe 'C:\Program Files\AetherCore\aethercore-maintenance-service.exe' 2>&1 | Out-String | Write-Output
(Get-Acl 'C:\Program Files\AetherCore\aethercore-maintenance-service.exe' -ErrorAction SilentlyContinue).Sddl | Write-Output
Write-Output '=== service present? ==='
& sc.exe query AetherCoreMaintenance 2>&1 | Out-String | Write-Output
