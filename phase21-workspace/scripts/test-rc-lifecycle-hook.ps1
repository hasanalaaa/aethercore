#Requires -Version 7.0
# Execute the actual ordinary-user hook with task/ACL cmdlets replaced by local fixtures.
# No task, installation, signer or existing service is invoked.
$ErrorActionPreference='Stop'
$source=Join-Path $PSScriptRoot 'phase16-installer-lifecycle.ps1'
$tokens=$null;$errors=$null
$ast=[Management.Automation.Language.Parser]::ParseFile($source,[ref]$tokens,[ref]$errors)
if ($errors.Count) { throw 'Lifecycle source did not parse.' }
foreach ($name in @('Run-Process','Write-BlockedAcceptance','Invoke-OrdinaryInstalledAcceptance','Invoke-InstalledAcceptanceWithRestart')) {
    $definition=$ast.Find({param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq $name},$true)
    if (-not $definition) { if ($name -eq 'Invoke-InstalledAcceptanceWithRestart') { continue };throw "Actual hook missing: $name" }
    . ([scriptblock]::Create($definition.Extent.Text))
}
$acceptanceRoot=Join-Path ([IO.Path]::GetTempPath()) ('aethercore-rc-hook-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $acceptanceRoot | Out-Null
$ExpectedSourceSha='a'*40;$ExpectedBundleSha256='b'*64
$release=$acceptanceRoot;$packagedScript=Join-Path $acceptanceRoot ('fixture-probe' + '.ps1')
Set-Content $packagedScript '# temporary fixture; never executed'
$fixtureOwners=@();$fixtureStartFails=$false;$fixtureTaskExit=0;$fixtureOwnershipReleased=$true
$registered=0;$unregistered=0;$principalObserved=$null;$restartFixture=$false;$fixtureSid='S-1-5-21-1-2-3-1001';$fixtureOwnerChanges=$false;$probeCalls=0;$serviceCalls=[Collections.Generic.List[string]]::new();$taskCommands=[Collections.Generic.List[string]]::new()
function Get-CimInstance { param($Filter) @($fixtureOwners) }
function Invoke-CimMethod { param($InputObject,$MethodName) @{ReturnValue=0;Sid=$fixtureSid} }
function Get-Acl { param($Path) $value=[pscustomobject]@{};$value|Add-Member ScriptMethod SetAccessRule {param($rule)};return $value }
function Set-Acl { param($Path,$AclObject) }
function New-ScheduledTaskAction { param($Execute,$Argument,$WorkingDirectory) $encoded=$Argument.Split(' ')[-1];$script:taskCommands.Add([Text.Encoding]::Unicode.GetString([Convert]::FromBase64String($encoded)));@{Execute=$Execute;Argument=$Argument} }
function New-ScheduledTaskPrincipal { param($UserId,$LogonType,$RunLevel) $script:principalObserved=@{SID=$UserId;LogonType=$LogonType;RunLevel=$RunLevel};return $principalObserved }
function New-ScheduledTaskSettingsSet { param($ExecutionTimeLimit) if ($ExecutionTimeLimit -ne [TimeSpan]::Zero) { throw 'Worker would be force-stopped.' };@{} }
function Register-ScheduledTask { param($TaskName,$Action,$Principal,$Settings,$ErrorAction) $script:registered++ }
function Start-ScheduledTask { param($TaskName,$ErrorAction)
    if ($fixtureStartFails) { throw 'controlled start/status failure' }
    $receipt=@{worker_ownership_released=$fixtureOwnershipReleased}
    if ($restartFixture) {
        $script:probeCalls++
        $receipt=@{worker_ownership_released=$fixtureOwnershipReleased;desktop_closed=$true;source_commit=$ExpectedSourceSha;bundle_sha256=$ExpectedBundleSha256;locale='en';read_only=$false;ordinary_user=$true;token=@{sid='S-1-5-21-1-2-3-1001';elevated=$false};care_run_id='11111111-2222-4333-8444-555555555555';restart_pending=($probeCalls -eq 1);cases=@(@{id='p76-care-timeline-persistence';checks=@{reconnect=$true;restart=($probeCalls -gt 1)}})}
        $script:fixtureTaskExit=$(if ($probeCalls -eq 1) { 1 } else { 0 })
    }
    $receipt|ConvertTo-Json -Depth 8|Set-Content $output
}
function Get-ScheduledTask { param($TaskName,$ErrorAction) @{State='Ready'} }
function Get-ScheduledTaskInfo { param($TaskName,$ErrorAction) @{LastRunTime=(Get-Date);LastTaskResult=$fixtureTaskExit} }
function Unregister-ScheduledTask { param($TaskName,[switch]$Confirm,$ErrorAction) $script:unregistered++ }
function Get-Service { param($Name,$ErrorAction)
    Require ($Name -eq 'AetherCoreMaintenance') 'Wrong service selected.'
    $value=[pscustomobject]@{}
    $value|Add-Member ScriptMethod Stop { $script:serviceCalls.Add('Stop') }
    $value|Add-Member ScriptMethod Start { $script:serviceCalls.Add('Start');if ($fixtureOwnerChanges) { $script:fixtureSid='S-1-5-21-1-2-3-2002' } }
    $value|Add-Member ScriptMethod WaitForStatus { param($State,$Timeout)
        Require ($Timeout.TotalSeconds -le 30) 'Restart wait widened.'
        $script:serviceCalls.Add($State.ToString())
    }
    $value|Add-Member ScriptMethod Dispose { }
    return $value
}
$processCalls=[Collections.Generic.List[object]]::new();$fixtureProcessExit=0
function Start-Process { param([string]$FilePath,[string[]]$ArgumentList,[switch]$PassThru,[switch]$Wait)
    $processCalls.Add(@{File=$FilePath;Arguments=@($ArgumentList);PassThru=[bool]$PassThru;Wait=[bool]$Wait})
    return [pscustomobject]@{ExitCode=$fixtureProcessExit}
}
function Require([bool]$Value,[string]$Reason) { if (-not $Value) { throw $Reason } }
try {
    Run-Process 'fixture-setup.exe' @('/install','/quiet','/norestart') 'Fixture install'
    Require ($processCalls.Count -eq 1 -and ($processCalls[0].Arguments -join ',') -eq '/install,/quiet,/norestart') 'Actual Run-Process lost its installer arguments.'
    Require ($processCalls[0].File -eq 'fixture-setup.exe' -and $processCalls[0].Wait -and $processCalls[0].PassThru) 'Actual Run-Process lost synchronous process ownership.'
    $fixtureProcessExit=3010
    Run-Process 'msiexec.exe' @('/fa','"C:\fixture path\test.msi"','/qn','/norestart') 'Fixture repair'
    Require (($processCalls[1].Arguments -join ',') -eq '/fa,"C:\fixture path\test.msi",/qn,/norestart') 'Actual Run-Process changed quoted MSI arguments.'
    $fixtureProcessExit=1603
    try { Run-Process 'fixture-setup.exe' @('/uninstall','/quiet') 'Fixture uninstall';throw 'Failed process accepted.' } catch {
        Require ($_.Exception.Message -eq 'Fixture uninstall failed with exit code 1603.') 'Actual Run-Process lost exit failure or label.'
    }
    $fixtureProcessExit=0
    $acceptanceStillRunning=$false
    try { Invoke-OrdinaryInstalledAcceptance 'en';throw 'No desktop was accepted.' } catch {
        Require ($_.Exception.Message -match 'no unique explorer owner') 'Wrong rejection for absent desktop.'
    }
    $blocked=Get-Content (Join-Path $acceptanceRoot 'installed-en.json') -Raw | ConvertFrom-Json
    Require ($blocked.disposition -eq 'blocked' -and -not $blocked.ordinary_user -and $registered -eq 0) 'Absent desktop created a task or passed.'
    $fixtureOwners=@([pscustomobject]@{Name='explorer.exe'})
    $null=Invoke-OrdinaryInstalledAcceptance 'ar'
    Require ($registered -eq 1 -and $unregistered -eq 1 -and -not $acceptanceStillRunning) 'Completed hook did not release task ownership.'
    Require ($principalObserved.LogonType -eq 'Interactive' -and $principalObserved.RunLevel -eq 'Limited') 'Hook requested elevated or non-interactive token.'
    $fixtureStartFails=$true
    try { Invoke-OrdinaryInstalledAcceptance 'en';throw 'Controlled failure accepted.' } catch {
        Require ($_.Exception.Message -match 'controlled start/status failure') 'Wrong worker failure rejection.'
    }
    Require ($acceptanceStillRunning -and $registered -eq 2 -and $unregistered -eq 1) 'Potentially active task ownership was released after failure.'
    $fixtureStartFails=$false;$fixtureTaskExit=1
    try { Invoke-OrdinaryInstalledAcceptance 'ar';throw 'Failed task accepted.' } catch {
        Require ($_.Exception.Message -match 'did not produce successful evidence') 'Wrong task exit rejection.'
    }
    Require (-not $acceptanceStillRunning -and $unregistered -eq 2) 'Actually completed failed task remained owned.'
    $fixtureTaskExit=0;$fixtureOwnershipReleased=$false
    try { Invoke-OrdinaryInstalledAcceptance 'en';throw 'Active nested worker accepted.' } catch {
        Require ($_.Exception.Message -match 'nested worker ownership') 'Wrong nested-worker rejection.'
    }
    Require ($acceptanceStillRunning -and $unregistered -eq 3) 'Completed task allowed service cleanup while nested worker remained active.'
    $fixtureOwnershipReleased=$true;$restartFixture=$true;$probeCalls=0
    $AcknowledgeDisposableMachine=$true;$installed=$true;$service='AetherCoreMaintenance'
    Invoke-InstalledAcceptanceWithRestart 'en'
    Require ($probeCalls -eq 2 -and ($serviceCalls -join ',') -eq 'Stop,Stopped,Start,Running') 'Same-run service restart flow did not execute.'
    Require ($principalObserved.RunLevel -eq 'Limited') 'Verification was elevated.'
    Require ($taskCommands[-2] -notmatch 'VerifyCareRunId' -and $taskCommands[-1] -match "-VerifyCareRunId '11111111-2222-4333-8444-555555555555'") 'Actual verification task arguments lost the same Care UUID.'
    $installed=$false;$serviceCalls.Clear();$probeCalls=0
    try { Invoke-InstalledAcceptanceWithRestart 'en';throw 'Unowned service restart accepted.' } catch {
        Require ($_.Exception.Message -match 'isolated installed service') 'Wrong unowned-service rejection.'
    }
    Require ($serviceCalls.Count -eq 0 -and $probeCalls -eq 0) 'Unowned service or task touched.'
    $installed=$true;$fixtureOwnershipReleased=$false
    try { Invoke-InstalledAcceptanceWithRestart 'en';throw 'Active worker restart accepted.' } catch {
        Require ($_.Exception.Message -match 'nested worker ownership') 'Wrong active worker rejection.'
    }
    Require ($serviceCalls.Count -eq 0) 'Service restarted with an active nested worker.'
    $fixtureOwnershipReleased=$true;$fixtureOwnerChanges=$true;$probeCalls=0
    try { Invoke-InstalledAcceptanceWithRestart 'en';throw 'Different verification desktop accepted.' } catch {
        Require ($_.Exception.Message -match 'desktop owner changed') 'Wrong changed-owner rejection.'
    }
    Require ($probeCalls -eq 1) 'Changed desktop launched verification.'
    $fixtureSid='S-1-5-21-1-2-3-1001';$fixtureOwnerChanges=$false;$serviceCalls.Clear();$probeCalls=0;$AcknowledgeDisposableMachine=$false
    $savedMachineAck=$env:AETHERCORE_INSTALLER_TEST_MACHINE;$env:AETHERCORE_INSTALLER_TEST_MACHINE=''
    try {
        try { Invoke-InstalledAcceptanceWithRestart 'en';throw 'Missing disposable acknowledgement accepted.' } catch {
            Require ($_.Exception.Message -match 'isolated installed service') 'Wrong disposable rejection.'
        }
        Require ($probeCalls -eq 0 -and $serviceCalls.Count -eq 0) 'Unacknowledged host touched.'
    } finally { $env:AETHERCORE_INSTALLER_TEST_MACHINE=$savedMachineAck }
    $AcknowledgeDisposableMachine=$true;$beforeExpired=$registered
    try { Invoke-OrdinaryInstalledAcceptance 'en' -ObservationDeadline ([DateTimeOffset]::UtcNow.AddSeconds(-1));throw 'Expired observer started work.' } catch {
        Require ($_.Exception.Message -match 'budget expired before task start') 'Wrong exhausted deadline rejection.'
    }
    Require ($registered -eq $beforeExpired) 'Expired shared observer budget scheduled new work.'
    # Execute the actual encoded shell with an exit-only fixture, never a product probe.
    # A child's explicit exit belongs to the called script; the observer needs that code.
    function Observe-FixtureExit([string]$Command) {
        $start=[Diagnostics.ProcessStartInfo]::new((Get-Process -Id $PID).Path)
        $start.UseShellExecute=$false
        foreach ($argument in @('-NoProfile','-NonInteractive','-EncodedCommand',[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($Command)))) { $start.ArgumentList.Add($argument) }
        $child=[Diagnostics.Process]::Start($start)
        try {
            Require ($child.WaitForExit(20000)) 'Exit-only fixture did not settle.'
            return $child.ExitCode
        } finally { $child.Dispose() }
    }
    Set-Content $packagedScript 'exit 1'
    $actualCommand=$taskCommands[0]
    $maskedCommand=$actualCommand.Replace('exit $LASTEXITCODE','exit 0')
    Require ((Observe-FixtureExit $maskedCommand) -eq 0) 'Historical exit-masking control did not reproduce.'
    Require ((Observe-FixtureExit $actualCommand) -eq 1) 'Actual observer shell lost the probe failure/pending exit.'
    Set-Content $packagedScript 'exit 0'
    Require ((Observe-FixtureExit $actualCommand) -eq 0) 'Actual observer shell changed a successful probe exit.'
    Write-Output 'RC_HOOK_FIXTURES_PASS: actual process arguments/quoted MSI/exit codes/ownership, missing desktop, limited interactive token, active-worker failure, actual completed failure, nested ownership, same-run restart, unowned service, active-worker restart rejection, changed desktop, missing disposable acknowledgement, exhausted observer deadline.'
} finally { Remove-Item $acceptanceRoot -Recurse -Force }
