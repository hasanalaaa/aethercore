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
    [string]$ExpectedPreviousSourceSha

)
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path;Set-Location $Root
if($env:OS -ne 'Windows_NT'){throw 'Phase 16 installer lifecycle requires Windows.'}
if(-not $AcknowledgeDisposableMachine -and $env:AETHERCORE_INSTALLER_TEST_MACHINE -ne '1'){throw 'This test installs, repairs and uninstalls AetherCore. Use a disposable VM and acknowledge it.'}
$identity=[Security.Principal.WindowsIdentity]::GetCurrent();$principal=[Security.Principal.WindowsPrincipal]::new($identity)
if(-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Run installer lifecycle from an elevated PowerShell host.'}
$release=(Resolve-Path $ReleaseRoot).Path
$meta=Get-Content (Join-Path $release 'RELEASE-METADATA.json') -Raw|ConvertFrom-Json
$version=$meta.version;$artifacts=Join-Path $release 'artifacts';$msi=Join-Path $artifacts "AetherCore-$version-x64.msi";$bundle=Join-Path $artifacts "AetherCoreSetup-$version-x64.exe"
foreach($f in @($msi,$bundle)){if(-not(Test-Path $f)){throw "Release artifact missing: $f"};if((Get-AuthenticodeSignature $f).Status -ne 'Valid'){throw "Invalid Authenticode signature: $f"}}
if ($InstalledAcceptanceScript -or $PreviousBundlePath) {
    if ($ExpectedSourceSha -notmatch '^[0-9a-f]{40,64}$' -or $ExpectedBundleSha256 -notmatch '^[0-9a-f]{64}$' -or -not $AcceptanceEvidenceDirectory) { throw 'Signed RC acceptance identity/evidence parameters are missing.' }
    if ($meta.source_commit -ne $ExpectedSourceSha -or (Get-FileHash $bundle -Algorithm SHA256).Hash.ToLowerInvariant() -ne $ExpectedBundleSha256) { throw 'Signed RC lifecycle source/bundle pin mismatch.' }
    if (-not $PreviousBundlePath -or $ExpectedPreviousBundleSha256 -notmatch '^[0-9a-f]{64}$' -or $ExpectedPreviousSourceSha -notmatch '^[0-9a-f]{40,64}$') { throw 'Signed upgrade qualification requires a pinned previous signed bundle.' }
    $previous=(Resolve-Path $PreviousBundlePath).Path
    if ((Get-FileHash $previous -Algorithm SHA256).Hash.ToLowerInvariant() -ne $ExpectedPreviousBundleSha256) { throw 'Previous signed bundle hash mismatch.' }
    $previousSignature=Get-AuthenticodeSignature $previous
    if ($previousSignature.Status -ne 'Valid' -or $previousSignature.SignerCertificate.Thumbprint -ne ($env:AETHERCORE_CODESIGN_THUMBPRINT -replace '\s','').ToUpperInvariant()) { throw 'Previous bundle signature/signer is invalid.' }
}
$service='AetherCoreMaintenance';$installDir=Join-Path $env:ProgramW6432 'AetherCore';$dataDir=Join-Path $env:ProgramData 'AetherCore'
if(Get-Service $service -ErrorAction SilentlyContinue){throw 'Refusing GA lifecycle: AetherCore service already exists.'}
if(Test-Path $installDir){throw 'Refusing GA lifecycle: AetherCore install directory already exists.'}
if ($InstalledAcceptanceScript) {
    $acceptanceRoot=[IO.Path]::GetFullPath($AcceptanceEvidenceDirectory)
    $allowedRoot=[IO.Path]::GetFullPath((Join-Path $Root 'out')) + [IO.Path]::DirectorySeparatorChar
    if (-not $acceptanceRoot.StartsWith($allowedRoot,[StringComparison]::OrdinalIgnoreCase)) { throw 'Acceptance evidence must be confined to fresh test output.' }
    if (Test-Path $acceptanceRoot) { throw 'Acceptance evidence directory must be fresh.' }
    $packagedScript=Join-Path $release 'acceptance/p87-installed-acceptance.ps1'
    if ((Get-FileHash $InstalledAcceptanceScript -Algorithm SHA256).Hash -ne (Get-FileHash $packagedScript -Algorithm SHA256).Hash) { throw 'Installed acceptance script differs from packaged RC bytes.' }
}
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
    $verifyArgument=if ($VerifyCareRunId) { ' -VerifyCareRunId ' + (& $quote $VerifyCareRunId) } else { '' }
    $command="`$ErrorActionPreference='Stop'; try { & $(& $quote $packagedScript) -ReleaseRoot $(& $quote $release) -ExpectedSourceSha $(& $quote $ExpectedSourceSha) -ExpectedBundleSha256 $(& $quote $ExpectedBundleSha256) -Locale $(& $quote $Locale) -OutputPath $(& $quote $output) -TimeoutSeconds 300 -AcknowledgeDisposableMachine$verifyArgument; exit `$LASTEXITCODE } catch { exit 1 }"
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
    if (-not $installed -or $service -ne 'AetherCoreMaintenance' -or (-not $AcknowledgeDisposableMachine -and $env:AETHERCORE_INSTALLER_TEST_MACHINE -ne '1')) { throw 'Care restart requires this disposable isolated installed service.' }
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
$acceptanceStillRunning=$false
$sentinel=Join-Path $dataDir 'phase16-ga-preserve.sentinel';$installed=$false;$activeBundle=$bundle
try{
    Run-Step 'burn-install' { Run-Process $bundle @('/install','/quiet','/norestart') 'Burn install';$script:installed=$true }
    Run-Step 'installed-security-boundaries' { & (Join-Path $PSScriptRoot 'verify-installer-security.ps1') -MsiPath $msi -VerifyInstalledStateOnly -RequireSignedArtifacts -ExpectedPayloadDirectory (Join-Path $release 'payload');if($LASTEXITCODE -ne 0){throw 'Installed state verifier failed.'} }
    Run-Step 'program-data-preservation-sentinel' { 'phase16-preserve'|Set-Content $sentinel -Encoding ascii }
    Run-Step 'repair-closes-acl-drift' {
        & "$env:SystemRoot\System32\icacls.exe" $installDir /grant '*S-1-1-0:(OI)(CI)M' /Q|Out-Null
        if($LASTEXITCODE -ne 0){throw 'Unable to inject ACL drift.'}
        Run-Process msiexec.exe @('/fa',"`"$msi`"",'/qn','/norestart') 'MSI repair'
        & (Join-Path $PSScriptRoot 'verify-installer-security.ps1') -MsiPath $msi -VerifyInstalledStateOnly -RequireSignedArtifacts -ExpectedPayloadDirectory (Join-Path $release 'payload')
        if($LASTEXITCODE -ne 0){throw 'Post-repair state verifier failed.'}
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
        if(-not(Test-Path $sentinel)){throw 'ProgramData preservation sentinel was removed.'}
        Remove-Item $sentinel -Force
    }
    if ($PreviousBundlePath) {
        Run-Step 'signed-upgrade-preserves-owner-data' {
            $script:activeBundle=$previous
            Run-Process $previous @('/install','/quiet','/norestart') 'Previous signed bundle install';$script:installed=$true
            'phase16-upgrade-preserve' | Set-Content $sentinel -Encoding ascii
            $preservedHash=(Get-FileHash $sentinel -Algorithm SHA256).Hash
            Run-Process $bundle @('/install','/quiet','/norestart') 'Signed RC update';$script:activeBundle=$bundle
            & (Join-Path $PSScriptRoot 'verify-installer-security.ps1') -MsiPath $msi -VerifyInstalledStateOnly -RequireSignedArtifacts -ExpectedPayloadDirectory (Join-Path $release 'payload')
            if ($LASTEXITCODE -ne 0) { throw 'Updated signed RC installed security failed.' }
            if ((Get-FileHash $sentinel -Algorithm SHA256).Hash -ne $preservedHash) { throw 'Signed upgrade changed owner data.' }
            Run-Process $bundle @('/uninstall','/quiet','/norestart') 'Updated RC uninstall';$script:installed=$false
            if (Get-Service $service -ErrorAction SilentlyContinue) { throw 'Service remains after updated RC uninstall.' }
            if (Test-Path (Join-Path $installDir 'aethercore-desktop.exe')) { throw 'Updated desktop remains after uninstall.' }
            if ((Get-FileHash $sentinel -Algorithm SHA256).Hash -ne $preservedHash) { throw 'Updated uninstall changed owner data.' }
            Remove-Item $sentinel -Force
        }
    }
} finally {
    if($installed -and -not $acceptanceStillRunning){try{Run-Process $activeBundle @('/uninstall','/quiet','/norestart') 'cleanup Burn uninstall'}catch{Write-Warning $_}}
    if ($acceptanceStillRunning) { Write-Warning 'Blocked test worker remains active on the isolated VM; service uninstall was not started.' }
}
$doc=[ordered]@{schema='aethercore.ga-installer-lifecycle.v1';ok=$true;version=$version;source_commit=$meta.source_commit;bundle_sha256=(Get-FileHash $bundle -Algorithm SHA256).Hash.ToLowerInvariant();previous_source_commit=$ExpectedPreviousSourceSha;previous_bundle_sha256=$ExpectedPreviousBundleSha256;windows_build=[Environment]::OSVersion.Version.Build;architecture=$env:PROCESSOR_ARCHITECTURE;executed_utc=[DateTimeOffset]::UtcNow.ToString('o');steps=$steps}
$doc|ConvertTo-Json -Depth 6|Set-Content $out -Encoding utf8
Write-Host "Phase 16 Burn/MSI lifecycle passed. Evidence: $out" -ForegroundColor Green
