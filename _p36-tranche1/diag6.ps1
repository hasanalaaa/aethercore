$ErrorActionPreference = 'Continue'
$diag = 'C:\AetherCore-P36\incoming\diag'
Write-Output '=== run with libomp beside exe, capture stderr via cmd redirection ==='
& cmd.exe /d /s /c "cd /d $diag && aethercore-maintenance-service.exe --console 1>stdout.txt 2>stderr.txt & echo EXITCODE=%errorlevel%"
Write-Output '--- stdout.txt ---'
Get-Content "$diag\stdout.txt" -ErrorAction SilentlyContinue | Select-Object -First 15 | ForEach-Object { Write-Output $_ }
Write-Output '--- stderr.txt ---'
Get-Content "$diag\stderr.txt" -ErrorAction SilentlyContinue | Select-Object -First 15 | ForEach-Object { Write-Output $_ }
Write-Output '--- service.jsonl (should exist now if log init ran) ---'
if (Test-Path 'C:\ProgramData\AetherCore\logs\service.jsonl') { Get-Content 'C:\ProgramData\AetherCore\logs\service.jsonl' -Tail 10 | ForEach-Object { Write-Output $_ } } else { Write-Output 'still none' }
Write-Output '--- pipes ---'
[System.IO.Directory]::GetFiles('\\.\pipe\') | Where-Object { $_ -like '*aether*' } | ForEach-Object { Write-Output $_ }
