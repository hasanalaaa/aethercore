#Requires -Version 5.1
<#
.SYNOPSIS
    Gate 4 -- install a driver, verify it, roll it back, verify the rollback.

.DESCRIPTION
    Every phase writes its EXPECTED value before it runs and compares against it
    after. A phase whose observation differs from EXPECTED stops the run; it does
    not redefine the criterion and does not continue to the next phase.

    Verification is by enumeration throughout. An exit code of 0 from
    Checkpoint-Computer, pnputil or DISM is never accepted as proof that the
    thing happened -- the object is looked up afterwards and its absence is a
    FAIL regardless of what the exit code said. This project has been bitten
    four times by a measuring instrument that lied, most recently by a Windows
    Update search that threw an exception and reported "0 drivers offered".

.PARAMETER InstanceId
    The device to mutate. Must be in an allowed class (see -AllowedClasses).

.PARAMETER DriverPath
    The .inf of the driver package to install over the current one.

.PARAMETER DryRun
    Run every read-only phase and every precondition, print what each mutating
    phase WOULD do, and mutate nothing. This is the default.

.PARAMETER Execute
    Actually mutate. Requires -Execute explicitly; -DryRun is the default so
    that a mistyped invocation cannot install a driver.

.EXAMPLE
    .\gate4-driver-runbook.ps1 -InstanceId 'ACPI\INTC1070\2&DABA3FF&0' -DriverPath C:\stage\hideventfilter.inf
    Dry run. Nothing is mutated.

.EXAMPLE
    .\gate4-driver-runbook.ps1 -InstanceId '...' -DriverPath '...' -Execute
    The real gate.
