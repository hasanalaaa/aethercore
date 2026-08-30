$ErrorActionPreference = 'Continue'
$inc = 'C:\AetherCore-P36\incoming'
$ws = 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery'
Set-Location $ws
Write-Output '=== hold test, sequential: hold client first (bg), then request client ==='
$pA = Start-Process -FilePath 'target\release\examples\ipc_two_client_probe.exe' -ArgumentList 'hold' -PassThru -WindowStyle Hidden -RedirectStandardOutput "$inc\hold-out.txt" -RedirectStandardError "$inc\hold-err.txt"
Start-Sleep -Seconds 1
$out1 = & 'target\release\examples\ipc_two_client_probe.exe' request 2>&1 | Out-String
Write-Output '--- request client result ---'
$out1 -split "`r?`n" | Where-Object { $_ } | ForEach-Object { Write-Output $_ }
# wait for holder
$pA.WaitForExit(20000) | Out-Null
Write-Output '--- holder output ---'
Get-Content "$inc\hold-out.txt" -ErrorAction SilentlyContinue | ForEach-Object { Write-Output $_ }
Write-Output '=== service log tail ==='
Get-Content 'C:\ProgramData\AetherCore\logs\service.jsonl' -Tail 3 | ForEach-Object { Write-Output $_ }
