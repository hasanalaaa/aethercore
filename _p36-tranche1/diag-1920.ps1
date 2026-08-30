# P36 - diagnose service start failure: PE imports of payload service exe + System event log
$ErrorActionPreference = 'Continue'
$ro = 'C:\AetherCore-P36\toolchain\llvm-22.1.8\bin\llvm-readobj.exe'
Write-Output '=== DLL imports of aethercore-maintenance-service.exe (payload) ==='
& $ro --coff-imports 'C:\AetherCore-P36\incoming\payload\aethercore-maintenance-service.exe' 2>$null | Select-String 'Name:.*\.dll' | Select-Object -First 15 | ForEach-Object { Write-Output $_.Line.Trim() }
Write-Output '=== DLL imports of aethercore-desktop.exe (payload) ==='
& $ro --coff-imports 'C:\AetherCore-P36\incoming\payload\aethercore-desktop.exe' 2>$null | Select-String 'Name:.*\.dll' | Select-Object -First 15 | ForEach-Object { Write-Output $_.Line.Trim() }
Write-Output '=== System event log: service control manager, last 30 min ==='
$since = (Get-Date).AddMinutes(-30)
Get-WinEvent -FilterHashtable @{ LogName = 'System'; ProviderName = 'Service Control Manager'; StartTime = $since } -ErrorAction SilentlyContinue |
    Where-Object { $_.Message -match 'AetherCore' } |
    Select-Object -First 6 |
    ForEach-Object { Write-Output ("[$($_.Id)] $($_.TimeCreated) :: " + ($_.Message -replace "`r`n", ' ')) }
Write-Output '=== service presence after rollback ==='
$svc = Get-Service -Name AetherCoreMaintenance -ErrorAction SilentlyContinue
if ($svc) { Write-Output ("present: " + $svc.Status) } else { Write-Output 'AetherCoreMaintenance NOT present (rolled back)' }
Write-Output '=== leftover install dir after rollback ==='
$pf = Join-Path $env:ProgramFiles 'AetherCore'
if (Test-Path $pf) { Get-ChildItem $pf | ForEach-Object { Write-Output $_.Name } } else { Write-Output 'install dir absent (fully rolled back)' }
