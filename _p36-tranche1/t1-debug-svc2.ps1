# P36 T1 - console service + full-round-trip probe with stderr capture to files
$ErrorActionPreference = 'Continue'
$inc = 'C:\AetherCore-P36\incoming'
$ws = 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery'
Set-Location $ws
Write-Output '=== stop real service ==='
& sc.exe stop AetherCoreMaintenance 2>&1 | Out-Null
Start-Sleep 5
Write-Output '=== launch debug console service (output to files) ==='
$p = Start-Process -FilePath 'target\debug\aethercore-maintenance-service.exe' `
    -ArgumentList '--console' `
    -WorkingDirectory $ws `
    -RedirectStandardOutput "$inc\console-svc-stdout.txt" `
    -RedirectStandardError "$inc\tranche1-console-stderr.txt" `
    -PassThru -WindowStyle Hidden
Start-Sleep 8
Write-Output ("service alive: " + (-not $p.HasExited))
Write-Output '=== drive one GetPerformanceSnapshot request ==='
& 'target\release\examples\ipc_request_probe.exe' 2>&1 | ForEach-Object { Write-Output $_ }
Start-Sleep 3
Write-Output '=== console service stdout so far ==='
& cmd.exe /d /s /c "type NUL" | Out-Null
Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
Start-Sleep 2
# read captured streams from the files Start-Process wrote
Write-Output '--- stdout ---'
Get-Content "$ws\tranche1-console-stdout.txt" -ErrorAction SilentlyContinue | Select-Object -First 15 | ForEach-Object { Write-Output $_ }
