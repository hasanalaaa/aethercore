Get-WinEvent -FilterHashtable @{ LogName='System'; ProviderName='Service Control Manager'; StartTime=(Get-Date).AddMinutes(-6) } -ErrorAction SilentlyContinue |
    Where-Object { $_.Message -match 'AetherCore' } |
    Select-Object -First 4 | ForEach-Object { Write-Output ("[$($_.Id)] $($_.TimeCreated) :: $($_.Message)") }
if (Test-Path 'C:\Program Files\AetherCore\libomp140.aarch64.dll') { Write-Output 'libomp INSTALLED beside service exe' } else { Write-Output 'libomp NOT in install dir' }
Get-ChildItem 'C:\Program Files\AetherCore' -ErrorAction SilentlyContinue | ForEach-Object { Write-Output $_.Name }
