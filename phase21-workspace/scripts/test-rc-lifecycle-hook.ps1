#Requires -Version 7.0
# Execute the actual ordinary-user hook with task/ACL cmdlets replaced by local fixtures.
# No task, installation, signer or existing service is invoked.
$ErrorActionPreference='Stop'
$source=Join-Path $PSScriptRoot 'phase16-installer-lifecycle.ps1'
$tokens=$null;$errors=$null
$ast=[Management.Automation.Language.Parser]::ParseFile($source,[ref]$tokens,[ref]$errors)
if ($errors.Count) { throw 'Lifecycle source did not parse.' }
foreach ($name in @('Write-BlockedAcceptance','Invoke-OrdinaryInstalledAcceptance')) {
    $definition=$ast.Find({param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq $name},$true)
    if (-not $definition) { throw "Actual hook missing: $name" }
    . ([scriptblock]::Create($definition.Extent.Text))
}
$acceptanceRoot=Join-Path ([IO.Path]::GetTempPath()) ('aethercore-rc-hook-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $acceptanceRoot | Out-Null
$ExpectedSourceSha='a'*40;$ExpectedBundleSha256='b'*64
$release=$acceptanceRoot;$packagedScript=Join-Path $acceptanceRoot 'fixture-probe.ps1'
Set-Content $packagedScript '# temporary fixture; never executed'
$fixtureOwners=@();$fixtureStartFails=$false;$fixtureTaskExit=0;$fixtureOwnershipReleased=$true
$registered=0;$unregistered=0;$principalObserved=$null
function Get-CimInstance { param($Filter) @($fixtureOwners) }
function Invoke-CimMethod { param($InputObject,$MethodName) @{ReturnValue=0;Sid='S-1-5-21-1-2-3-1001'} }
function Get-Acl { param($Path) $value=[pscustomobject]@{};$value|Add-Member ScriptMethod SetAccessRule {param($rule)};return $value }
function Set-Acl { param($Path,$AclObject) }
function New-ScheduledTaskAction { param($Execute,$Argument,$WorkingDirectory) @{Execute=$Execute;Argument=$Argument} }
function New-ScheduledTaskPrincipal { param($UserId,$LogonType,$RunLevel) $script:principalObserved=@{SID=$UserId;LogonType=$LogonType;RunLevel=$RunLevel};return $principalObserved }
function New-ScheduledTaskSettingsSet { param($ExecutionTimeLimit) if ($ExecutionTimeLimit -ne [TimeSpan]::Zero) { throw 'Worker would be force-stopped.' };@{} }
function Register-ScheduledTask { param($TaskName,$Action,$Principal,$Settings,$ErrorAction) $script:registered++ }
function Start-ScheduledTask { param($TaskName,$ErrorAction) if ($fixtureStartFails) { throw 'controlled start/status failure' };@{worker_ownership_released=$fixtureOwnershipReleased}|ConvertTo-Json|Set-Content $output }
function Get-ScheduledTask { param($TaskName,$ErrorAction) @{State='Ready'} }
function Get-ScheduledTaskInfo { param($TaskName,$ErrorAction) @{LastRunTime=(Get-Date);LastTaskResult=$fixtureTaskExit} }
function Unregister-ScheduledTask { param($TaskName,[switch]$Confirm,$ErrorAction) $script:unregistered++ }
function Require([bool]$Value,[string]$Reason) { if (-not $Value) { throw $Reason } }
try {
    $acceptanceStillRunning=$false
    try { Invoke-OrdinaryInstalledAcceptance 'en';throw 'No desktop was accepted.' } catch {
        Require ($_.Exception.Message -match 'no unique explorer owner') 'Wrong rejection for absent desktop.'
    }
    $blocked=Get-Content (Join-Path $acceptanceRoot 'installed-en.json') -Raw | ConvertFrom-Json
    Require ($blocked.disposition -eq 'blocked' -and -not $blocked.ordinary_user -and $registered -eq 0) 'Absent desktop created a task or passed.'
    $fixtureOwners=@([pscustomobject]@{Name='explorer.exe'})
    Invoke-OrdinaryInstalledAcceptance 'ar'
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
    Write-Output 'RC_HOOK_FIXTURES_PASS: missing desktop, limited interactive token, active-worker failure, actual completed failure.'
} finally { Remove-Item $acceptanceRoot -Recurse -Force }
