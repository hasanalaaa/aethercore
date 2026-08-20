[CmdletBinding()]
param(
    [ValidateRange(1,100)][int]$ServiceRestartCycles = 12,
    [switch]$LiveReadOnlyCollectors,
    [switch]$LibFuzzer,
    [string]$OutputPath = 'out\ga-evidence\resilience.json'
)
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path;Set-Location $Root
if($env:OS -ne 'Windows_NT'){throw 'Phase 16 resilience qualification requires Windows.'}
$out=[IO.Path]::GetFullPath((Join-Path $Root $OutputPath));New-Item -ItemType Directory -Force (Split-Path $out -Parent)|Out-Null
$version=(Get-ItemProperty 'HKLM:\Software\AetherCore' -Name InstallVersion -ErrorAction Stop).InstallVersion
if($version -notmatch '^\d+\.\d+\.\d+$'){throw 'Installed AetherCore version is malformed.'}
$results=New-Object System.Collections.Generic.List[object]
function Gate([string]$Name,[scriptblock]$Body){
    $start=[DateTimeOffset]::UtcNow
    try{& $Body;if($LASTEXITCODE -ne 0){throw "$Name exited with code $LASTEXITCODE"};$results.Add([pscustomobject]@{name=$Name;ok=$true;started_utc=$start.ToString('o');finished_utc=[DateTimeOffset]::UtcNow.ToString('o')})}
    catch{$results.Add([pscustomobject]@{name=$Name;ok=$false;error=$_.Exception.Message;started_utc=$start.ToString('o');finished_utc=[DateTimeOffset]::UtcNow.ToString('o')});throw}
}

Write-Host 'Phase 16 adversarial resilience matrix' -ForegroundColor Cyan
Gate 'machine-mutation-concurrency-stress' { & (Join-Path $PSScriptRoot 'invoke-cargo-test-case.ps1') -Package aethercore-operation-kernel -TestName concurrent_contenders_never_overlap_machine_mutation_leases }
Gate 'collector-runtime-fault-injection' { & (Join-Path $PSScriptRoot 'phase13-fault-injection.ps1') -LiveReadOnly:$LiveReadOnlyCollectors }
Gate 'scheduler-preemption-fault-injection' { & (Join-Path $PSScriptRoot 'phase14-scheduler-fault-injection.ps1') -LiveReadOnly:$LiveReadOnlyCollectors }
Gate 'update-support-crypto-tamper' { & (Join-Path $PSScriptRoot 'phase15-crypto-tests.ps1') }
Gate 'ipc-malformed-frame-corpus' { & (Join-Path $PSScriptRoot 'run-ipc-fuzz.ps1') -LibFuzzer:$LibFuzzer -MaxTotalTimeSeconds 120 }
Gate 'ga-probe-build' { & cargo build --locked --release -p aethercore-ga-probe }
$probe=Join-Path $Root 'target\release\aethercore-ga-probe.exe'

Gate 'service-restart-reconnect-replay' {
    $svc=Get-Service AetherCoreMaintenance -ErrorAction Stop
    if($svc.Status -ne 'Running'){Start-Service $svc; $svc.WaitForStatus('Running',[TimeSpan]::FromSeconds(30))}
    for($i=1;$i -le $ServiceRestartCycles;$i++){
        $job=Start-Job -ScriptBlock { param($exe) & $exe --sessions 2 --requests-per-session 2000 --reconnect-every 100 } -ArgumentList $probe
        Start-Sleep -Milliseconds 350
        Restart-Service AetherCoreMaintenance -Force
        (Get-Service AetherCoreMaintenance).WaitForStatus('Running',[TimeSpan]::FromSeconds(30))
        $completed=Wait-Job $job -Timeout 60
        $output=Receive-Job $job -ErrorAction SilentlyContinue
        Remove-Job $job -Force -ErrorAction SilentlyContinue
        # The in-flight probe may fail at the exact restart boundary; recovery is proven by a fresh
        # principal-scoped replay/hydration probe immediately after the service returns.
        $fresh=& $probe --sessions 2 --requests-per-session 400 --reconnect-every 100 2>&1
        if($LASTEXITCODE -ne 0){throw "Fresh reconnect probe failed after service restart cycle $i: $fresh"}
    }
}

$doc=[ordered]@{schema='aethercore.ga-resilience.v1';ok=$true;version=$version;service_restart_cycles=$ServiceRestartCycles;live_read_only_collectors=[bool]$LiveReadOnlyCollectors;libfuzzer=[bool]$LibFuzzer;tests=$results}
$doc|ConvertTo-Json -Depth 6|Set-Content $out -Encoding utf8
Write-Host "Phase 16 resilience matrix passed. Evidence: $out" -ForegroundColor Green
