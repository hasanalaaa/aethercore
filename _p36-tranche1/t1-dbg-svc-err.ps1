$ErrorActionPreference = 'Continue'
$ws = 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery'
$inc = 'C:\AetherCore-P36\incoming'
Set-Location $ws
# why did the console service die? capture stderr synchronously
& cmd.exe /d /s /c "target\debug\aethercore-maintenance-service.exe --console 1>$inc\dbgsvc-out.txt 2>$inc\dbgsvc-err.txt"
Write-Output ("console service exit=" + $LASTEXITCODE)
Write-Output '--- stderr ---'
Get-Content "$inc\dbgsvc-err.txt" -ErrorAction SilentlyContinue | Select-Object -First 10 | ForEach-Object { Write-Output $_ }
Write-Output '--- stdout ---'
Get-Content "$inc\dbgsvc-out.txt" -ErrorAction SilentlyContinue | Select-Object -First 10 | ForEach-Object { Write-Output $_ }
