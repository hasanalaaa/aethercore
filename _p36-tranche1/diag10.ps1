Get-Content -LiteralPath 'C:\AetherCore-P36\incoming\tranche1-wix-build2.log' -ErrorAction SilentlyContinue |
    Select-String -Pattern 'warning|libomp|payload' |
    Select-Object -First 12 |
    ForEach-Object { Write-Output $_.Line }
Write-Output '--- payload dir at build time ---'
Get-ChildItem 'C:\AetherCore-P36\incoming\payload' | ForEach-Object { Write-Output ($_.Name + '  ' + $_.Length) }
