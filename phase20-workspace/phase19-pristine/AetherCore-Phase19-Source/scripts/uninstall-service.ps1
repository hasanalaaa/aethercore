#Requires -RunAsAdministrator
$ErrorActionPreference='Stop'
$svc=Get-Service AetherCoreMaintenance -ErrorAction SilentlyContinue
if ($svc) { if ($svc.Status -ne 'Stopped') { Stop-Service AetherCoreMaintenance -Force }; & sc.exe delete AetherCoreMaintenance | Out-Null }
Write-Host 'AetherCoreMaintenance removed.' -ForegroundColor Green
