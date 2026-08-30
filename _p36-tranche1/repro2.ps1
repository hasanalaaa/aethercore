# P36 - check surviving service log + full-mirror repro of MSI ordering
$ErrorActionPreference = 'Continue'

Write-Output '=== surviving ProgramData\AetherCore contents after rollback ==='
Get-ChildItem 'C:\ProgramData\AetherCore' -Recurse -Force -ErrorAction SilentlyContinue | ForEach-Object { Write-Output ($_.FullName + '  (' + $_.Length + ' bytes)') }
$svcLog = 'C:\ProgramData\AetherCore\logs\service.jsonl'
if (Test-Path $svcLog) {
    Write-Output '=== service.jsonl content ==='
    Get-Content $svcLog | ForEach-Object { Write-Output $_ }
} else {
    Write-Output 'no service.jsonl survived'
}

Write-Output '=== FULL-MIRROR repro (MSI ordering: create auto -> sidtype unrestricted -> config delayed-auto -> sdset -> start) ==='
$diag = 'C:\AetherCore-P36\incoming\diag'
& sc.exe create AetherCoreDiag binPath= "`"$diag\aethercore-maintenance-service.exe`"" type= own start= auto error= normal 2>&1 | Out-String | Write-Output
& sc.exe sidtype AetherCoreDiag unrestricted 2>&1 | Out-String | Write-Output
& sc.exe config AetherCoreDiag start= delayed-auto obj= LocalSystem 2>&1 | Out-String | Write-Output
& sc.exe sdset AetherCoreDiag "D:(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWLOCRRC;;;AU)" 2>&1 | Out-String | Write-Output
Write-Output '--- qsidtype ---'
& sc.exe qsidtype AetherCoreDiag 2>&1 | Out-String | Write-Output
Write-Output '--- start attempt ---'
& sc.exe start AetherCoreDiag 2>&1 | Out-String | Write-Output
Start-Sleep 5
& sc.exe query AetherCoreDiag 2>&1 | Out-String | Write-Output
Get-Service AetherCoreDiag -ErrorAction SilentlyContinue | Format-List Name,Status | Out-String | Write-Output
Write-Output '--- service.jsonl now? ---'
if (Test-Path $svcLog) { Get-Content $svcLog -Tail 10 | ForEach-Object { Write-Output $_ } } else { Write-Output 'still no service.jsonl' }
Write-Output '--- cleanup ---'
& sc.exe stop AetherCoreDiag 2>&1 | Out-String | Write-Output
Start-Sleep 2
& sc.exe delete AetherCoreDiag 2>&1 | Out-String | Write-Output
