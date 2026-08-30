$ErrorActionPreference = 'Continue'
Write-Output '=== uninstall existing product first (1638 = another version installed) ==='
$p = Start-Process msiexec.exe -ArgumentList "/x", "{FC8A3841-759D-B452-1864-161F84F56C03}", "/qn", "/norestart", "/l*v", "C:\AetherCore-P36\logs\tranche1-msi-final-uninstall2.log" -Wait -PassThru
Write-Output "uninstall exit=$($p.ExitCode)"
Start-Sleep 3
Write-Output '=== now fresh install ==='
$ilog = 'C:\AetherCore-P36\logs\tranche1-msi-final-install.log'
$p2 = Start-Process msiexec.exe -ArgumentList "/i", "C:\AetherCore-P36\incoming\AetherCore-0.1.0-arm64.msi", "/l*v", "`"$ilog`"", "/qn", "/norestart" -Wait -PassThru
Write-Output "install exit=$($p2.ExitCode)"
Set-Content 'C:\AetherCore-P36\incoming\tranche1-stepC1.status' "EXIT=$($p2.ExitCode) (msiexec)"
Start-Sleep 8
& sc.exe query AetherCoreMaintenance 2>&1 | Select-String 'STATE' | ForEach-Object { Write-Output $_.Line.Trim() }
$h = (Get-FileHash 'C:\Program Files\AetherCore\aethercore-maintenance-service.exe' -Algorithm SHA256).Hash
Write-Output "installed service sha=$h"
$msiHash = (Get-FileHash 'C:\AetherCore-P36\incoming\AetherCore-0.1.0-arm64.msi' -Algorithm SHA256).Hash
Write-Output "FINAL MSI sha256=$msiHash bytes=$((Get-Item 'C:\AetherCore-P36\incoming\AetherCore-0.1.0-arm64.msi').Length)"
