$ErrorActionPreference = 'Continue'
Write-Output '=== restore service after Admin probe stopped it ==='
$q = (& sc.exe query AetherCoreMaintenance | Out-String)
if ($q -match 'STOPPED') {
    & sc.exe start AetherCoreMaintenance 2>&1 | Out-Null
    Start-Sleep 6
}
& sc.exe query AetherCoreMaintenance 2>&1 | Select-String 'STATE' | ForEach-Object { Write-Output $_.Line.Trim() }
Write-Output '=== check ACLs of trust json + mutation lock as evidence (Admin could write both) ==='
Write-Output '--- update-trust.json ---'
& icacls 'C:\Program Files\AetherCore\update-trust.json' | Out-String | Write-Output
Write-Output '--- machine-mutation.lock ---'
& icacls 'C:\ProgramData\AetherCore\state\machine-mutation.lock' | Out-String | Write-Output
(Get-Acl 'C:\Program Files\AetherCore\update-trust.json').Sddl | Write-Output
(Get-Acl 'C:\ProgramData\AetherCore\state\machine-mutation.lock').Sddl | Write-Output
