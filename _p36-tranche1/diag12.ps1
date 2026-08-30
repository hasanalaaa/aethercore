$log = 'C:\AetherCore-P36\logs\tranche1-msi-install2.log'
Write-Output '=== hardener + services sequence this run ==='
Get-Content -LiteralPath $log |
    Select-String -Pattern 'HardenInstalledSecurity|InstallServices|StartServices|sc\.exe|icacls|sdset|sidtype|Note: 1: 2898' |
    Select-Object -Last 25 |
    ForEach-Object { Write-Output $_.Line }
Write-Output '=== custom action log lines (hardener writes to stderr only on failure) ==='
Get-Content -LiteralPath $log |
    Select-String -Pattern 'CustomAction HardenInstalledSecurity|MSI \(s\) .*: Invoking|exteral|return value' |
    Select-Object -Last 12 |
    ForEach-Object { Write-Output $_.Line }
