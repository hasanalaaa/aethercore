[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$ReleaseRoot,
    [string]$Lane,
    [string]$WitnessPath,
    [ValidateSet('quick','standard','extended')][string]$StressProfile='extended',
    [switch]$LiveReadOnlyCollectors,
    [switch]$LibFuzzer,
    [switch]$AcknowledgeDisposableMachine,
    [string]$OutputDirectory='out\sigma-master-native'
)
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $Root
if($env:OS -ne 'Windows_NT'){throw 'Sigma Master qualification requires x64 Windows.'}
if(-not [Environment]::Is64BitOperatingSystem){throw 'Sigma Master qualification requires x64 Windows.'}
if(-not $AcknowledgeDisposableMachine -and $env:AETHERCORE_INSTALLER_TEST_MACHINE -ne '1'){
    throw 'Sigma Master qualification changes installed state. Use a disposable qualification VM and acknowledge it.'
}
$identity=[Security.Principal.WindowsIdentity]::GetCurrent()
$principal=[Security.Principal.WindowsPrincipal]::new($identity)
if(-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){
    throw 'Sigma Master qualification must run elevated for service/installer/native security checks.'
}

$OutputDirectory=$OutputDirectory -replace '/','\\'
if([IO.Path]::IsPathRooted($OutputDirectory) -or $OutputDirectory -match '(^|\\)\.\.(\\|$)'){throw 'OutputDirectory must be a source-relative external evidence directory such as out\sigma-master-native.'}
$Out=[IO.Path]::GetFullPath((Join-Path $Root $OutputDirectory))
New-Item -ItemType Directory -Force $Out|Out-Null
$steps=New-Object System.Collections.Generic.List[object]
function Gate([string]$Name,[scriptblock]$Body){
    $started=[DateTimeOffset]::UtcNow
    try{
        & $Body
        if($LASTEXITCODE -and $LASTEXITCODE -ne 0){throw "$Name exited with code $LASTEXITCODE"}
        $steps.Add([pscustomobject]@{name=$Name;ok=$true;started_utc=$started.ToString('o');finished_utc=[DateTimeOffset]::UtcNow.ToString('o')})
    }catch{
        $steps.Add([pscustomobject]@{name=$Name;ok=$false;error=$_.Exception.Message;started_utc=$started.ToString('o');finished_utc=[DateTimeOffset]::UtcNow.ToString('o')})
        throw
    }
}
function Require-Command([string]$Name){
    $cmd=Get-Command $Name -ErrorAction SilentlyContinue
    if(-not $cmd){throw "Required command is unavailable: $Name"}
    return $cmd.Source
}

