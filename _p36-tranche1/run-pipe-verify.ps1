$ErrorActionPreference = 'Continue'
Set-Location 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery'
Write-Output '=== run project-native verify-ipc-pipe-security.ps1 (SYSTEM) ==='
& powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'scripts\verify-ipc-pipe-security.ps1' -OutputPath 'C:\AetherCore-P36\evidence\tranche1-ipc-pipe-security.json' 2>&1 | Out-String | Write-Output
Write-Output ("exit=" + $LASTEXITCODE)
if (Test-Path 'C:\AetherCore-P36\evidence\tranche1-ipc-pipe-security.json') {
    Get-Content 'C:\AetherCore-P36\evidence\tranche1-ipc-pipe-security.json' | Write-Output
}
