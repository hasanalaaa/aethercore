param([string]$LogPath)
Get-Content -LiteralPath $LogPath -ErrorAction Stop |
    Select-String -Pattern 'VersionNT64|MsiNTProductType|WindowsBuild|22621|requires Windows|LaunchConditions|Launching|Skipping' |
    ForEach-Object { Write-Output $_.Line }
