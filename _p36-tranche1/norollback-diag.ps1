$ErrorActionPreference = 'Continue'
$inc = 'C:\AetherCore-P36\incoming'
$logs = 'C:\AetherCore-P36\logs'
$msi = 'C:\AetherCore-P36\incoming\AetherCore-0.1.0-arm64.msi'
# Diagnostic install with rollback DISABLED so the failing state stays inspectable.
$ilog = "$logs\tranche1-msi-install-norollback.log"
$p = Start-Process msiexec.exe -ArgumentList "/i", "`"$msi`"", "/l*v", "`"$ilog`"", "DISABLEROLLBACK=1", "/qn", "/norestart" -Wait -PassThru
Write-Output ("msiexec exit: " + $p.ExitCode)

Write-Output '=== state at failure (kept because rollback disabled) ==='
& sc.exe query AetherCoreMaintenance 2>&1 | Out-String | Write-Output
& sc.exe qc AetherCoreMaintenance 2>&1 | Out-String | Write-Output
& sc.exe qsidtype AetherCoreMaintenance 2>&1 | Out-String | Write-Output
& sc.exe sdshow AetherCoreMaintenance 2>&1 | Out-String | Write-Output
Write-Output '=== service exe ACL ==='
& icacls 'C:\Program Files\AetherCore\aethercore-maintenance-service.exe' 2>&1 | Out-String | Write-Output
Write-Output '=== dir ACL ==='
& icacls 'C:\Program Files\AetherCore' 2>&1 | Out-String | Write-Output
Write-Output '=== manual start attempt right now (SYSTEM, sc.exe) ==='
& sc.exe start AetherCoreMaintenance 2>&1 | Out-String | Write-Output
Start-Sleep 5
& sc.exe query AetherCoreMaintenance 2>&1 | Out-String | Write-Output
Get-WinEvent -FilterHashtable @{ LogName='System'; ProviderName='Service Control Manager'; StartTime=(Get-Date).AddMinutes(-4) } -ErrorAction SilentlyContinue |
    Where-Object { $_.Message -match 'AetherCore' } | Select-Object -First 3 |
    ForEach-Object { Write-Output ("[$($_.Id)] $($_.TimeCreated) :: $($_.Message)") }
