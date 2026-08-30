$ro = 'C:\AetherCore-P36\toolchain\llvm-22.1.8\bin\llvm-readobj.exe'
Write-Output '=== ALL imports: aethercore-maintenance-service.exe ==='
& $ro --coff-imports 'C:\AetherCore-P36\incoming\payload\aethercore-maintenance-service.exe' 2>$null | Select-String 'Name:.*\.dll' | ForEach-Object { Write-Output $_.Line.Trim() }
Write-Output '=== ALL imports: aethercore-desktop.exe ==='
& $ro --coff-imports 'C:\AetherCore-P36\incoming\payload\aethercore-desktop.exe' 2>$null | Select-String 'Name:.*\.dll' | ForEach-Object { Write-Output $_.Line.Trim() }
Write-Output '=== ALL imports: aethercore-install-hardener.exe ==='
& $ro --coff-imports 'C:\AetherCore-P36\incoming\payload\aethercore-install-hardener.exe' 2>$null | Select-String 'Name:.*\.dll' | ForEach-Object { Write-Output $_.Line.Trim() }
Write-Output '=== ALL imports: aethercore-consent-broker.exe ==='
& $ro --coff-imports 'C:\AetherCore-P36\incoming\payload\aethercore-consent-broker.exe' 2>$null | Select-String 'Name:.*\.dll' | ForEach-Object { Write-Output $_.Line.Trim() }
Write-Output '=== ALL imports: aethercore-update-broker.exe ==='
& $ro --coff-imports 'C:\AetherCore-P36\incoming\payload\aethercore-update-broker.exe' 2>$null | Select-String 'Name:.*\.dll' | ForEach-Object { Write-Output $_.Line.Trim() }
Write-Output '=== OpenMP redist dir on VM ==='
$d = 'C:\AetherCore-P36\toolchain\vs2022\VC\Redist\MSVC\14.44.35112\debug_nonredist\arm64\Microsoft.VC143.OpenMP.LLVM'
Write-Output ("exists: " + (Test-Path $d))
if (Test-Path $d) { Get-ChildItem $d | ForEach-Object { Write-Output ($_.Name + '  ' + $_.Length) } }
