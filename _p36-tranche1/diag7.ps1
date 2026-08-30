$ErrorActionPreference = 'Continue'
$diag = 'C:\AetherCore-P36\incoming\diag'
Write-Output '=== resolve NT SERVICE\AetherCoreMaintenance SID right now (after sidtype unrestricted was set earlier) ==='
$acct = 'NT SERVICE\AetherCoreMaintenance'
$null = [System.Reflection.Assembly]::LoadWithPartialName('System.ServiceProcess')
try {
    $sid = (New-Object Security.Principal.NTAccount($acct)).Translate([Security.Principal.SecurityIdentifier]).Value
    Write-Output ("translated SID: $sid")
} catch {
    Write-Output ("translate FAILED: " + $_.Exception.Message)
}
& sc.exe showsid AetherCoreMaintenance 2>&1 | Out-String | Write-Output
& sc.exe qsidtype AetherCoreMaintenance 2>&1 | Out-String | Write-Output
Write-Output '=== enum machine SIDs for a service named AetherCoreMaintenance ==='
& sc.exe queryex AetherCoreMaintenance 2>&1 | Out-String | Write-Output
