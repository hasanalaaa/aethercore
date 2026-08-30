[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$MsiPath,
    [switch]$InstallLifecycle,
    [switch]$VerifyInstalledStateOnly,
    [switch]$AcknowledgeDisposableMachine,
    [switch]$RequireSignedArtifacts
)
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'Installer security verification requires Windows.' }
$Root = Split-Path $PSScriptRoot -Parent
$Msi = (Resolve-Path $MsiPath).Path
$ServiceName = 'AetherCoreMaintenance'
$ServiceSddl = 'D:(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWLOCRRC;;;AU)'
$InstallDir = Join-Path $env:ProgramW6432 'AetherCore'
$DataDir = Join-Path $env:ProgramData 'AetherCore'
$MutationLock = Join-Path $DataDir 'state\machine-mutation.lock'

function Find-Mt {
    $cmd = Get-Command mt.exe -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    $kits = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
    $found = Get-ChildItem $kits -Filter mt.exe -Recurse -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match '\\x64\\mt\.exe$' } |
        Sort-Object FullName -Descending | Select-Object -First 1
    if (-not $found) { throw 'mt.exe was not found; install Windows SDK tools.' }
    return $found.FullName
}
function Execution-Level([string]$Exe) {
    $mt = Find-Mt
    $tmp = Join-Path $env:TEMP ("aethercore-manifest-{0}.xml" -f [guid]::NewGuid())
    try {
        & $mt -nologo "-inputresource:$Exe;#1" "-out:$tmp"
        if ($LASTEXITCODE -ne 0) { throw "Unable to extract PE manifest: $Exe" }
        $text = Get-Content $tmp -Raw
        if ($text -notmatch 'requestedExecutionLevel\s+level=["'']([^"'']+)["'']') {
            throw "requestedExecutionLevel is missing from $Exe"
        }
        return $Matches[1]
    } finally { Remove-Item $tmp -Force -ErrorAction SilentlyContinue }
}
function Run-Msi([string[]]$Arguments,[string]$Label) {
    $p = Start-Process msiexec.exe -ArgumentList $Arguments -Wait -PassThru
    if ($p.ExitCode -notin @(0,3010)) { throw "$Label failed with Windows Installer exit code $($p.ExitCode)." }
}
function Sid-Of([System.Security.Principal.IdentityReference]$Identity) {
    try { return $Identity.Translate([System.Security.Principal.SecurityIdentifier]).Value } catch { return $Identity.Value }
}
function Assert-ProtectedAcl([string]$Path,[bool]$DataAcl) {
    $acl = Get-Acl $Path
    if (-not $acl.AreAccessRulesProtected) { throw "ACL inheritance is not protected: $Path" }
    $rules = @($acl.Access | ForEach-Object {
        [pscustomobject]@{ Sid=(Sid-Of $_.IdentityReference); Type=$_.AccessControlType.ToString(); Rights=$_.FileSystemRights }
    })
    foreach ($dangerSid in @('S-1-1-0','S-1-5-11')) {
        if ($rules | Where-Object { $_.Sid -eq $dangerSid -and $_.Type -eq 'Allow' }) {
            throw "Dangerous broad allow ACE $dangerSid remains on $Path"
        }
    }
    $users = @($rules | Where-Object { $_.Sid -eq 'S-1-5-32-545' -and $_.Type -eq 'Allow' })
    if ($DataAcl) {
        if ($users.Count -ne 0) { throw "Users must not have access to service state root: $Path" }
    } else {
        if ($users.Count -eq 0) { throw "Users read/execute ACE missing from install root: $Path" }
        $danger = [System.Security.AccessControl.FileSystemRights]::Write -bor
                  [System.Security.AccessControl.FileSystemRights]::Modify -bor
                  [System.Security.AccessControl.FileSystemRights]::FullControl -bor
                  [System.Security.AccessControl.FileSystemRights]::Delete -bor
                  [System.Security.AccessControl.FileSystemRights]::ChangePermissions -bor
                  [System.Security.AccessControl.FileSystemRights]::TakeOwnership
        foreach ($rule in $users) {
            if (($rule.Rights -band $danger) -ne 0) { throw "Users have write/modify rights on install root: $Path" }
        }
    }
}

