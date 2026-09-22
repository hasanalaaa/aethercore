#Requires -Version 5.1
<#
.SYNOPSIS
    Gate 4 -- verify the six preconditions mechanically and state which hold.

.DESCRIPTION
    Read-only. Mutates nothing, installs nothing, and is safe to run at any time.

    Each condition is measured, never assumed, and each reports the evidence it
    measured rather than a bare verdict. Conditions that cannot be measured from
    inside a running Windows say so explicitly instead of guessing -- a check
    that cannot see its subject must report UNKNOWN, not PASS.

    Exit code is 0 only when every condition holds.
#>
[CmdletBinding()]
param(
    [string]$CandidateFile = (Join-Path $PSScriptRoot '..\docs\phase55\GATE4-CHOSEN-DEVICE.txt'),
    [string]$BootTestFile  = (Join-Path $PSScriptRoot '..\docs\phase55\RECOVERY-MEDIA-BOOT-TEST.md')
)

$ErrorActionPreference = 'Continue'
$results = New-Object System.Collections.ArrayList

function Add-Result {
    param([string]$Condition, [string]$State, [string]$Evidence)
    [void]$results.Add([pscustomobject]@{ Condition = $Condition; State = $State; Evidence = $Evidence })
}

# Two of the six conditions read instruments that require elevation:
# Get-ComputerRestorePoint and wbadmin. Run unelevated they do not report an
# absent subject, they refuse to look -- and condition 6 already states the rule
# this file follows: an instrument that cannot see its subject reports UNKNOWN,
# never a verdict about the subject. P71 measured both reporting FAILS from an
# unelevated session on a machine that has a restore point and an image, which
# is a false accusation of exactly the kind the phase 26/27 audits carried.
# UNKNOWN still blocks Gate 4 -- `State -ne 'HOLDS'` -- so nothing is loosened.
$IsElevated = ([Security.Principal.WindowsPrincipal] `
    [Security.Principal.WindowsIdentity]::GetCurrent()
    ).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $IsElevated) {
    Write-Host ("NOTE: not elevated. Conditions that need elevation report " +
                "UNKNOWN rather than FAILS.") -ForegroundColor DarkYellow
}

# --- 1. recovery point -----------------------------------------------------
try {
    $rps = @(Get-ComputerRestorePoint -ErrorAction Stop)
    if ($rps.Count -gt 0) {
        $newest = $rps | Sort-Object CreationTime -Descending | Select-Object -First 1
        Add-Result 'recovery point' 'HOLDS' ("$($rps.Count) point(s); newest seq=$($newest.SequenceNumber) '$($newest.Description)'")
    } else {
        # System Restore answering with an empty list is the empty-state trap:
        # queryable but holding nothing is NOT a recovery point.
        Add-Result 'recovery point' 'FAILS' 'System Restore is queryable but holds zero restore points'
    }
} catch {
    # A throw is the instrument failing, never evidence the subject is absent.
    # The empty-list branch above stays FAILS because a queryable System Restore
    # holding zero points IS evidence; this is not.
    $why = if ($IsElevated) { '' } else { ' (session is not elevated; this cmdlet requires it)' }
    Add-Result 'recovery point' 'UNKNOWN' `
        "Get-ComputerRestorePoint threw: $($_.Exception.Message)$why -- not measured, not claimed"
}

# --- 2. disk image ---------------------------------------------------------
$imageTarget = $null
foreach ($v in (Get-Volume | Where-Object { $_.DriveLetter } | Sort-Object DriveLetter)) {
    if (Test-Path ("{0}:\WindowsImageBackup" -f $v.DriveLetter)) { $imageTarget = "$($v.DriveLetter):"; break }
}
if (-not $imageTarget) {
    Add-Result 'disk image' 'FAILS' 'no volume carries a WindowsImageBackup directory'
} else {
    $txt = & wbadmin get versions -backupTarget:$imageTarget 2>&1 | Out-String
    $ids = [regex]::Matches($txt, 'Version identifier:\s*(\S+)') | ForEach-Object { $_.Groups[1].Value }
    if ($ids.Count -eq 0 -and -not $IsElevated) {
        # wbadmin requires elevation. Unelevated it prints an access error and
        # parses to zero versions, which is indistinguishable from "no image"
        # to the regex and is not the same thing at all. P71 measured this
        # against a D:\WindowsImageBackup dated 2026-09-02 whose ACL denies an
        # unelevated read outright.
        Add-Result 'disk image' 'UNKNOWN' (
            "WindowsImageBackup exists on $imageTarget but wbadmin needs elevation; " +
            "version count is UNKNOWN, not zero -- re-run elevated")
    } elseif ($ids.Count -eq 0) {
        # P41 recorded wbadmin returning 0 having done nothing. An empty listing
        # is a FAIL regardless of the exit code.
        Add-Result 'disk image' 'FAILS' "WindowsImageBackup exists on $imageTarget but wbadmin lists zero versions"
    } else {
        $age = $null
        if ($txt -match 'Backup time:\s*(.+?)\s*$') {
            try { $age = [int]((Get-Date) - [datetime]::Parse($Matches[1])).TotalDays } catch {}
        }
        $ageTxt = if ($null -ne $age) { ", $age day(s) old" } else { '' }
        Add-Result 'disk image' 'HOLDS' ("$imageTarget version(s): $($ids -join ', ')$ageTxt")
    }
}

