param([string]$LogPath)
Get-Content -LiteralPath $LogPath -ErrorAction Stop |
    Select-String -Pattern 'Product: AetherCore --|Action start|Action ended|2318|OSCURRENTBUILD|CurrentBuildNumber|Skipping action|incorrect|MainEngineThread' |
    ForEach-Object { Write-Output $_.Line }
