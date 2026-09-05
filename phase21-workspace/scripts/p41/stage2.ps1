<#
.SYNOPSIS
  P41 Stage 2 - install the x64 MSI on a physical machine and verify it against
  the ARM64 baseline recorded in docs/phase36/SESSION_CONTEXT.md.

.WHY THE PREFLIGHT
  This machine has NO VM snapshot and NO disk image. A System Restore point is the
  only recovery that exists, and a point created AFTER the first install cannot
  recover the first install. So recovery is a HARD GATE here, not a step someone
  has to remember: the script enumerates restore points, creates one only if it
  must, re-enumerates to PROVE it (Checkpoint-Computer returning OK is not proof,
  the listing is), and REFUSES to install if it still cannot verify one.

.NOTE ON ENCODING
  Deliberately ASCII-only. Windows PowerShell 5.1 reads a UTF-8 file with no BOM
  as ANSI, which turns an em dash into mojibake and has already broken this file
  once. Do not reintroduce non-ASCII characters.

.USAGE
  Run from an ELEVATED PowerShell:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts\p41\stage2.ps1
  Results: C:\AetherCore-P41\logs\stage2.log  (override with -OutDir)
#>
param(
    [string]$OutDir = 'C:\AetherCore-P41\logs',
    [string]$Msi    = 'C:\dev\aethercore\phase21-workspace\out\release\AetherCore.msi'
)
$ErrorActionPreference = 'Continue'
New-Item -ItemType Directory -Force $OutDir | Out-Null
$log = Join-Path $OutDir 'stage2.log'
function W($s){ Add-Content -Path $log -Value $s -Encoding UTF8 }
Set-Content -Path $log -Value ("STAGE 2 INSTALL+VERIFY " + (Get-Date -Format o)) -Encoding UTF8

$elev = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
W ("Elevated: " + $elev)
if (-not $elev) { W "FATAL: not elevated. Start an admin PowerShell and re-run."; W "STAGE2_DONE"; exit 2 }

$IF     = 'C:\Program Files\AetherCore'
$MSILOG = Join-Path $OutDir 'msi-install.log'

# ---------------------------------------------------------------------------
# [0] PREFLIGHT - a verified restore point MUST exist before anything installs
# ---------------------------------------------------------------------------
W "=== [0] PREFLIGHT: verified restore point required before install ==="
function RpTime($r){ try { [Management.ManagementDateTimeConverter]::ToDateTime($r.CreationTime) } catch { [datetime]::MinValue } }

$rp = @()
try { $rp = @(Get-ComputerRestorePoint -ErrorAction Stop) } catch { W ("Get-ComputerRestorePoint FAILED: " + $_.Exception.Message) }
W ("EXISTING_RESTORE_POINTS=" + $rp.Count)
foreach ($r in $rp) { W ("  seq=" + $r.SequenceNumber + " type=" + $r.RestorePointType + " created=" + $r.CreationTime + " desc=" + $r.Description) }

# Usable = created within the last 24h, i.e. still reflecting this pre-install machine.
$cutoff = (Get-Date).AddHours(-24)
$usable = @($rp | Where-Object { (RpTime $_) -ge $cutoff })
W ("USABLE_RECENT_POINTS=" + $usable.Count)

