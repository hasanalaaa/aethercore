# P36 Tranche 1 - per-user probe inner (runs AS target user; output to user temp)
# Bounded negative/positive probes; no destructive writes; artifacts cleaned.
$ErrorActionPreference = 'Continue'
$userLabel = $args[0]
$outFile = Join-Path $env:TEMP ("p36-probe-" + $userLabel + ".txt")
$cli = 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery\target\release\aetherctl.exe'
"=== P36 USER PROBE: $userLabel ===" | Out-File $outFile -Encoding utf8
"Context: $(whoami)  Time: $(Get-Date -Format o)  Temp: $env:TEMP" | Out-File $outFile -Append -Encoding utf8
$grp = (& whoami.exe /groups 2>&1 | Out-String)
"Mandatory label + admin membership:" | Out-File $outFile -Append -Encoding utf8
($grp -split "`r?`n" | Where-Object { $_ -match 'Mandatory Label|S-1-5-32-544' }) -join "`n" | Out-File $outFile -Append -Encoding utf8

"--- IPC: aetherctl doctor (real named-pipe connect + hello) ---" | Out-File $outFile -Append -Encoding utf8
$doc = & $cli doctor 2>&1 | Out-String
"exit=$LASTEXITCODE" | Out-File $outFile -Append -Encoding utf8
$doc | Out-File $outFile -Append -Encoding utf8

"--- IPC: aetherctl update status ---" | Out-File $outFile -Append -Encoding utf8
$us = & $cli update status 2>&1 | Out-String
"exit=$LASTEXITCODE" | Out-File $outFile -Append -Encoding utf8
$us | Out-File $outFile -Append -Encoding utf8

"--- IPC: aetherctl update check (trust disabled -> expect fail-closed) ---" | Out-File $outFile -Append -Encoding utf8
$uc = & $cli update check 2>&1 | Out-String
"exit=$LASTEXITCODE" | Out-File $outFile -Append -Encoding utf8
$uc | Out-File $outFile -Append -Encoding utf8

"--- FILE PROBE: open service exe for WRITE (no write performed) ---" | Out-File $outFile -Append -Encoding utf8
try {
    $fs = [System.IO.File]::Open('C:\Program Files\AetherCore\aethercore-maintenance-service.exe', 'Open', 'ReadWrite', 'None')
    $fs.Close()
    "WRITE_OPEN=SUCCESS (DEFECT if StandardUser)" | Out-File $outFile -Append -Encoding utf8
} catch {
    "WRITE_OPEN=DENIED: $($_.Exception.GetType().Name): $($_.Exception.Message)" | Out-File $outFile -Append -Encoding utf8
}

"--- FILE PROBE: open update-trust.json for WRITE (no content change) ---" | Out-File $outFile -Append -Encoding utf8
try {
    $fs = [System.IO.File]::Open('C:\Program Files\AetherCore\update-trust.json', 'Open', 'ReadWrite', 'None')
    $fs.Close()
    "TRUST_WRITE_OPEN=SUCCESS (DEFECT if StandardUser)" | Out-File $outFile -Append -Encoding utf8
} catch {
    "TRUST_WRITE_OPEN=DENIED: $($_.Exception.GetType().Name): $($_.Exception.Message)" | Out-File $outFile -Append -Encoding utf8
}

"--- FILE PROBE: create new file in install dir (harmless; deleted if created) ---" | Out-File $outFile -Append -Encoding utf8
$probe = 'C:\Program Files\AetherCore\p36-probe.tmp'
try {
    [System.IO.File]::WriteAllText($probe, 'probe')
    "CREATE=SUCCESS (DEFECT if StandardUser)" | Out-File $outFile -Append -Encoding utf8
    Remove-Item $probe -Force -ErrorAction SilentlyContinue
} catch {
    "CREATE=DENIED: $($_.Exception.GetType().Name): $($_.Exception.Message)" | Out-File $outFile -Append -Encoding utf8
}

"--- FILE PROBE: open machine-mutation.lock for WRITE ---" | Out-File $outFile -Append -Encoding utf8
try {
    $fs = [System.IO.File]::Open('C:\ProgramData\AetherCore\state\machine-mutation.lock', 'Open', 'ReadWrite', 'None')
    $fs.Close()
    "LOCK_WRITE_OPEN=SUCCESS (DEFECT if StandardUser)" | Out-File $outFile -Append -Encoding utf8
} catch {
    "LOCK_WRITE_OPEN=DENIED: $($_.Exception.GetType().Name): $($_.Exception.Message)" | Out-File $outFile -Append -Encoding utf8
}

"--- SERVICE CONTROL PROBE: sc stop AetherCoreMaintenance ---" | Out-File $outFile -Append -Encoding utf8
$stop = & sc.exe stop AetherCoreMaintenance 2>&1 | Out-String
"sc stop exit=$LASTEXITCODE" | Out-File $outFile -Append -Encoding utf8
$stop | Out-File $outFile -Append -Encoding utf8

"--- SERVICE CONTROL PROBE: sc config obj (must fail for StandardUser) ---" | Out-File $outFile -Append -Encoding utf8
$cfg = & sc.exe config AetherCoreMaintenance obj= .\NoSuchAccount 2>&1 | Out-String
"sc config exit=$LASTEXITCODE" | Out-File $outFile -Append -Encoding utf8
$cfg | Out-File $outFile -Append -Encoding utf8

"--- PIPE RAW CONNECT ---" | Out-File $outFile -Append -Encoding utf8
try {
    $ps = New-Object System.IO.Pipes.NamedPipeClientStream('.', 'AetherCore.Maintenance.v7', [System.IO.Pipes.PipeDirection]::InOut)
    $ps.Connect(3000)
    "PIPE_CONNECT=SUCCESS" | Out-File $outFile -Append -Encoding utf8
    $ps.Dispose()
} catch {
    "PIPE_CONNECT=DENIED: $($_.Exception.GetType().Name): $($_.Exception.Message)" | Out-File $outFile -Append -Encoding utf8
}
"=== PROBE END ===" | Out-File $outFile -Append -Encoding utf8
