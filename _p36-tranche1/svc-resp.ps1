$ErrorActionPreference = 'Continue'
$cli = 'C:\Program Files\AetherCore\ipc_probe.exe'
Write-Output '=== SYSTEM client connect (fresh, confirms service responsive) ==='
& $cli 2>&1 | Select-Object -First 2 | Out-String | Write-Output
Write-Output '=== service log tail after connect ==='
Get-Content 'C:\ProgramData\AetherCore\logs\service.jsonl' -Tail 3 | ForEach-Object { Write-Output $_ }
