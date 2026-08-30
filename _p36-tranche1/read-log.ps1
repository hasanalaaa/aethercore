param([string]$LogPath, [int]$Tail = 30)
Get-Content -LiteralPath $LogPath -ErrorAction Stop |
    Select-String -Pattern 'error 3|return value 3|CustomAction .* returned|Note: 1:|WARNING|DEBUG: Error|Action ended' |
    Select-Object -Last $Tail |
    ForEach-Object { Write-Output $_.Line }
