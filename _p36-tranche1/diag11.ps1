Write-Output '=== SCM events last 5 min ==='
Get-WinEvent -FilterHashtable @{ LogName='System'; ProviderName='Service Control Manager'; StartTime=(Get-Date).AddMinutes(-5) } -ErrorAction SilentlyContinue |
    Where-Object { $_.Message -match 'AetherCore' } |
    Select-Object -First 4 | ForEach-Object { Write-Output ("[$($_.Id)] $($_.TimeCreated) :: $($_.Message)") }
Write-Output '=== install dir after rollback attempt ==='
Get-ChildItem 'C:\Program Files\AetherCore' -ErrorAction SilentlyContinue | ForEach-Object { Write-Output $_.Name }
if (-not (Test-Path 'C:\Program Files\AetherCore')) { Write-Output '(absent - rolled back)' }
Write-Output '=== last MSI log lines around failure ==='
Get-Content -LiteralPath 'C:\AetherCore-P36\logs\tranche1-msi-install2.log' |
    Select-String -Pattern 'Error 19|Access is denied|did not respond|Return value 3' |
    Select-Object -Last 6 |
    ForEach-Object { Write-Output $_.Line }
