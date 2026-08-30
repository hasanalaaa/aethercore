Get-Content -LiteralPath 'C:\AetherCore-P36\incoming\tranche1-wix-build2.log' -Tail 12 | ForEach-Object { Write-Output $_ }
