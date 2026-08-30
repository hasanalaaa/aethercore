# P36 Tranche 1 - stop + remove diag service, then add runtime DLL to payload dir
$ErrorActionPreference = 'Continue'
Write-Output '=== stop + delete diag service ==='
& sc.exe stop AetherCoreMaintenance 2>&1 | Out-String | Write-Output
Start-Sleep 3
& sc.exe delete AetherCoreMaintenance 2>&1 | Out-String | Write-Output
Start-Sleep 2
& sc.exe query AetherCoreMaintenance 2>&1 | Out-String | Write-Output

Write-Output '=== clean ProgramData state created by the diag run ==='
# keep machine-mutation.lock (pre-existing); remove service.jsonl and logs dir created during diagnosis
$logDir = 'C:\ProgramData\AetherCore\logs'
if (Test-Path $logDir) { Remove-Item $logDir -Recurse -Force; Write-Output "removed $logDir" }
$db = 'C:\ProgramData\AetherCore\state\aethercore.db'
if (Test-Path $db) { Remove-Item $db -Force; Write-Output "removed $db" }

Write-Output '=== add libomp140.aarch64.dll to payload dir ==='
Copy-Item 'C:\AetherCore-P36\toolchain\vs2022\VC\Redist\MSVC\14.44.35112\debug_nonredist\arm64\Microsoft.VC143.OpenMP.LLVM\libomp140.aarch64.dll' 'C:\AetherCore-P36\incoming\payload\libomp140.aarch64.dll' -Force
$h = (Get-FileHash 'C:\AetherCore-P36\incoming\payload\libomp140.aarch64.dll' -Algorithm SHA256).Hash
Write-Output ("libomp140.aarch64.dll SHA256=" + $h)
Write-Output '=== payload dir now ==='
Get-ChildItem 'C:\AetherCore-P36\incoming\payload' | ForEach-Object { Write-Output ($_.Name + '  ' + $_.Length) }
