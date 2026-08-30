# P36 - run the real service binary in console mode (no SCM) to see the raw error or a live server
$ErrorActionPreference = 'Continue'
$diag = 'C:\AetherCore-P36\incoming\diag'
Write-Output '=== direct exe run: --console (composition + server, no SCM) ==='
$p = Start-Process -FilePath "$diag\aethercore-maintenance-service.exe" -ArgumentList '--console' -PassThru -WindowStyle Hidden -RedirectStandardError "$diag\console-stderr.txt" -RedirectStandardOutput "$diag\console-stdout.txt"
Start-Sleep 12
if ($p.HasExited) {
    Write-Output ("EXITED code=" + $p.ExitCode)
} else {
    Write-Output ("STILL RUNNING pid=" + $p.Id + "  (binary works; killing)")
    Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
}
Write-Output '=== stdout ==='
Get-Content "$diag\console-stdout.txt" -ErrorAction SilentlyContinue | Select-Object -First 20 | ForEach-Object { Write-Output $_ }
Write-Output '=== stderr ==='
Get-Content "$diag\console-stderr.txt" -ErrorAction SilentlyContinue | Select-Object -First 20 | ForEach-Object { Write-Output $_ }
Write-Output '=== pipes created? ==='
[System.IO.Directory]::GetFiles('\\.\pipe\') | Where-Object { $_ -like '*aether*' } | ForEach-Object { Write-Output $_ }
Write-Output '=== service.jsonl / logs now? ==='
Get-ChildItem 'C:\ProgramData\AetherCore' -Recurse -Force -ErrorAction SilentlyContinue | ForEach-Object { Write-Output ($_.FullName + '  (' + $_.Length + ')') }
if (Test-Path 'C:\ProgramData\AetherCore\logs\service.jsonl') { Write-Output '--- tail ---'; Get-Content 'C:\ProgramData\AetherCore\logs\service.jsonl' -Tail 12 | ForEach-Object { Write-Output $_ } }