#>
[CmdletBinding(DefaultParameterSetName = 'DryRun')]
param(
    [Parameter(Mandatory = $true)][string]$InstanceId,
    [Parameter(Mandatory = $true)][string]$DriverPath,
    [Parameter(ParameterSetName = 'DryRun')][switch]$DryRun,
    [Parameter(ParameterSetName = 'Execute')][switch]$Execute,
    # Rehearsal only. Lets a DRY RUN walk past an owner-gated precondition so the
    # owner can read what phases 1-5 would do. It is rejected outright when
    # combined with -Execute, so it can never weaken the real gate: the checks
    # still run and still report FAIL, they just do not halt the rehearsal.
    [Parameter(ParameterSetName = 'DryRun')][switch]$RehearseBeyondOwnerGate,
    [string]$EvidenceDir = (Join-Path $PSScriptRoot '..\out\gate4'),
    [string[]]$AllowedClasses = @('Printer', 'PrintQueue', 'HIDClass', 'USB', 'Image', 'Camera', 'Mouse', 'Keyboard'),
    [string[]]$ForbiddenClasses = @('Display', 'Net', 'SCSIAdapter', 'HDC', 'DiskDrive', 'Volume', 'System', 'Firmware', 'Computer', 'USBDevice')
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (-not $Execute) { $DryRun = $true }

# --------------------------------------------------------------------------
# Evidence and result plumbing
# --------------------------------------------------------------------------

$script:Failures = 0
$script:Stamp = (Get-Date).ToString('yyyyMMdd-HHmmss')
New-Item -ItemType Directory -Force -Path $EvidenceDir | Out-Null
$script:Transcript = Join-Path $EvidenceDir "gate4-$script:Stamp.log"

function Write-Phase {
    param([string]$Number, [string]$Title)
    $line = "`n=== PHASE $Number -- $Title ==="
    Write-Host $line -ForegroundColor Cyan
    Add-Content -Path $script:Transcript -Value $line -Encoding utf8
}

function Write-Record {
    param([string]$Text)
    Write-Host $Text
    Add-Content -Path $script:Transcript -Value $Text -Encoding utf8
}

# The single place a criterion is judged. Every check routes through here so
# that no phase can quietly downgrade its own EXPECTED value.
function Assert-Expected {
    param(
        [Parameter(Mandatory = $true)][string]$Check,
        [Parameter(Mandatory = $true)][string]$Expected,
        [Parameter(Mandatory = $true)][string]$Observed,
        [switch]$Fatal
    )
    $ok = ($Expected -eq $Observed)
    $verdict = if ($ok) { 'PASS' } else { 'FAIL' }
    Write-Record ("  [{0}] {1}`n         EXPECTED: {2}`n         OBSERVED: {3}" -f $verdict, $Check, $Expected, $Observed)
    if (-not $ok) {
        $script:Failures++
        if ($Fatal) { throw "STOP: '$Check' failed. Expected '$Expected', observed '$Observed'. Nothing further runs." }
    }
}

function Get-DeviceFacts {
    param([string]$Id)
    $d = Get-PnpDevice -InstanceId $Id -ErrorAction SilentlyContinue
    if (-not $d) { return $null }
    $props = Get-PnpDeviceProperty -InstanceId $Id -ErrorAction SilentlyContinue
    function Prop([string]$k) { ($props | Where-Object KeyName -eq $k).Data }
    [pscustomobject]@{
        InstanceId    = $Id
        FriendlyName  = $d.FriendlyName
        Class         = $d.Class
        Status        = $d.Status
        Problem       = $d.Problem
        DriverVersion = Prop 'DEVPKEY_Device_DriverVersion'
        DriverDate    = Prop 'DEVPKEY_Device_DriverDate'
        DriverProvider= Prop 'DEVPKEY_Device_DriverProvider'
        InfPath       = Prop 'DEVPKEY_Device_DriverInfPath'
    }
}

# --------------------------------------------------------------------------
# PHASE 0 -- preconditions. Nothing mutates until every one of these holds.
# --------------------------------------------------------------------------

Write-Phase '0' 'PRECONDITIONS (read-only)'
Write-Record "mode: $(if ($DryRun) { 'DRY RUN -- nothing will be mutated' } else { 'EXECUTE' })"
Write-Record "evidence: $script:Transcript"

# 0a -- elevation
$elevated = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(544)
Assert-Expected -Check '0a elevation' -Expected 'True' -Observed "$elevated" -Fatal

# 0b -- System Restore enabled on C:. Without it there is no rollback for a
#      driver that takes the machine down, and this box has no snapshots.
$srDisabled = $true
try {
    $rp = Get-ComputerRestorePoint -ErrorAction Stop
    $srDisabled = $false
} catch { $srDisabled = $true }
Assert-Expected -Check '0b System Restore queryable' -Expected 'True' -Observed "$(-not $srDisabled)" -Fatal

# 0c -- a system image exists AND is listable. Exit code is not the evidence.
$imageTarget = $null
foreach ($v in (Get-Volume | Where-Object { $_.DriveLetter })) {
    if (Test-Path ("{0}:\WindowsImageBackup" -f $v.DriveLetter)) { $imageTarget = "$($v.DriveLetter):" ; break }
}
$versions = if ($imageTarget) { & wbadmin get versions -backupTarget:$imageTarget 2>&1 | Out-String } else { '' }
$hasVersion = $versions -match 'Version identifier:'
Assert-Expected -Check '0c system image listable' -Expected 'True' -Observed "$hasVersion" -Fatal
if ($hasVersion) {
    ($versions -split "`r?`n" | Where-Object { $_ -match 'Backup time|Version identifier|Can recover' }) |
        ForEach-Object { Write-Record "         $($_.Trim())" }
}

# 0d -- recovery media boot-tested. THIS IS OWNER-GATED and cannot be measured
#      from inside a running Windows. The marker file is written by the owner
#      after booting the media once and confirming the recovery environment
#      sees the system disk and the image. Creating media is not testing it.
$bootTestMarker = Join-Path $PSScriptRoot '..\docs\phase55\RECOVERY-MEDIA-BOOT-TEST.md'
$bootTested = Test-Path $bootTestMarker
Assert-Expected -Check '0d recovery media boot-tested (owner)' -Expected 'True' -Observed "$bootTested" `
    -Fatal:(-not ($DryRun -and $RehearseBeyondOwnerGate))
if (-not $bootTested -and $DryRun -and $RehearseBeyondOwnerGate) {
    Write-Record "         REHEARSAL: continuing past a FAILED owner gate because -RehearseBeyondOwnerGate"
    Write-Record "         was given on a dry run. The real gate stops here until the owner boot-tests"
    Write-Record "         the media and writes $bootTestMarker."
}

# 0e -- the device exists, and is in an allowed class
$before = Get-DeviceFacts -Id $InstanceId
Assert-Expected -Check '0e device present' -Expected 'True' -Observed "$($null -ne $before)" -Fatal
Assert-Expected -Check '0e class not forbidden' -Expected 'True' -Observed "$($ForbiddenClasses -notcontains $before.Class)" -Fatal
Assert-Expected -Check '0e class allowed' -Expected 'True' -Observed "$($AllowedClasses -contains $before.Class)" -Fatal

# 0f -- the driver package to install exists, and is signed
Assert-Expected -Check '0f driver package exists' -Expected 'True' -Observed "$(Test-Path $DriverPath)" -Fatal
$catSigned = 'unknown'
if (Test-Path $DriverPath) {
    $sig = Get-AuthenticodeSignature $DriverPath
    $catSigned = $sig.Status
    Write-Record "         inf signature status: $catSigned (an .inf is usually signed by its .cat; see phase 2)"
    $sha = (Get-FileHash $DriverPath -Algorithm SHA256).Hash.ToLowerInvariant()
    Write-Record "         inf sha256: $sha"
}

# 0g -- the package is actually a different version from what is installed
$newVersion = 'unknown'
try {
    $infText = Get-Content $DriverPath -Raw -ErrorAction Stop
    if ($infText -match 'DriverVer\s*=\s*[^,]+,\s*([0-9.]+)') { $newVersion = $Matches[1] }
} catch {}
Write-Record "         installed version: $($before.DriverVersion)"
Write-Record "         package version:   $newVersion"
Assert-Expected -Check '0g package differs from installed' -Expected 'True' `
    -Observed "$($newVersion -ne 'unknown' -and $newVersion -ne $before.DriverVersion)" -Fatal

if ($script:Failures -gt 0 -and -not ($DryRun -and $RehearseBeyondOwnerGate)) {
    throw "STOP: $script:Failures precondition(s) failed."
}

# --------------------------------------------------------------------------
# PHASE 1 -- pre-mutation record + restore point
# --------------------------------------------------------------------------

Write-Phase '1' 'PRE-MUTATION RECORD AND RESTORE POINT'

$beforeFile = Join-Path $EvidenceDir "before-$script:Stamp.json"
$before | ConvertTo-Json -Depth 4 | Set-Content -Path $beforeFile -Encoding utf8
Write-Record "  pre-mutation state -> $beforeFile"
$before | Format-List | Out-String | ForEach-Object { Write-Record $_ }

$rpName = "AetherCore Gate 4 -- before $($before.FriendlyName) $script:Stamp"
if ($DryRun) {
    Write-Record "  DRY RUN: would create restore point '$rpName' and verify it by enumeration"
} else {
    $rpBefore = @(Get-ComputerRestorePoint).Count
    Checkpoint-Computer -Description $rpName -RestorePointType 'APPLICATION_INSTALL'
    # Verification by enumeration. Checkpoint-Computer returning without error
    # is NOT proof; Get-ComputerRestorePoint listing the new point is.
    $rpAfter = @(Get-ComputerRestorePoint)
    $mine = $rpAfter | Where-Object { $_.Description -eq $rpName }
    Assert-Expected -Check '1 restore point listed by enumeration' -Expected 'True' -Observed "$($null -ne $mine)" -Fatal
    Write-Record "         restore points: $rpBefore -> $($rpAfter.Count); seq=$($mine.SequenceNumber)"
}

# --------------------------------------------------------------------------
# PHASE 2 -- install
# --------------------------------------------------------------------------

Write-Phase '2' 'INSTALL THE DRIVER'

if ($DryRun) {
    Write-Record "  DRY RUN: would run: pnputil /add-driver `"$DriverPath`" /install"
    Write-Record "  DRY RUN: EXPECTED afterwards: device Status=OK, DriverVersion=$newVersion"
} else {
    $out = & pnputil /add-driver $DriverPath /install 2>&1 | Out-String
    Write-Record $out
    Write-Record "  pnputil exit: $LASTEXITCODE  (recorded, NOT the pass criterion)"
}

# --------------------------------------------------------------------------
# PHASE 3 -- verify the install, by enumeration
# --------------------------------------------------------------------------

Write-Phase '3' 'VERIFY THE INSTALL'

if ($DryRun) {
    Write-Record "  DRY RUN: would re-read the device and compare:"
    Write-Record "           EXPECTED DriverVersion = $newVersion"
    Write-Record "           EXPECTED Status        = OK"
    Write-Record "           EXPECTED Problem       = CM_PROB_NONE"
} else {
    Start-Sleep -Seconds 5
    $after = Get-DeviceFacts -Id $InstanceId
    $afterFile = Join-Path $EvidenceDir "after-install-$script:Stamp.json"
    $after | ConvertTo-Json -Depth 4 | Set-Content -Path $afterFile -Encoding utf8
    Assert-Expected -Check '3 device still present' -Expected 'True' -Observed "$($null -ne $after)" -Fatal
    Assert-Expected -Check '3 device healthy'  -Expected 'OK'        -Observed "$($after.Status)"
    Assert-Expected -Check '3 version changed' -Expected $newVersion -Observed "$($after.DriverVersion)"
    Write-Record "  post-install state -> $afterFile"
}

# --------------------------------------------------------------------------
# PHASE 4 -- roll back
# --------------------------------------------------------------------------

Write-Phase '4' 'ROLL BACK'

if ($DryRun) {
    Write-Record "  DRY RUN: would roll back to $($before.DriverVersion) via the device's"
    Write-Record "           previous driver, then delete the staged package with"
    Write-Record "           pnputil /delete-driver <oemNN.inf> /uninstall"
    Write-Record "  DRY RUN: EXPECTED afterwards: DriverVersion=$($before.DriverVersion), Status=OK"
} else {
    # Roll back to the previous driver. Device Manager's Roll Back Driver is the
    # supported path; pnputil /delete-driver /uninstall on the newly staged
    # package makes Windows fall back to the previously staged one.
    $staged = (& pnputil /enum-drivers 2>&1 | Out-String)
    Write-Record "  (staged packages captured to evidence for the rollback target)"
    Set-Content -Path (Join-Path $EvidenceDir "staged-$script:Stamp.txt") -Value $staged -Encoding utf8

    $afterNow = Get-DeviceFacts -Id $InstanceId
    $out = & pnputil /delete-driver $afterNow.InfPath /uninstall /force 2>&1 | Out-String
    Write-Record $out
    Write-Record "  pnputil exit: $LASTEXITCODE  (recorded, NOT the pass criterion)"
}

# --------------------------------------------------------------------------
# PHASE 5 -- verify the rollback, by enumeration
# --------------------------------------------------------------------------

Write-Phase '5' 'VERIFY THE ROLLBACK'

if ($DryRun) {
    Write-Record "  DRY RUN: would compare the device against the phase-1 record field by field."
    Write-Record "           EXPECTED: DriverVersion, DriverProvider and Status all equal to"
    Write-Record "           the pre-mutation values recorded in phase 1."
} else {
    Start-Sleep -Seconds 5
    $rolled = Get-DeviceFacts -Id $InstanceId
    $rolledFile = Join-Path $EvidenceDir "after-rollback-$script:Stamp.json"
    $rolled | ConvertTo-Json -Depth 4 | Set-Content -Path $rolledFile -Encoding utf8
    Assert-Expected -Check '5 device present after rollback' -Expected 'True' -Observed "$($null -ne $rolled)" -Fatal
    Assert-Expected -Check '5 version restored'  -Expected "$($before.DriverVersion)"  -Observed "$($rolled.DriverVersion)"
    Assert-Expected -Check '5 provider restored' -Expected "$($before.DriverProvider)" -Observed "$($rolled.DriverProvider)"
    Assert-Expected -Check '5 device healthy'    -Expected 'OK'                        -Observed "$($rolled.Status)"
    Write-Record "  post-rollback state -> $rolledFile"
}

# --------------------------------------------------------------------------
# VERDICT
# --------------------------------------------------------------------------

Write-Phase 'X' 'VERDICT'
if ($DryRun) {
    Write-Record "  DRY RUN COMPLETE. Preconditions were evaluated for real; no mutation occurred."
    Write-Record "  Failures in the read-only phases: $script:Failures"
} else {
    Write-Record "  Failures: $script:Failures"
    Write-Record "  $(if ($script:Failures -eq 0) { 'GATE 4: PASS' } else { 'GATE 4: FAIL' })"
}
Write-Record "  evidence: $script:Transcript"
exit $(if ($script:Failures -eq 0) { 0 } else { 1 })