function Assert-MutationLockAcl([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "Machine mutation authority file is missing: $Path" }
    $item = Get-Item -LiteralPath $Path -Force
    if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "Machine mutation authority must not be a reparse point: $Path"
    }
    $acl = Get-Acl -LiteralPath $Path
    if (-not $acl.AreAccessRulesProtected) { throw "Machine mutation authority ACL inheritance is not protected: $Path" }
    $expected = @(
        'S-1-5-18',
        'S-1-5-32-544',
        (New-Object System.Security.Principal.NTAccount('NT SERVICE','AetherCoreMaintenance')).Translate([System.Security.Principal.SecurityIdentifier]).Value
    )
    $allows = @($acl.Access | Where-Object { $_.AccessControlType -eq [System.Security.AccessControl.AccessControlType]::Allow } | ForEach-Object { Sid-Of $_.IdentityReference })
    foreach ($sid in $expected) {
        if ($sid -notin $allows) { throw "Required machine mutation authority principal is missing: $sid" }
    }
    $unexpected = @($allows | Where-Object { $_ -notin $expected } | Sort-Object -Unique)
    if ($unexpected.Count -ne 0) { throw "Unexpected machine mutation authority allow trustee(s): $($unexpected -join ', ')" }
}


function Assert-MutationLockContention([string]$Path) {
    $share = [System.IO.FileShare]::ReadWrite -bor [System.IO.FileShare]::Delete
    $first = [System.IO.File]::Open($Path,[System.IO.FileMode]::Open,[System.IO.FileAccess]::ReadWrite,$share)
    $second = $null
    $firstLocked = $false
    $secondLocked = $false
    try {
        $second = [System.IO.File]::Open($Path,[System.IO.FileMode]::Open,[System.IO.FileAccess]::ReadWrite,$share)
        $first.Lock(0,1)
        $firstLocked = $true
        $blocked = $false
        try {
            $second.Lock(0,1)
            $secondLocked = $true
        } catch [System.IO.IOException] {
            $blocked = $true
        }
        if (-not $blocked) { throw 'Machine mutation byte-range lock allowed overlapping exclusive ownership.' }

        $first.Unlock(0,1)
        $firstLocked = $false
        $second.Lock(0,1)
        $secondLocked = $true
        $second.Unlock(0,1)
        $secondLocked = $false
    } finally {
        if ($secondLocked -and $second) { try { $second.Unlock(0,1) } catch {} }
        if ($firstLocked) { try { $first.Unlock(0,1) } catch {} }
        if ($second) { $second.Dispose() }
        $first.Dispose()
    }
}

function Verify-InstalledState {
    $svc = Get-CimInstance Win32_Service -Filter "Name='$ServiceName'"
    if (-not $svc) { throw 'Maintenance service is not installed.' }
    if ($svc.StartName -notin @('LocalSystem','Local System')) { throw "Service account drift: $($svc.StartName)" }
    if ($svc.StartMode -ne 'Auto') { throw "Service is not configured automatic: $($svc.StartMode)" }
    $expectedExe = (Join-Path $InstallDir 'aethercore-maintenance-service.exe').ToLowerInvariant()
    $actualExe = ($svc.PathName.Trim('"')).ToLowerInvariant()
    if ($actualExe -ne $expectedExe) { throw "Service binary path drift: $($svc.PathName)" }
    $delayed = (Get-ItemProperty "HKLM:\SYSTEM\CurrentControlSet\Services\$ServiceName" -Name DelayedAutoStart -ErrorAction Stop).DelayedAutoStart
    if ($delayed -ne 1) { throw 'DelayedAutoStart is not enabled.' }

    $sidType = (& "$env:SystemRoot\System32\sc.exe" qsidtype $ServiceName | Out-String)
    if ($sidType -notmatch 'UNRESTRICTED') { throw "Service SID type is not UNRESTRICTED: $sidType" }
    $tokenEvidence = Join-Path $Root 'out\installer-test\maintenance-service-token.json'
    & (Join-Path $PSScriptRoot 'verify-maintenance-service-token.ps1') -ServiceName $ServiceName -OutputPath $tokenEvidence
    if (-not (Test-Path $tokenEvidence)) { throw 'Maintenance service token verifier produced no evidence artifact.' }
    $sddl = (& "$env:SystemRoot\System32\sc.exe" sdshow $ServiceName | Out-String)
    $compact = ($sddl -replace '\s','')
    if ($compact -notmatch [regex]::Escape($ServiceSddl)) { throw "Service DACL differs from the fixed policy: $sddl" }

    Assert-ProtectedAcl $InstallDir $false
    Assert-ProtectedAcl $DataDir $true
    Assert-MutationLockAcl $MutationLock
    Assert-MutationLockContention $MutationLock
    $desktop = Join-Path $InstallDir 'aethercore-desktop.exe'
    $broker = Join-Path $InstallDir 'aethercore-consent-broker.exe'
    $updateBroker = Join-Path $InstallDir 'aethercore-update-broker.exe'
    if ((Execution-Level $desktop) -ne 'asInvoker') { throw 'Desktop PE manifest is not asInvoker.' }
    if ((Execution-Level $broker) -ne 'requireAdministrator') { throw 'Consent broker PE manifest is not requireAdministrator.' }
    if ((Execution-Level $updateBroker) -ne 'requireAdministrator') { throw 'Update broker PE manifest is not requireAdministrator.' }

    if ($RequireSignedArtifacts) {
        foreach ($file in @($desktop,$broker,$updateBroker,(Join-Path $InstallDir 'aethercore-maintenance-service.exe'),(Join-Path $InstallDir 'aethercore-install-hardener.exe'))) {
            $sig = Get-AuthenticodeSignature $file
            if ($sig.Status -ne 'Valid') { throw "Installed artifact has invalid Authenticode signature: $file ($($sig.Status))" }
        }
    }
}

