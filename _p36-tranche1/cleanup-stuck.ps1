$ErrorActionPreference = 'Continue'
Write-Output '=== stop + delete stuck service ==='
& sc.exe stop AetherCoreMaintenance 2>&1 | Out-String | Write-Output
Start-Sleep 2
& sc.exe delete AetherCoreMaintenance 2>&1 | Out-String | Write-Output
Write-Output '=== uninstall stuck product (DISABLEROLLBACK install) ==='
$p = Start-Process msiexec.exe -ArgumentList "/x", "{FC8A3841-759D-B452-1864-161F84F56C03}", "/qn", "/norestart", "/l*v", "C:\AetherCore-P36\logs\tranche1-msi-cleanup-uninstall.log" -Wait -PassThru
Write-Output ("msiexec /x exit: " + $p.ExitCode)
Start-Sleep 2
Write-Output '=== verify clean ==='
& sc.exe query AetherCoreMaintenance 2>&1 | Out-String | Write-Output
Write-Output ("install dir exists: " + (Test-Path 'C:\Program Files\AetherCore'))
$u = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*' -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -like '*AetherCore*' }
Write-Output ("uninstall entry: " + $(if ($u) { 'PRESENT' } else { 'absent' }))
Write-Output 'CLEANUP-DONE'
