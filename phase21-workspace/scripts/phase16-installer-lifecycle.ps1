[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$ReleaseRoot,
    [switch]$AcknowledgeDisposableMachine,
    [string]$OutputPath = 'out\ga-evidence\installer-lifecycle.json',
    [string]$InstalledAcceptanceScript,
    [string]$ExpectedSourceSha,
    [string]$ExpectedBundleSha256,
    [string]$AcceptanceEvidenceDirectory,
    [string]$PreviousBundlePath,
    [string]$ExpectedPreviousBundleSha256,
    [string]$ExpectedPreviousSourceSha,
    # Owner decision D32: the RC and its upgrade baseline are unsigned, and must really be unsigned.
    [switch]$UnsignedByDecisionD32,
    # Owner decision D33: run on the owner's own PC instead of a disposable machine.
    [switch]$OwnerHostAccepted
)
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path;Set-Location $Root
if($env:OS -ne 'Windows_NT'){throw 'Phase 16 installer lifecycle requires Windows.'}
if($OwnerHostAccepted -and $AcknowledgeDisposableMachine){throw 'Choose one host: -OwnerHostAccepted (D33) or -AcknowledgeDisposableMachine.'}
if(-not $OwnerHostAccepted -and -not $AcknowledgeDisposableMachine -and $env:AETHERCORE_INSTALLER_TEST_MACHINE -ne '1'){throw 'This test installs, repairs and uninstalls AetherCore. Use a disposable VM and acknowledge it, or the owner host under D33 with -OwnerHostAccepted.'}
function Assert-OwnerDecision([string]$Id){
    $decisions=Join-Path $Root 'docs/roadmap/DECISIONS.md'
    if(-not (Test-Path -LiteralPath $decisions) -or (Get-Content -LiteralPath $decisions -Raw) -notmatch "(?m)^\| $Id \|"){throw "Owner decision $Id is not recorded in docs/roadmap/DECISIONS.md."}
}
function Assert-RcSignature([string]$Path){
    $status=(Get-AuthenticodeSignature $Path).Status.ToString()
    if($UnsignedByDecisionD32){if($status -ne 'NotSigned'){throw "D32 artifact is not unsigned: $Path ($status)"}}
    elseif($status -ne 'Valid'){throw "Invalid Authenticode signature: $Path"}
}
if($OwnerHostAccepted){Assert-OwnerDecision 'D33'}
if($UnsignedByDecisionD32){Assert-OwnerDecision 'D32'}
$hostMode=if($OwnerHostAccepted){'owner-host-d33'}else{'disposable-vm'}
$identity=[Security.Principal.WindowsIdentity]::GetCurrent();$principal=[Security.Principal.WindowsPrincipal]::new($identity)
if(-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Run installer lifecycle from an elevated PowerShell host.'}
$release=(Resolve-Path $ReleaseRoot).Path
$meta=Get-Content (Join-Path $release 'RELEASE-METADATA.json') -Raw|ConvertFrom-Json
$version=$meta.version;$artifacts=Join-Path $release 'artifacts';$msi=Join-Path $artifacts "AetherCore-$version-x64.msi";$bundle=Join-Path $artifacts "AetherCoreSetup-$version-x64.exe"
foreach($f in @($msi,$bundle)){if(-not(Test-Path $f)){throw "Release artifact missing: $f"};Assert-RcSignature $f}
if ($InstalledAcceptanceScript -or $PreviousBundlePath) {
    if ($ExpectedSourceSha -notmatch '^[0-9a-f]{40,64}$' -or $ExpectedBundleSha256 -notmatch '^[0-9a-f]{64}$' -or -not $AcceptanceEvidenceDirectory) { throw 'Signed RC acceptance identity/evidence parameters are missing.' }
    if ($meta.source_commit -ne $ExpectedSourceSha -or (Get-FileHash $bundle -Algorithm SHA256).Hash.ToLowerInvariant() -ne $ExpectedBundleSha256) { throw 'Signed RC lifecycle source/bundle pin mismatch.' }
}
# Without a baseline the upgrade step is not run and not recorded; rc-provenance still requires it,
# so such evidence observes the installed product but can never promote.
if ($PreviousBundlePath) {
    if ($ExpectedPreviousBundleSha256 -notmatch '^[0-9a-f]{64}$' -or $ExpectedPreviousSourceSha -notmatch '^[0-9a-f]{40,64}$') { throw 'Signed upgrade qualification requires a pinned previous signed bundle.' }
    $previous=(Resolve-Path $PreviousBundlePath).Path
    if ((Get-FileHash $previous -Algorithm SHA256).Hash.ToLowerInvariant() -ne $ExpectedPreviousBundleSha256) { throw 'Previous signed bundle hash mismatch.' }
    if ($UnsignedByDecisionD32) { Assert-RcSignature $previous } else {
        $previousSignature=Get-AuthenticodeSignature $previous
        if ($previousSignature.Status -ne 'Valid' -or $previousSignature.SignerCertificate.Thumbprint -ne ($env:AETHERCORE_CODESIGN_THUMBPRINT -replace '\s','').ToUpperInvariant()) { throw 'Previous bundle signature/signer is invalid.' }
    }
}
$service='AetherCoreMaintenance';$installDir=Join-Path $env:ProgramW6432 'AetherCore';$dataDir=Join-Path $env:ProgramData 'AetherCore'
function Assert-CleanHost{
    if(Get-Service $service -ErrorAction SilentlyContinue){throw 'Refusing GA lifecycle: AetherCore service already exists.'}
    if(Test-Path $installDir){throw 'Refusing GA lifecycle: AetherCore install directory already exists.'}
}
# On the owner host the guards run after its own install is backed up and removed.
if(-not $OwnerHostAccepted){Assert-CleanHost}
if ($InstalledAcceptanceScript) {
    $acceptanceRoot=[IO.Path]::GetFullPath($AcceptanceEvidenceDirectory)
    $allowedRoot=[IO.Path]::GetFullPath((Join-Path $Root 'out')) + [IO.Path]::DirectorySeparatorChar
    if (-not $acceptanceRoot.StartsWith($allowedRoot,[StringComparison]::OrdinalIgnoreCase)) { throw 'Acceptance evidence must be confined to fresh test output.' }
    if (Test-Path $acceptanceRoot) { throw 'Acceptance evidence directory must be fresh.' }
    $packagedScript=Join-Path $release 'acceptance/p87-installed-acceptance.ps1'
    if ((Get-FileHash $InstalledAcceptanceScript -Algorithm SHA256).Hash -ne (Get-FileHash $packagedScript -Algorithm SHA256).Hash) { throw 'Installed acceptance script differs from packaged RC bytes.' }
}
$signedArtifacts=-not $UnsignedByDecisionD32
$out=[IO.Path]::GetFullPath((Join-Path $Root $OutputPath));New-Item -ItemType Directory -Force (Split-Path $out -Parent)|Out-Null
$steps=New-Object System.Collections.Generic.List[object]
function Run-Step([string]$Name,[scriptblock]$Body){$t=[DateTimeOffset]::UtcNow;try{&$Body;$steps.Add([pscustomobject]@{name=$Name;ok=$true;started_utc=$t.ToString('o');finished_utc=[DateTimeOffset]::UtcNow.ToString('o')})}catch{$steps.Add([pscustomobject]@{name=$Name;ok=$false;error=$_.Exception.Message;started_utc=$t.ToString('o');finished_utc=[DateTimeOffset]::UtcNow.ToString('o')});throw}}
function Run-Process([string]$File,[string[]]$ProcessArgs,[string]$Label){$p=Start-Process $File -ArgumentList $ProcessArgs -PassThru -Wait;if($p.ExitCode -notin @(0,3010)){throw "$Label failed with exit code $($p.ExitCode)."}}
function Write-BlockedAcceptance([string]$Path,[string]$Locale,[string]$Reason) {
    [ordered]@{schema='aethercore.p87-installed-acceptance.v1';source_commit=$ExpectedSourceSha;bundle_sha256=$ExpectedBundleSha256;locale=$Locale;ordinary_user=$false;cases=@();disposition='blocked';reason=$Reason} |
        ConvertTo-Json -Depth 6 | Set-Content $Path -Encoding utf8
}
function Invoke-OrdinaryInstalledAcceptance([string]$Locale,[string]$VerifyCareRunId='',[string]$ExpectedOwnerSid='',[switch]$AllowRestartPending,[DateTimeOffset]$ObservationDeadline=[DateTimeOffset]::UtcNow.AddSeconds(660)) {
    $output=Join-Path $acceptanceRoot "installed-$Locale.json"
    $owners=@(Get-CimInstance Win32_Process -Filter "Name='explorer.exe'" | ForEach-Object {
        $owner=Invoke-CimMethod -InputObject $_ -MethodName GetOwnerSid
        if ($owner.ReturnValue -eq 0 -and $owner.Sid -match '^S-1-5-21-') { $owner.Sid }
    } | Select-Object -Unique)
    if ($owners.Count -ne 1) {
        Write-BlockedAcceptance $output $Locale 'No unique ordinary interactive desktop is available.'
        throw 'Ordinary installed acceptance is blocked: no unique explorer owner.'
    }
    if ($ExpectedOwnerSid -and $owners[0] -ne $ExpectedOwnerSid) { throw 'Ordinary verification desktop owner changed.' }
    if ([DateTimeOffset]::UtcNow -ge $ObservationDeadline) { throw 'Ordinary acceptance observation budget expired before task start.' }
    # Permission is local to disposable test evidence; service/pipe/install ACLs are unchanged.
    $acl=Get-Acl $acceptanceRoot
    $sid=[Security.Principal.SecurityIdentifier]::new($owners[0])
    $rule=[Security.AccessControl.FileSystemAccessRule]::new($sid,'Modify','ContainerInherit,ObjectInherit','None','Allow')
    $acl.SetAccessRule($rule);Set-Acl $acceptanceRoot $acl
    $quote={param($value) "'" + ($value -replace "'","''") + "'"}
    $hostSwitch=if ($OwnerHostAccepted) { '-OwnerHostAccepted' } else { '-AcknowledgeDisposableMachine' }
    $verifyArgument=if ($VerifyCareRunId) { ' -VerifyCareRunId ' + (& $quote $VerifyCareRunId) } else { '' }
    $command="`$ErrorActionPreference='Stop'; try { & $(& $quote $packagedScript) -ReleaseRoot $(& $quote $release) -ExpectedSourceSha $(& $quote $ExpectedSourceSha) -ExpectedBundleSha256 $(& $quote $ExpectedBundleSha256) -Locale $(& $quote $Locale) -OutputPath $(& $quote $output) -TimeoutSeconds 300 $hostSwitch$verifyArgument; exit `$LASTEXITCODE } catch { exit 1 }"
    $encoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($command))
    $name='AetherCore-RC-Acceptance-' + [Guid]::NewGuid().ToString('N')
    $action=New-ScheduledTaskAction -Execute (Get-Process -Id $PID).Path -Argument "-NoProfile -NonInteractive -EncodedCommand $encoded" -WorkingDirectory $release
    $principal=New-ScheduledTaskPrincipal -UserId $owners[0] -LogonType Interactive -RunLevel Limited
    # No forced stop: the probe bounds its work; observer timeout is not worker completion.
    $settings=New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::Zero)
    $completed=$false
    try {
        Register-ScheduledTask -TaskName $name -Action $action -Principal $principal -Settings $settings -ErrorAction Stop | Out-Null
        $started=Get-Date;$script:acceptanceStillRunning=$true;Start-ScheduledTask -TaskName $name -ErrorAction Stop
        do {
            $task=Get-ScheduledTask -TaskName $name -ErrorAction Stop
            $info=Get-ScheduledTaskInfo -TaskName $name -ErrorAction Stop
            if ($task.State -eq 'Ready' -and $info.LastRunTime -ge $started.AddSeconds(-2)) { $completed=$true;$script:acceptanceStillRunning=$false;break }
            Start-Sleep -Milliseconds 250
        } while ([DateTimeOffset]::UtcNow -lt $ObservationDeadline)
        if (-not $completed) {
            $script:acceptanceStillRunning=$true
            Write-BlockedAcceptance $output $Locale "Ordinary test task remains active: $name"
            throw 'Ordinary installed acceptance observer timed out; actual task/service ownership retained.'
        }
        # The scheduled shell can exit while an API worker still owns assessment work.
        # Only an explicit producer drain receipt permits service teardown.
        $workerReleased=$false
        if (Test-Path $output) {
            try { $receipt=Get-Content $output -Raw | ConvertFrom-Json;$workerReleased=$receipt.worker_ownership_released -is [bool] -and $receipt.worker_ownership_released } catch { }
        }
        if (-not $workerReleased) {
            $script:acceptanceStillRunning=$true
            throw 'Ordinary installed acceptance did not release nested worker ownership; service cleanup retained.'
        }
        $pendingRestart=$AllowRestartPending -and $info.LastTaskResult -eq 1 -and $receipt.restart_pending -is [bool] -and $receipt.restart_pending
        if (($info.LastTaskResult -ne 0 -and -not $pendingRestart) -or -not (Test-Path $output)) {
            if (-not (Test-Path $output)) { Write-BlockedAcceptance $output $Locale 'Ordinary probe exited without evidence.' }
            throw 'Ordinary installed acceptance did not produce successful evidence.'
        }
        return [pscustomobject]@{Receipt=$receipt;OwnerSid=$owners[0]}
    } finally {
        if ($completed) { Unregister-ScheduledTask -TaskName $name -Confirm:$false -ErrorAction Stop }
    }
}
function Invoke-InstalledAcceptanceWithRestart([string]$Locale) {
    # The existing pre-install guards reject any pre-existing service or product directory.
    if (-not $installed -or $service -ne 'AetherCoreMaintenance' -or (-not $AcknowledgeDisposableMachine -and -not $OwnerHostAccepted -and $env:AETHERCORE_INSTALLER_TEST_MACHINE -ne '1')) { throw 'Care restart requires this disposable isolated installed service, or the owner host under D33.' }
    $deadline=[DateTimeOffset]::UtcNow.AddSeconds(660)
    $first=Invoke-OrdinaryInstalledAcceptance $Locale -AllowRestartPending -ObservationDeadline $deadline
    $receipt=$first.Receipt
    if ($receipt.source_commit -ne $ExpectedSourceSha -or $receipt.bundle_sha256 -ne $ExpectedBundleSha256 -or $receipt.locale -ne $Locale -or $receipt.read_only -isnot [bool] -or $receipt.read_only -or $receipt.token.sid -ne $first.OwnerSid) { throw 'Care restart report identity does not match this signed RC and desktop.' }
    if ($receipt.restart_pending -is [bool] -and $receipt.restart_pending) {
        $care=@($receipt.cases | Where-Object id -eq 'p76-care-timeline-persistence')
        $runId=$receipt.care_run_id
        if ($runId -notmatch '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' -or $receipt.desktop_closed -isnot [bool] -or -not $receipt.desktop_closed -or $care.Count -ne 1 -or $care[0].checks.reconnect -isnot [bool] -or -not $care[0].checks.reconnect -or $care[0].checks.restart -isnot [bool] -or $care[0].checks.restart) { throw 'Care restart lacks a drained same-run reconnect receipt.' }
        if ([DateTimeOffset]::UtcNow.AddSeconds(60) -ge $deadline) { throw 'Care restart observation budget is exhausted.' }
        $controller=Get-Service -Name $service -ErrorAction Stop
        try {
            $controller.Stop()
            $controller.WaitForStatus([ServiceProcess.ServiceControllerStatus]::Stopped,[TimeSpan]::FromSeconds(30))
            $controller.Start()
            $controller.WaitForStatus([ServiceProcess.ServiceControllerStatus]::Running,[TimeSpan]::FromSeconds(30))
        } finally { $controller.Dispose() }
        $verified=Invoke-OrdinaryInstalledAcceptance $Locale -VerifyCareRunId $runId -ExpectedOwnerSid $first.OwnerSid -ObservationDeadline $deadline
        $finalCare=@($verified.Receipt.cases | Where-Object id -eq 'p76-care-timeline-persistence')
        if ($verified.Receipt.care_run_id -ne $runId -or $verified.Receipt.restart_pending -isnot [bool] -or $verified.Receipt.restart_pending -or $finalCare.Count -ne 1 -or $finalCare[0].checks.restart -isnot [bool] -or -not $finalCare[0].checks.restart) { throw 'Same Care run was not proved after isolated service restart.' }
    }
}
function Stop-OwnerHostService {
    $controller=Get-Service -Name $service -ErrorAction SilentlyContinue
    if (-not $controller) { return }
    try {
        if ($controller.Status -ne 'Stopped') { $controller.Stop() }
        $controller.WaitForStatus([ServiceProcess.ServiceControllerStatus]::Stopped,[TimeSpan]::FromSeconds(30))
    } finally { $controller.Dispose() }
}
function Backup-OwnerHostState([string]$DataDirectory,[string]$BackupParent) {
    $target=Join-Path $BackupParent ('AetherCore-owner-backup-' + [DateTimeOffset]::UtcNow.ToString('yyyyMMddTHHmmssZ'))
    if (Test-Path -LiteralPath $target) { throw "Owner backup directory already exists: $target" }
    New-Item -ItemType Directory -Path $target | Out-Null
    # The copy of the owner's data is SYSTEM/Administrators-only, never ProgramData's inherited Users grants.
    $acl=[Security.AccessControl.DirectorySecurity]::new()
    $acl.SetSecurityDescriptorSddlForm('D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)','Access')
    Set-Acl -LiteralPath $target -AclObject $acl
    # A running service may be writing its database; the copy is taken from a stopped service.
    Stop-OwnerHostService
    $copy=Join-Path $target 'ProgramData-AetherCore'
    $lines=New-Object System.Collections.Generic.List[string]
    if (Test-Path -LiteralPath $DataDirectory) {
        & "$env:SystemRoot\System32\robocopy.exe" $DataDirectory $copy /E /COPY:DAT /DCOPY:DAT /R:0 /W:0 /NP /NFL /NDL /NJH /NJS | Out-Null
        if ($LASTEXITCODE -ge 8) { throw "Owner data backup copy failed (robocopy $LASTEXITCODE)." }
        $sourceRoot=(Resolve-Path -LiteralPath $DataDirectory).Path
        foreach ($file in Get-ChildItem -LiteralPath $sourceRoot -Recurse -File -Force | Sort-Object FullName) {
            $relative=$file.FullName.Substring($sourceRoot.TrimEnd('\').Length).TrimStart('\')
            $hash=(Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
            $copied=Join-Path $copy $relative
            if (-not (Test-Path -LiteralPath $copied) -or (Get-FileHash -LiteralPath $copied -Algorithm SHA256).Hash.ToLowerInvariant() -ne $hash) { throw "Owner data backup differs from the source: $relative" }
            $lines.Add("$hash  $($relative -replace '\\','/')")
        }
    }
    $manifest=Join-Path $target 'MANIFEST.sha256'
    Set-Content -LiteralPath $manifest -Value $lines -Encoding ascii
    return [ordered]@{directory=$target;manifest_sha256=(Get-FileHash -LiteralPath $manifest -Algorithm SHA256).Hash.ToLowerInvariant();files=$lines.Count}
}
function Get-OwnerHostUninstallEntries {
    @('HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*','HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*') |
        ForEach-Object { Get-ItemProperty -Path $_ -ErrorAction SilentlyContinue } |
        Where-Object { $_.DisplayName -like 'AetherCore*' }
}
function Remove-OwnerHostInstall {
    $entries=@(Get-OwnerHostUninstallEntries)
    $bundles=@($entries | Where-Object { $_.BundleCachePath })
    $packages=@($entries | Where-Object { $_.WindowsInstaller -eq 1 -and $_.PSChildName -match '^\{[0-9A-Fa-f-]{36}\}$' })
    if ($bundles.Count -gt 1 -or ($bundles.Count -eq 0 -and $packages.Count -gt 1)) { throw 'Owner host has more than one AetherCore installation; remove the extra one by hand first.' }
    if ($bundles.Count -eq 1) { Run-Process $bundles[0].BundleCachePath @('/uninstall','/quiet','/norestart') 'Owner-host prior bundle uninstall' }
    elseif ($packages.Count -eq 1) { Run-Process msiexec.exe @('/x',$packages[0].PSChildName,'/qn','/norestart') 'Owner-host prior MSI uninstall' }
    Assert-CleanHost
}
function Restore-OwnerHostData {
    # The prior-uninstall and every lifecycle uninstall run PurgeMachineData, so the owner's
    # data exists only in the backup; it is mirrored back (dropping the fresh DB's -wal/-shm)
    # from a stopped service and must match the manifest byte for byte.
    if (-not $script:ownerBackup) { throw 'Owner data cannot be restored: no backup was taken.' }
    Stop-OwnerHostService
    $copy=Join-Path $script:ownerBackup.directory 'ProgramData-AetherCore'
    if (Test-Path -LiteralPath $copy) {
        & "$env:SystemRoot\System32\robocopy.exe" $copy $dataDir /MIR /COPY:DAT /DCOPY:DAT /R:0 /W:0 /NP /NFL /NDL /NJH /NJS | Out-Null
        if ($LASTEXITCODE -ge 8) { throw "Owner data restore copy failed (robocopy $LASTEXITCODE)." }
    }
    foreach ($line in Get-Content -LiteralPath (Join-Path $script:ownerBackup.directory 'MANIFEST.sha256')) {
        if (-not $line) { continue }
        $hash,$name=$line -split '  ',2
        $path=Join-Path $dataDir $name
        if (-not (Test-Path -LiteralPath $path) -or (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $hash) { throw "Restored owner data differs from the backup: $name" }
    }
    $script:ownerDataRestored=$true
}
function Restore-OwnerHostService {
    if (-not $script:installed) { Run-Process $bundle @('/install','/quiet','/norestart') 'Owner-host RC reinstall';$script:installed=$true;$script:activeBundle=$bundle }
    Restore-OwnerHostData
    $controller=Get-Service -Name $service -ErrorAction Stop
    try {
        if ($controller.Status -ne 'Running') { $controller.Start() }
        $controller.WaitForStatus([ServiceProcess.ServiceControllerStatus]::Running,[TimeSpan]::FromSeconds(30))
    } finally { $controller.Dispose() }
    $script:serviceRunningAtEnd=$true
}
$acceptanceStillRunning=$false;$serviceRunningAtEnd=$false;$ownerBackup=$null;$ownerDataRestored=$false
$sentinel=Join-Path $dataDir 'phase16-ga-preserve.sentinel';$installed=$false;$activeBundle=$bundle
try{
    if ($OwnerHostAccepted) {
        Run-Step 'owner-host-backup' { $script:ownerBackup=Backup-OwnerHostState $dataDir $env:ProgramData }
        Run-Step 'owner-host-prior-uninstall' { Remove-OwnerHostInstall }
    }
    Run-Step 'burn-install' { Run-Process $bundle @('/install','/quiet','/norestart') 'Burn install';$script:installed=$true }
    Run-Step 'installed-security-boundaries' { & (Join-Path $PSScriptRoot 'verify-installer-security.ps1') -MsiPath $msi -VerifyInstalledStateOnly -RequireSignedArtifacts:$signedArtifacts -ExpectedPayloadDirectory (Join-Path $release 'payload');if($LASTEXITCODE -ne 0){throw 'Installed state verifier failed.'} }
    Run-Step 'program-data-preservation-sentinel' { 'phase16-preserve'|Set-Content $sentinel -Encoding ascii }
    Run-Step 'repair-closes-acl-drift' {
        & "$env:SystemRoot\System32\icacls.exe" $installDir /grant '*S-1-1-0:(OI)(CI)M' /Q|Out-Null
        if($LASTEXITCODE -ne 0){throw 'Unable to inject ACL drift.'}
        Run-Process msiexec.exe @('/fa',"`"$msi`"",'/qn','/norestart') 'MSI repair'
        & (Join-Path $PSScriptRoot 'verify-installer-security.ps1') -MsiPath $msi -VerifyInstalledStateOnly -RequireSignedArtifacts:$signedArtifacts -ExpectedPayloadDirectory (Join-Path $release 'payload')
        if($LASTEXITCODE -ne 0){throw 'Post-repair state verifier failed.'}
        if(-not(Test-Path $sentinel)){throw 'ProgramData preservation sentinel was removed by MSI repair.'}
    }
    if ($InstalledAcceptanceScript) {
        foreach ($locale in @('en','ar')) {
            Invoke-InstalledAcceptanceWithRestart $locale
        }
    }
    Run-Step 'burn-uninstall' { Run-Process $bundle @('/uninstall','/quiet','/norestart') 'Burn uninstall';$script:installed=$false }
    Run-Step 'uninstall-clean-state' {
        if(Get-Service $service -ErrorAction SilentlyContinue){throw 'Service remains after Burn uninstall.'}
        if(Test-Path (Join-Path $installDir 'aethercore-desktop.exe')){throw 'Desktop binary remains after Burn uninstall.'}
        # release/UNINSTALL.txt tells users a full uninstall deletes ALL machine data (PurgeMachineData).
        if(Test-Path $dataDir){throw "Machine data remains after Burn uninstall: $dataDir"}
    }
    if ($PreviousBundlePath) {
        Run-Step 'signed-upgrade-preserves-owner-data' {
            $script:activeBundle=$previous
            Run-Process $previous @('/install','/quiet','/norestart') 'Previous signed bundle install';$script:installed=$true
            'phase16-upgrade-preserve' | Set-Content $sentinel -Encoding ascii
            $preservedHash=(Get-FileHash $sentinel -Algorithm SHA256).Hash
            Run-Process $bundle @('/install','/quiet','/norestart') 'Signed RC update';$script:activeBundle=$bundle
            & (Join-Path $PSScriptRoot 'verify-installer-security.ps1') -MsiPath $msi -VerifyInstalledStateOnly -RequireSignedArtifacts:$signedArtifacts -ExpectedPayloadDirectory (Join-Path $release 'payload')
            if ($LASTEXITCODE -ne 0) { throw 'Updated signed RC installed security failed.' }
            if ((Get-FileHash $sentinel -Algorithm SHA256).Hash -ne $preservedHash) { throw 'Signed upgrade changed owner data.' }
            Run-Process $bundle @('/uninstall','/quiet','/norestart') 'Updated RC uninstall';$script:installed=$false
            if (Get-Service $service -ErrorAction SilentlyContinue) { throw 'Service remains after updated RC uninstall.' }
            if (Test-Path (Join-Path $installDir 'aethercore-desktop.exe')) { throw 'Updated desktop remains after uninstall.' }
            if (Test-Path $dataDir) { throw "Machine data remains after updated RC uninstall: $dataDir" }
        }
    }
    if ($OwnerHostAccepted) { Run-Step 'owner-host-service-restored' { Restore-OwnerHostService } }
} finally {
    if ($OwnerHostAccepted) {
        # The owner keeps a running AetherCore even when a step failed; nothing is uninstalled here.
        if (-not $serviceRunningAtEnd -and -not $acceptanceStillRunning -and $ownerBackup) { try { Restore-OwnerHostService } catch { Write-Warning "Owner-host restore failed: $_ The owner's data is in $($ownerBackup.directory)" } }
        # No backup means nothing was uninstalled yet; only the service the backup stopped is started again.
        elseif (-not $ownerBackup -and (Get-Service $service -ErrorAction SilentlyContinue)) { try { Start-Service $service } catch { Write-Warning "Owner-host service start failed: $_" } }
    } elseif($installed -and -not $acceptanceStillRunning){try{Run-Process $activeBundle @('/uninstall','/quiet','/norestart') 'cleanup Burn uninstall'}catch{Write-Warning $_}}
    if ($acceptanceStillRunning) { Write-Warning 'Blocked test worker remains active on the isolated VM; service uninstall was not started.' }
}
$doc=[ordered]@{schema='aethercore.ga-installer-lifecycle.v1';ok=$true;host=$hostMode;owner_backup=$ownerBackup;owner_data_restored=$ownerDataRestored;service_running_at_end=$serviceRunningAtEnd;signing=$(if($UnsignedByDecisionD32){'unsigned by owner decision D32'}else{'authenticode'});version=$version;source_commit=$meta.source_commit;bundle_sha256=(Get-FileHash $bundle -Algorithm SHA256).Hash.ToLowerInvariant();previous_source_commit=$ExpectedPreviousSourceSha;previous_bundle_sha256=$ExpectedPreviousBundleSha256;windows_build=[Environment]::OSVersion.Version.Build;architecture=$env:PROCESSOR_ARCHITECTURE;executed_utc=[DateTimeOffset]::UtcNow.ToString('o');steps=$steps}
$doc|ConvertTo-Json -Depth 6|Set-Content $out -Encoding utf8
Write-Host "Phase 16 Burn/MSI lifecycle passed. Evidence: $out" -ForegroundColor Green
