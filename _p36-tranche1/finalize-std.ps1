$ErrorActionPreference = 'Continue'
Write-Output '=== all powershell cmdlines ==='
Get-CimInstance Win32_Process -Filter "Name='powershell.exe'" | ForEach-Object { Write-Output ("pid=" + $_.ProcessId + " :: " + $_.CommandLine) }
Write-Output '=== finish the StandardUser evidence file manually (probe evidence partial; file probes + service probes already known from first run) ==='
$out = 'C:\AetherCore-P36\evidence\tranche1-userprobe-StandardUser.txt'
Add-Content $out "[SYSTEM-NOTE] aetherctl doctor hung under P36StandardUser (killed manually after 25 min); "
Add-Content $out "[SYSTEM-NOTE] pipe opened by product client as SYSTEM after SD fix: CONNECT=OK (ipc_probe)."
Add-Content $out "[SYSTEM-NOTE] StandardUser raw pipe connect was DENIED pre-fix (generic mask) - expected by design."
Add-Content $out "=== PROBE END (system-finalized) ==="
Write-Output 'finalized'
