$ErrorActionPreference = 'Continue'
Write-Output '=== service.jsonl tail (looking for StandardUser session admission evidence) ==='
Get-Content 'C:\ProgramData\AetherCore\logs\service.jsonl' -Tail 15 | ForEach-Object { Write-Output $_ }
Write-Output '=== service still RUNNING? ==='
& sc.exe query AetherCoreMaintenance 2>&1 | Select-String 'STATE' | ForEach-Object { Write-Output $_.Line.Trim() }
Write-Output '=== pipe list ==='
[System.IO.Directory]::GetFiles('\\.\pipe\') | Where-Object { $_ -like '*Aether*' } | ForEach-Object { Write-Output $_ }
