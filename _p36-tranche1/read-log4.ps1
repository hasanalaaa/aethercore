Get-Content -LiteralPath 'C:\AetherCore-P36\logs\tranche1-msi-install2.log' |
    Select-String -Pattern 'Error 1920|Error 1923|failed to start|Return value 3|OSCURRENTBUILD|Access is denied|did not respond' |
    Select-Object -Last 10 |
    ForEach-Object { Write-Output $_.Line }
