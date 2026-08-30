# P36 - controlled repro: does the real service binary + hardener SDDL start outside the MSI?
# Purely diagnostic; every object created here is removed again and recorded.
$ErrorActionPreference = 'Continue'
$diag = 'C:\AetherCore-P36\incoming\diag'
New-Item -ItemType Directory -Force $diag | Out-Null
Copy-Item 'C:\AetherCore-P36\incoming\payload\aethercore-maintenance-service.exe' "$diag\aethercore-maintenance-service.exe" -Force

Write-Output '=== SCM manager SD (context check) ==='
& sc.exe sdshow SCMANAGER 2>&1 | Out-String | Write-Output

Write-Output '=== create diag service (LocalSystem, own process, demand start) ==='
& sc.exe create AetherCoreDiag binPath= "`"$diag\aethercore-maintenance-service.exe`"" type= own start= demand obj= LocalSystem 2>&1 | Out-String | Write-Output

Write-Output '=== qc before hardening ==='
& sc.exe qc AetherCoreDiag 2>&1 | Out-String | Write-Output

Write-Output '=== apply EXACT hardener SDDL (from apps/install-hardener/src/main.rs) ==='
& sc.exe sdset AetherCoreDiag "D:(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWLOCRRC;;;AU)" 2>&1 | Out-String | Write-Output

Write-Output '=== try start ==='
& sc.exe start AetherCoreDiag 2>&1 | Out-String | Write-Output
Start-Sleep 3
& sc.exe query AetherCoreDiag 2>&1 | Out-String | Write-Output

Write-Output '=== service status + recent SCM events ==='
Get-Service AetherCoreDiag -ErrorAction SilentlyContinue | Format-List Name,Status,StartType | Out-String | Write-Output
Get-WinEvent -FilterHashtable @{ LogName='System'; ProviderName='Service Control Manager'; StartTime=(Get-Date).AddMinutes(-2) } -ErrorAction SilentlyContinue |
    Select-Object -First 6 | ForEach-Object { Write-Output ("[$($_.Id)] $($_.TimeCreated) :: $($_.Message)") }

Write-Output '=== cleanup diag service ==='
& sc.exe stop AetherCoreDiag 2>&1 | Out-String | Write-Output
Start-Sleep 2
& sc.exe delete AetherCoreDiag 2>&1 | Out-String | Write-Output