if ($RequireSignedArtifacts) {
    $msiSig = Get-AuthenticodeSignature $Msi
    if ($msiSig.Status -ne 'Valid') { throw "MSI Authenticode signature is invalid: $($msiSig.Status)" }
}
if ($VerifyInstalledStateOnly) {
    Verify-InstalledState
    Write-Host 'Installed AetherCore security/elevation state verified.' -ForegroundColor Green
    return
}
if (-not $InstallLifecycle) {
    $message = if ($RequireSignedArtifacts) { 'MSI path and Authenticode signature verified; lifecycle was not requested.' } else { 'MSI path resolved successfully; lifecycle was not requested.' }
    Write-Host $message -ForegroundColor Yellow
    return
}
if (-not $AcknowledgeDisposableMachine -and $env:AETHERCORE_INSTALLER_TEST_MACHINE -ne '1') {
    throw 'Lifecycle verification installs/repairs/uninstalls AetherCore. Use a disposable Windows VM and pass -AcknowledgeDisposableMachine (or set AETHERCORE_INSTALLER_TEST_MACHINE=1).'
}
if (Get-Service $ServiceName -ErrorAction SilentlyContinue) { throw 'Refusing lifecycle test: AetherCore service already exists.' }
if (Test-Path $InstallDir) { throw 'Refusing lifecycle test: AetherCore install directory already exists.' }

$logDir = Join-Path $Root 'out\installer-test'
New-Item -ItemType Directory -Force $logDir | Out-Null
$installLog = Join-Path $logDir 'install.log'
$repairLog = Join-Path $logDir 'repair.log'
$uninstallLog = Join-Path $logDir 'uninstall.log'
$sentinel = Join-Path $DataDir 'phase8-uninstall-preservation.sentinel'
$installed = $false
try {
    Run-Msi @('/i',"`"$Msi`"",'/qn','/norestart','/l*v',"`"$installLog`"") 'MSI install'
    $installed = $true
    Verify-InstalledState
    if (Get-Process 'aethercore-desktop' -ErrorAction SilentlyContinue) {
        throw 'Desktop shell was unexpectedly auto-launched by elevated installation.'
    }

    'phase8-preserve' | Set-Content $sentinel -Encoding ascii
    # Repair must re-run the hardener and close deliberate ACL drift.
    & "$env:SystemRoot\System32\icacls.exe" $InstallDir /grant '*S-1-1-0:(OI)(CI)M' /Q | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Unable to inject ACL drift for repair test.' }
    Run-Msi @('/fa',"`"$Msi`"",'/qn','/norestart','/l*v',"`"$repairLog`"") 'MSI repair'
    Verify-InstalledState

    Run-Msi @('/x',"`"$Msi`"",'/qn','/norestart','/l*v',"`"$uninstallLog`"") 'MSI uninstall'
    $installed = $false
    if (Get-Service $ServiceName -ErrorAction SilentlyContinue) { throw 'Service remains after uninstall.' }
    if (Test-Path (Join-Path $InstallDir 'aethercore-desktop.exe')) { throw 'Desktop binary remains after uninstall.' }
    if (-not (Test-Path $sentinel)) { throw 'ProgramData preservation failed: service state sentinel was removed by uninstall.' }
    Remove-Item $sentinel -Force
    Write-Host 'Install → security verification → repair hardening → uninstall lifecycle passed.' -ForegroundColor Green
} finally {
    if ($installed) {
        try { Run-Msi @('/x',"`"$Msi`"",'/qn','/norestart') 'cleanup uninstall' } catch { Write-Warning $_ }
    }
}
