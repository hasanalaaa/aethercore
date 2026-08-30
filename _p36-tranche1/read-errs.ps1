Get-Content -LiteralPath 'C:\AetherCore-P36\incoming\tranche1-rawwire-build.log' |
    Select-String -Pattern '^error' -Context 0,3 |
    Select-Object -First 4 |
    ForEach-Object { Write-Output $_.Line; $_.Context.PostContext | ForEach-Object { Write-Output $_ } }
