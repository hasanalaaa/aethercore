$ErrorActionPreference = 'Continue'
$t = 'C:\AetherCore-P36\incoming\acltest'
New-Item -ItemType Directory -Force $t | Out-Null
$f = "$t\probe.txt"
Set-Content $f 'probe'
Write-Output '=== baseline file SDDL ==='
(Get-Acl $f).Sddl | Write-Output
Write-Output '=== TEST 1: /grant with (OI)(CI) on a FILE (hardener pattern) ==='
& icacls.exe $f /grant:r "*S-1-5-18:(OI)(CI)F" "*S-1-5-32-544:(OI)(CI)F" 2>&1 | Out-String | Write-Output
Write-Output ("exit: " + $LASTEXITCODE)
(Get-Acl $f).Sddl | Write-Output
Write-Output '=== TEST 2: /inheritance:r then /grant on the file ==='
& icacls.exe $f /inheritance:r 2>&1 | Out-String | Write-Output
(Get-Acl $f).Sddl | Write-Output
& icacls.exe $f /grant:r "*S-1-5-18:F" "*S-1-5-32-544:F" "*S-1-5-32-545:RX" 2>&1 | Out-String | Write-Output
Write-Output ("exit: " + $LASTEXITCODE)
(Get-Acl $f).Sddl | Write-Output
Write-Output '=== TEST 3: dir-level (OI)(CI) grant + propagation into a fresh child ==='
$d = "$t\sub"
New-Item -ItemType Directory -Force $d | Out-Null
Set-Content "$d\child.txt" 'child'
& icacls.exe "$t" /grant:r "*S-1-5-18:(OI)(CI)F" 2>&1 | Out-String | Write-Output
(Get-Acl "$d\child.txt").Sddl | Write-Output
Write-Output '=== cleanup test tree ==='
Remove-Item $t -Recurse -Force
Write-Output 'cleaned'
