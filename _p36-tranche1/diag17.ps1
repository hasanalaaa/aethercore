Write-Output '=== find the actual failure in this run log ==='
Get-Content -LiteralPath 'C:\AetherCore-P36\logs\tranche1-msi-install2.log' |
    Select-String -Pattern 'Error 1|Error 2|Return value 3|failed|MainEngineThread' |
    Select-Object -Last 15 |
    ForEach-Object { Write-Output $_.Line }