# --- 3. recovery media present ---------------------------------------------
# A Windows recovery drive is a removable volume carrying \sources\boot.wim
# plus a boot manager. Look for that, not for a drive letter.
$media = @()
foreach ($v in (Get-Volume | Where-Object { $_.DriveLetter -and $_.DriveType -eq 'Removable' })) {
    $root = "$($v.DriveLetter):"
    if ((Test-Path "$root\sources\boot.wim") -or (Test-Path "$root\bootmgr")) {
        $media += "$root ($($v.FileSystemLabel))"
    }
}
if ($media.Count -gt 0) {
    Add-Result 'recovery media present' 'HOLDS' ($media -join '; ')
} else {
    $removable = @(Get-Volume | Where-Object { $_.DriveType -eq 'Removable' })
    Add-Result 'recovery media present' 'FAILS' (
        "no removable volume carries \sources\boot.wim or \bootmgr " +
        "($($removable.Count) removable volume(s) attached)")
}

# --- 4. recovery media boot-tested -----------------------------------------
# Cannot be measured from inside a running Windows: booting the media means not
# running this. The owner records the result in $BootTestFile after booting it
# once. Creating media is not testing it.
if (Test-Path $BootTestFile) {
    Add-Result 'recovery media boot-tested' 'HOLDS' "owner record: $BootTestFile"
} else {
    Add-Result 'recovery media boot-tested' 'FAILS' `
        "no owner record at $BootTestFile -- unmeasurable from a running Windows; owner action"
}

# --- 5. candidate device chosen --------------------------------------------
if (Test-Path $CandidateFile) {
    $chosen = (Get-Content $CandidateFile -Raw).Trim()
    if ($chosen) {
        $dev = Get-PnpDevice -InstanceId $chosen -ErrorAction SilentlyContinue
        if ($dev) {
            Add-Result 'candidate device chosen' 'HOLDS' "$chosen -> [$($dev.Class)] $($dev.FriendlyName)"
        } else {
            Add-Result 'candidate device chosen' 'FAILS' "$CandidateFile names '$chosen', which is not an attached device"
        }
    } else {
        Add-Result 'candidate device chosen' 'FAILS' "$CandidateFile is empty"
    }
} else {
    Add-Result 'candidate device chosen' 'FAILS' `
        "no $CandidateFile -- owner picks a row from docs/phase55/GATE4-CANDIDATES.md"
}

# --- 6. driver obtainable --------------------------------------------------
# Two sources: a second version already staged in the driver store, or one
# offered by Windows Update. Both are measured; neither is assumed.
$wuCount = $null
$wuNote  = ''
try {
    $searcher = (New-Object -ComObject Microsoft.Update.Session).CreateUpdateSearcher()
    $found = $searcher.Search("IsInstalled=0 and Type='Driver'")
    $wuCount = $found.Updates.Count
} catch {
    # A thrown search that leaves $found null renders as "0 offered" if the
    # caller is careless. Say UNKNOWN instead -- this is the instrument lying.
    $wuNote = "Windows Update search FAILED (0x{0:X8}); count is UNKNOWN, not zero" -f $_.Exception.HResult
}

$stagedNote = 'driver store not enumerated'
if (Test-Path $CandidateFile) {
    $chosen = (Get-Content $CandidateFile -Raw).Trim()
    $inf = (Get-PnpDeviceProperty -InstanceId $chosen -KeyName 'DEVPKEY_Device_DriverInfPath' -ErrorAction SilentlyContinue).Data
    if ($inf) {
        $enum = & pnputil /enum-drivers 2>&1 | Out-String
        $orig = $null
        foreach ($block in ($enum -split "`r?`n`r?`n")) {
            if ($block -match [regex]::Escape($inf) -and $block -match 'Original Name:\s*(\S+)') { $orig = $Matches[1] }
        }
        if ($orig) {
            $count = ([regex]::Matches($enum, 'Original Name:\s*' + [regex]::Escape($orig))).Count
            $stagedNote = "$orig has $count version(s) staged"
        }
    }
}

if ($wuNote) {
    Add-Result 'driver obtainable' 'UNKNOWN' "$wuNote; $stagedNote"
} elseif ($wuCount -gt 0) {
    Add-Result 'driver obtainable' 'HOLDS' "Windows Update offers $wuCount driver update(s); $stagedNote"
} else {
    Add-Result 'driver obtainable' 'FAILS' `
        "Windows Update offers 0 drivers (search succeeded); $stagedNote -- a package must be supplied out of band"
}

# --- report ----------------------------------------------------------------
Write-Host "`nGate 4 preconditions -- $(Get-Date -Format 'u')`n" -ForegroundColor Cyan
$results | Format-Table -AutoSize -Wrap | Out-String -Width 200 | Write-Host

$held = @($results | Where-Object State -eq 'HOLDS').Count
Write-Host ("{0} of {1} conditions hold." -f $held, $results.Count)
$blocking = @($results | Where-Object State -ne 'HOLDS')
if ($blocking.Count -gt 0) {
    Write-Host "`nGate 4 must NOT start. Blocking:" -ForegroundColor Yellow
    $blocking | ForEach-Object { Write-Host ("  - {0}: {1}" -f $_.Condition, $_.Evidence) }
}
exit $(if ($blocking.Count -eq 0) { 0 } else { 1 })
