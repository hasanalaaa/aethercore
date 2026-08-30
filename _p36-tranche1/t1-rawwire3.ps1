$ErrorActionPreference = 'Continue'
$ws = 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery'
$inc = 'C:\AetherCore-P36\incoming'
Set-Location $ws
# The probe HUNG - meaning even the raw-wire response read blocks. This is itself the
# decisive data point (server never sends a response frame). Re-run with output streaming
# to a file so partial progress survives, plus a global 45s killer.
$log = "$inc\rawwire-run.txt"
$p = Start-Process -FilePath 'target\release\examples\ipc_rawwire_probe.exe' `
    -PassThru -WindowStyle Hidden `
    -RedirectStandardOutput "$inc\rawwire-stdout.txt" `
    -RedirectStandardError "$inc\rawwire-stderr.txt"
$fin = $p.WaitForExit(45000)
if ($fin) {
    "exit=$($p.ExitCode)" | Set-Content $log
} else {
    Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
    "TIMED_OUT_45S killed" | Set-Content $log
}
Get-Content $log
Write-Output '--- stdout ---'
Get-Content "$inc\rawwire-stdout.txt" -ErrorAction SilentlyContinue | ForEach-Object { Write-Output $_ }
Write-Output '--- stderr ---'
Get-Content "$inc\rawwire-stderr.txt" -ErrorAction SilentlyContinue | ForEach-Object { Write-Output $_ }
Write-Output '--- service log tail ---'
Get-Content 'C:\ProgramData\AetherCore\logs\service.jsonl' -Tail 4 | ForEach-Object { Write-Output $_ }
