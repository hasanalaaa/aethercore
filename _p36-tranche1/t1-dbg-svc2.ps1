$ErrorActionPreference = 'Continue'
$ws = 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery'
$inc = 'C:\AetherCore-P36\incoming'
Set-Location $ws
# debug exe needs libomp140 beside it too (clang-cl OpenMP import)
Copy-Item "$inc\payload\libomp140.aarch64.dll" 'target\debug\' -Force
Write-Output '=== launch debug console service with libomp beside it ==='
$p = Start-Process -FilePath "$ws\target\debug\aethercore-maintenance-service.exe" `
    -ArgumentList '--console' `
    -WorkingDirectory $ws `
    -RedirectStandardOutput "$inc\dbgsvc-out.txt" `
    -RedirectStandardError "$inc\dbgsvc-err.txt" `
    -PassThru -WindowStyle Hidden
Start-Sleep 8
Write-Output ("service alive: " + (-not $p.HasExited))
Write-Output '=== drive one GetPerformanceSnapshot request ==='
& 'target\release\examples\ipc_request_probe.exe' 2>&1 | ForEach-Object { Write-Output $_ }
Start-Sleep 3
Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
Start-Sleep 2
Write-Output '--- console service stdout (captured during run) ---'
Get-Content "$inc\dbgsvc-out.txt" -ErrorAction SilentlyContinue | Select-Object -First 12 | ForEach-Object { Write-Output $_ }
Write-Output '--- console service stderr ---'
Get-Content "$inc\dbgsvc-err.txt" -ErrorAction SilentlyContinue | Select-Object -First 12 | ForEach-Object { Write-Output $_ }
