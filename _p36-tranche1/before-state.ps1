# P36 Tranche 1 - BEFORE-STATE capture (SYSTEM context, read-only)
$ErrorActionPreference = 'Continue'
$evid = 'C:\AetherCore-P36\evidence'
$logs = 'C:\AetherCore-P36\logs'
$out = Join-Path $logs 'tranche1-before-state.txt'
"=== P36 TRANCHE1 BEFORE-STATE ===" | Out-File $out -Encoding utf8
"Context: $(whoami)  Date: $(Get-Date -Format o)" | Out-File $out -Append -Encoding utf8
"OS: $([System.Environment]::OSVersion.VersionString)  Arch: $env:PROCESSOR_ARCHITECTURE" | Out-File $out -Append -Encoding utf8

"`n--- STALE PROCESS CHECK ---" | Out-File $out -Append -Encoding utf8
$procs = Get-Process -Name msiexec,cargo,rustc,clang-cl,aethercore-maintenance-service,aethercore-install-hardener,aethercore-consent-broker,aethercore-update-broker,aethercore-desktop -ErrorAction SilentlyContinue
if ($procs) { $procs | Format-Table Id,ProcessName,StartTime | Out-String | Out-File $out -Append -Encoding utf8; "STALE_PROCESS_FOUND=True" | Out-File $out -Append -Encoding utf8 }
else { "STALE_PROCESS_FOUND=False (no msiexec/cargo/rustc/clang-cl/aethercore processes)" | Out-File $out -Append -Encoding utf8 }

"`n--- INSTALLED AETHERCORE PRODUCTS (registry uninstall) ---" | Out-File $out -Append -Encoding utf8
$keys = @('HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*','HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*','HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*')
$found = Get-ItemProperty $keys -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -like '*AetherCore*' }
if ($found) { $found | Select-Object DisplayName,DisplayVersion,PSChildName | Format-List | Out-String | Out-File $out -Append -Encoding utf8; "PRODUCTS_INSTALLED=True" | Out-File $out -Append -Encoding utf8 }
else { "PRODUCTS_INSTALLED=False (no AetherCore entry in any uninstall hive)" | Out-File $out -Append -Encoding utf8 }

"`n--- AETHERCORE SERVICES ---" | Out-File $out -Append -Encoding utf8
$svc = Get-Service -Name 'AetherCoreMaintenance' -ErrorAction SilentlyContinue
if ($svc) { $svc | Format-List Name,Status,StartType | Out-String | Out-File $out -Append -Encoding utf8; "SERVICE_PRESENT=True" | Out-File $out -Append -Encoding utf8 }
else { "SERVICE_PRESENT=False (AetherCoreMaintenance not registered - expected pre-install state)" | Out-File $out -Append -Encoding utf8 }

"`n--- SERVICE SDBefore (sc sdshow) ---" | Out-File $out -Append -Encoding utf8
$sd = & sc.exe sdshow AetherCoreMaintenance 2>&1 | Out-String
$sd | Out-File $out -Append -Encoding utf8

"`n--- PROGRAM FILES / PROGRAMDATA PATHS ---" | Out-File $out -Append -Encoding utf8
$pf = Join-Path $env:ProgramFiles 'AetherCore'
$pd = Join-Path $env:ProgramData 'AetherCore'
"ProgramFiles path: $pf  Exists: $(Test-Path $pf)" | Out-File $out -Append -Encoding utf8
"ProgramData path:  $pd  Exists: $(Test-Path $pd)" | Out-File $out -Append -Encoding utf8
if (Test-Path $pf) { Get-ChildItem $pf -Recurse | Select-Object FullName,Length | Format-Table | Out-String | Out-File $out -Append -Encoding utf8; icacls $pf | Out-String | Out-File $out -Append -Encoding utf8 }
if (Test-Path $pd) { Get-ChildItem $pd -Recurse | Select-Object FullName,Length | Format-Table | Out-String | Out-File $out -Append -Encoding utf8; icacls $pd | Out-String | Out-File $out -Append -Encoding utf8 }

"`n--- REGISTRY HKLM:\Software\AetherCore ---" | Out-File $out -Append -Encoding utf8
if (Test-Path 'HKLM:\Software\AetherCore') { Get-ItemProperty 'HKLM:\Software\AetherCore' | Format-List | Out-String | Out-File $out -Append -Encoding utf8; "HKLM_KEY_PRESENT=True" | Out-File $out -Append -Encoding utf8 }
else { "HKLM_KEY_PRESENT=False" | Out-File $out -Append -Encoding utf8 }

"`n--- NAMED PIPES (aethercore) ---" | Out-File $out -Append -Encoding utf8
$pipes = [System.IO.Directory]::GetFiles('\\.\pipe\') | Where-Object { $_ -like '*aether*' }
if ($pipes) { $pipes | Out-File $out -Append -Encoding utf8; "PIPE_PRESENT=True" | Out-File $out -Append -Encoding utf8 } else { "PIPE_PRESENT=False" | Out-File $out -Append -Encoding utf8 }

"`n=== END BEFORE-STATE ===" | Out-File $out -Append -Encoding utf8
Get-Content $out