if ($usable.Count -eq 0) {
    W "No usable recent restore point. Creating one BEFORE the install."
    try { Enable-ComputerRestore -Drive 'C:\' -ErrorAction Stop; W "Enable-ComputerRestore C: -> OK" }
    catch { W ("Enable-ComputerRestore FAILED: " + $_.Exception.Message) }

    $k = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\SystemRestore'
    $hadFreq = $false
    $oldFreq = $null
    try { $oldFreq = (Get-ItemProperty -Path $k -Name SystemRestorePointCreationFrequency -ErrorAction Stop).SystemRestorePointCreationFrequency; $hadFreq = $true } catch { }
    try { Set-ItemProperty -Path $k -Name SystemRestorePointCreationFrequency -Value 0 -Type DWord -ErrorAction Stop; W "throttle temporarily set to 0" }
    catch { W ("throttle set FAILED: " + $_.Exception.Message) }

    $desc = 'AetherCore baseline - before any install'
    try { Checkpoint-Computer -Description $desc -RestorePointType 'MODIFY_SETTINGS' -ErrorAction Stop; W "Checkpoint-Computer -> OK" }
    catch { W ("Checkpoint-Computer FAILED: " + $_.Exception.Message) }

    try {
        if ($hadFreq) { Set-ItemProperty -Path $k -Name SystemRestorePointCreationFrequency -Value $oldFreq -Type DWord -ErrorAction Stop; W ("throttle restored to " + $oldFreq) }
        else { Remove-ItemProperty -Path $k -Name SystemRestorePointCreationFrequency -ErrorAction Stop; W "throttle value removed (back to unset default)" }
    } catch { W ("throttle restore FAILED: " + $_.Exception.Message) }

    # Re-enumerate. Creation returning OK is not proof; the listing is.
    $rp = @()
    try { $rp = @(Get-ComputerRestorePoint -ErrorAction Stop) } catch { W ("re-enumerate FAILED: " + $_.Exception.Message) }
    $usable = @($rp | Where-Object { (RpTime $_) -ge $cutoff })
    W ("AFTER_CREATE_RESTORE_POINTS=" + $rp.Count + " USABLE=" + $usable.Count)
    foreach ($r in $usable) { W ("  seq=" + $r.SequenceNumber + " created=" + $r.CreationTime + " desc=" + $r.Description) }
}

if ($usable.Count -eq 0) {
    W "PREFLIGHT=FAIL no verified restore point exists. REFUSING to install."
    W "STAGE2_ABORTED_NO_RESTORE_POINT"
    W "STAGE2_DONE"
    exit 1
}
$chosen = $usable | Sort-Object { RpTime $_ } -Descending | Select-Object -First 1
W ("PREFLIGHT=PASS recovery point verified: seq=" + $chosen.SequenceNumber + " created=" + $chosen.CreationTime + " desc=" + $chosen.Description)

# ---------------------------------------------------------------------------
# [1] install
# ---------------------------------------------------------------------------
W "=== [1] msiexec /i /qn /l*v ==="
if (-not (Test-Path $Msi)) { W ("FATAL: MSI not found: " + $Msi); W "STAGE2_DONE"; exit 1 }
W ("MSI=" + $Msi)
W ("MSI_SHA256=" + (Get-FileHash $Msi -Algorithm SHA256).Hash.ToLower())
$args = @('/i', ('"' + $Msi + '"'), '/qn', '/l*v', ('"' + $MSILOG + '"'))
$p = Start-Process -FilePath (Join-Path $env:SystemRoot 'System32\msiexec.exe') -ArgumentList $args -PassThru
$null = $p.Handle
$p.WaitForExit()
W ("MSIEXEC_EXIT=" + $p.ExitCode)

# ---------------------------------------------------------------------------
# [2] installed files
# ---------------------------------------------------------------------------
W "=== [2] installed files ==="
$files = @()
if (Test-Path $IF) {
    foreach ($f in (Get-ChildItem $IF -File -Recurse)) {
        $files += New-Object psobject -Property @{ name = $f.Name; size = $f.Length; sha256 = (Get-FileHash $f.FullName -Algorithm SHA256).Hash.ToLower() }
    }
}
W ("INSTALL_FILE_COUNT=" + $files.Count)
foreach ($f in ($files | Sort-Object name)) { W ("  " + $f.name.PadRight(45) + " " + ([string]$f.size).PadLeft(14) + "  " + $f.sha256) }
W ("VCOMP140_PRESENT=" + (Test-Path (Join-Path $IF 'vcomp140.dll')))
W ("LIBOMP_AARCH64_PRESENT=" + (Test-Path (Join-Path $IF 'libomp140.aarch64.dll')))
$devPattern = '(?i)(^|[\\/_-])(p\d+_|probe|attack|fuzz|bench|example|_test|-test)'
$dev = @(Get-ChildItem $IF -Recurse -ErrorAction SilentlyContinue | Where-Object { $_.Name -match $devPattern })
if ($dev.Count -gt 0) { W ("DEV_BINARY_IN_INSTALL_IMAGE=" + (($dev | ForEach-Object { $_.Name }) -join ',')) } else { W "DEV_BINARY_IN_INSTALL_IMAGE=NO" }

# ---------------------------------------------------------------------------
# [3] service
# ---------------------------------------------------------------------------
W "=== [3] service ==="
$sc = Join-Path $env:SystemRoot 'System32\sc.exe'
W ("--- sc query ---`n"     + ((& $sc query      AetherCoreMaintenance 2>&1 | Out-String).Trim()))
W ("--- sc qc ---`n"        + ((& $sc qc         AetherCoreMaintenance 2>&1 | Out-String).Trim()))
W ("--- sc qsidtype ---`n"  + ((& $sc qsidtype   AetherCoreMaintenance 2>&1 | Out-String).Trim()))
W ("--- sc showsid ---`n"   + ((& $sc showsid    AetherCoreMaintenance 2>&1 | Out-String).Trim()))
W ("--- sc sdshow ---`n"    + ((& $sc sdshow     AetherCoreMaintenance 2>&1 | Out-String).Trim()))

# ---------------------------------------------------------------------------
# [4] pipe DACL - NamedPipeClientStream is the method that works (Get-Acl and
#     FileStream both fail on a pipe). See SESSION_CONTEXT section 10 correction.
# ---------------------------------------------------------------------------
W "=== [4] pipe DACL ==="
$sddl = 'ERROR: not read'
try {
    $pc = New-Object System.IO.Pipes.NamedPipeClientStream('.', 'AetherCore.Maintenance.v7', [IO.Pipes.PipeDirection]::In)
    $pc.Connect(5000)
    $sddl = $pc.GetAccessControl().GetSecurityDescriptorSddlForm('All')
    $pc.Dispose()
} catch { $sddl = 'ERROR: ' + $_.Exception.Message }
W ("PIPE_SDDL=" + $sddl)
W ("PIPE_PRESENT=" + [bool](Get-ChildItem '\\.\pipe\' -ErrorAction SilentlyContinue | Where-Object { $_.Name -like 'AetherCore.Maintenance*' }))

# ---------------------------------------------------------------------------
# [5] install-dir ACLs
# ---------------------------------------------------------------------------
W "=== [5] install-dir ACLs ==="
W ((& (Join-Path $env:SystemRoot 'System32\icacls.exe') $IF 2>&1 | Out-String).Trim())

# ---------------------------------------------------------------------------
# [6] registration
# ---------------------------------------------------------------------------
W "=== [6] registration ==="
$arp = @(Get-ChildItem 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall' -ErrorAction SilentlyContinue |
    ForEach-Object { Get-ItemProperty $_.PSPath -ErrorAction SilentlyContinue } |
    Where-Object { $_.DisplayName -like '*AetherCore*' })
foreach ($a in $arp) { W ("ARP: " + $a.PSChildName + " | " + $a.DisplayName + " | " + $a.DisplayVersion) }
W ("HKLM_InstallVersion=" + (Get-ItemProperty 'HKLM:\SOFTWARE\AetherCore' -Name InstallVersion -ErrorAction SilentlyContinue).InstallVersion)

# ---------------------------------------------------------------------------
# [7] verbs, against the INSTALLED aetherctl
# ---------------------------------------------------------------------------
W "=== [7] verbs ==="
$ctl  = Join-Path $IF 'aetherctl.exe'
$vdir = Join-Path $OutDir 'verbs'
New-Item -ItemType Directory -Force $vdir | Out-Null
function Run-Verb([string]$label, [string[]]$argv, [int]$timeoutMs) {
    if (-not $timeoutMs) { $timeoutMs = 30000 }
    W ("--- VERB " + $label + " : aetherctl " + ($argv -join ' ') + " ---")
    $so = Join-Path $vdir ($label + '.out')
    $se = Join-Path $vdir ($label + '.err')
    try {
        $pp = Start-Process -FilePath $ctl -ArgumentList $argv -PassThru -NoNewWindow -RedirectStandardOutput $so -RedirectStandardError $se
        # Cache the handle BEFORE exit or ExitCode reads back empty on PS 5.1.
        # See SESSION_CONTEXT section 16.4 - this is a measured harness gap.
        $null = $pp.Handle
        $done = $pp.WaitForExit($timeoutMs)
        if ($done) { W 'RESULT=RETURNED'; W ('EXIT_CODE=' + $pp.ExitCode) }
        else { try { $pp.Kill() } catch { }; W ('RESULT=TIMED_OUT_' + $timeoutMs + 'MS') }
    } catch { W ('RESULT=LAUNCH_FAILED: ' + $_.Exception.Message) }
    $o = Get-Content $so -Raw -ErrorAction SilentlyContinue
    $e = Get-Content $se -Raw -ErrorAction SilentlyContinue
    if ($o) { W ('STDOUT: ' + $o.Trim()) } else { W 'STDOUT: (empty)' }
    if ($e) { W ('STDERR: ' + $e.Trim()) }
}
if (-not (Test-Path $ctl)) { W ('FATAL: installed aetherctl not found at ' + $ctl) }
else {
    W ('CTL_SHA256=' + (Get-FileHash $ctl -Algorithm SHA256).Hash.ToLower())
    Run-Verb 'servicedetect'  @('--output','json','service','detect')      30000
    Run-Verb 'doctor'         @('--output','json','doctor')                60000
    Run-Verb 'optimizestatus' @('--output','json','optimize','status')     30000
    Run-Verb 'scanstatus'     @('--output','json','scan','status')         30000
    Run-Verb 'insightslist'   @('--output','json','insights','list')      180000
    Run-Verb 'selfcheck'      @('--output','json','self-check')            60000
}

# ---------------------------------------------------------------------------
# [8] engineLabel - the single most important line of Stage 2.
#     localModel = the embedded reasoner is live. ruleFallback = it is not.
# ---------------------------------------------------------------------------
W "=== [8] engineLabel ==="
$ins = Get-Content (Join-Path $vdir 'insightslist.out') -Raw -ErrorAction SilentlyContinue
if ($ins -and $ins -match '"engineLabel"\s*:\s*"([^"]+)"') { W ('ENGINE_LABEL=' + $Matches[1]) }
else { W 'ENGINE_LABEL=NOT_FOUND_IN_OUTPUT' }

W 'STAGE2_DONE'