Gate 'toolchain-and-freeze' {
    Require-Command cargo|Out-Null;Require-Command rustc|Out-Null;Require-Command pnpm|Out-Null;Require-Command python|Out-Null
    & (Join-Path $PSScriptRoot 'freeze-dependencies.ps1') -VerifyOnly
    if($LASTEXITCODE -ne 0){throw 'Approved dependency freeze is not valid.'}
}
Gate 'format-check-build-test-clippy' {
    & cargo fmt --all -- --check;if($LASTEXITCODE -ne 0){throw 'cargo fmt failed.'}
    & cargo check --workspace --all-targets --locked;if($LASTEXITCODE -ne 0){throw 'cargo check failed.'}
    & cargo test --workspace --all-targets --locked;if($LASTEXITCODE -ne 0){throw 'cargo test failed.'}
    & cargo clippy --workspace --all-targets --locked -- -D warnings;if($LASTEXITCODE -ne 0){throw 'cargo clippy failed.'}
    & pnpm --dir apps/ui install --frozen-lockfile;if($LASTEXITCODE -ne 0){throw 'pnpm frozen install failed.'}
    & pnpm --dir apps/ui check;if($LASTEXITCODE -ne 0){throw 'Svelte/TypeScript check failed.'}
    & pnpm --dir apps/ui build;if($LASTEXITCODE -ne 0){throw 'UI build failed.'}
}
Gate 'source-evidence-integrity' {
    & python scripts/sigma-evidence-integrity-test.py --json (Join-Path $Out 'sigma-evidence-integrity.json')
    if($LASTEXITCODE -ne 0){throw 'Sigma evidence-integrity regression failed.'}
    & python scripts/sigma-master-full-app-ui.py --json (Join-Path $Out 'full-app-ui.json')
    if($LASTEXITCODE -ne 0){throw 'Actual Svelte application browser suite failed.'}
}
Gate 'phase16-source-native-master-gate' {
    & (Join-Path $PSScriptRoot 'verify-phase16.ps1') -LiveReadOnlyFaultInjection -LiveReadOnlySchedulerProbe -UserShellPrivilegeCheck -LibFuzzer:$LibFuzzer
    if($LASTEXITCODE -ne 0){throw 'Phase 16 inherited source/native gate failed.'}
}
Gate 'ipc-pipe-security' {
    & (Join-Path $PSScriptRoot 'verify-ipc-pipe-security.ps1') -OutputPath (Join-Path $OutputDirectory 'ipc-pipe-security.json')
    if($LASTEXITCODE -ne 0){throw 'Native pipe security check failed.'}
}
Gate 'resilience-and-teardown' {
    & (Join-Path $PSScriptRoot 'phase16-resilience-matrix.ps1') -LiveReadOnlyCollectors:$LiveReadOnlyCollectors -LibFuzzer:$LibFuzzer -OutputPath (Join-Path $OutputDirectory 'resilience.json')
    if($LASTEXITCODE -ne 0){throw 'Native resilience matrix failed.'}
    $pipeStatus=& python scripts/check-pipe-teardown-qualification.py --evidence (Join-Path $Root 'out\omega-pipe-teardown.json') --json
    $pipeExit=$LASTEXITCODE
    $pipeStatus|Set-Content (Join-Path $Out 'pipe-teardown-status.json') -Encoding utf8
    if($pipeExit -ne 0){throw 'Named-pipe teardown qualification remains blocked; provide out\omega-pipe-teardown.json from the approved native teardown campaign.'}
}
Gate 'stress-soak' {
    & (Join-Path $PSScriptRoot 'phase16-stress-soak.ps1') -Profile $StressProfile -OutputPath (Join-Path $OutputDirectory 'stress-soak.json')
    if($LASTEXITCODE -ne 0){throw 'Stress/soak qualification failed.'}
}
Gate 'installer-lifecycle' {
    & (Join-Path $PSScriptRoot 'phase16-installer-lifecycle.ps1') -ReleaseRoot $ReleaseRoot -AcknowledgeDisposableMachine -OutputPath (Join-Path $OutputDirectory 'installer-lifecycle.json')
    if($LASTEXITCODE -ne 0){throw 'Installer lifecycle qualification failed.'}
}
if($Lane -or $WitnessPath){
    if(-not $Lane -or -not $WitnessPath){throw 'Lane and WitnessPath must be supplied together.'}
    Gate 'host-witness-accessibility-rtl' {
        & (Join-Path $PSScriptRoot 'phase16-host-qualification.ps1') -Lane $Lane -WitnessPath $WitnessPath -ReleaseRoot $ReleaseRoot -StressProfile none -OutputPath (Join-Path $OutputDirectory 'host-witness.json')
        if($LASTEXITCODE -ne 0){throw 'Host witness qualification failed.'}
    }
}else{
    throw 'Final qualification requires both -Lane and -WitnessPath so accessibility, DPI, RTL and manual native surfaces cannot be silently skipped.'
}

$doc=[ordered]@{
    schema='aethercore.sigma-master-native-qualification-evidence.v1'
    ok=$true
    platform='Windows'
    architecture=$env:PROCESSOR_ARCHITECTURE
    windows_build=[Environment]::OSVersion.Version.Build
    release_root=(Resolve-Path $ReleaseRoot).Path
    lane=$Lane
    witness=(Resolve-Path $WitnessPath).Path
    completed_utc=[DateTimeOffset]::UtcNow.ToString('o')
    steps=$steps
}
$doc|ConvertTo-Json -Depth 7|Set-Content (Join-Path $Out 'sigma-master-native-qualification.json') -Encoding utf8
Write-Host "Sigma Master Windows production qualification passed. Evidence: $Out" -ForegroundColor Green
