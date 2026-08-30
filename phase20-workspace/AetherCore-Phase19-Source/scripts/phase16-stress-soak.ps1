[CmdletBinding()]
param(
    [ValidateSet('quick','standard','extended')][string]$Profile = 'quick',
    [ValidateRange(0,10080)][int]$DurationMinutes = 0,
    [ValidateRange(1,4)][int]$Sessions = 4,
    [ValidateRange(100,1000000)][int]$RequestsPerProbe = 4000,
    [ValidateRange(1,1000000)][int]$ReconnectEvery = 250,
    [string]$OutputPath = 'out\ga-evidence\stress-soak.json'
)
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path;Set-Location $Root
if($env:OS -ne 'Windows_NT'){throw 'Phase 16 stress/soak qualification requires Windows.'}

$matrix=Get-Content (Join-Path $Root 'release\ga-matrix.json') -Raw | ConvertFrom-Json
$version=(Get-ItemProperty 'HKLM:\Software\AetherCore' -Name InstallVersion -ErrorAction Stop).InstallVersion
if($version -notmatch '^\d+\.\d+\.\d+$'){throw 'Installed AetherCore version is malformed.'}
if($DurationMinutes -eq 0){$DurationMinutes=@{quick=5;standard=120;extended=1440}[$Profile]}
$out=[IO.Path]::GetFullPath((Join-Path $Root $OutputPath));New-Item -ItemType Directory -Force (Split-Path $out -Parent)|Out-Null

Write-Host "Phase 16 stress/soak — profile=$Profile duration=${DurationMinutes}m" -ForegroundColor Cyan
& cargo build --locked --release -p aethercore-ga-probe
if($LASTEXITCODE -ne 0){throw 'GA IPC probe build failed.'}
$probe=Join-Path $Root 'target\release\aethercore-ga-probe.exe'
if(-not(Test-Path $probe)){throw 'GA IPC probe executable is missing.'}

function Service-Process {
    $svc=Get-CimInstance Win32_Service -Filter "Name='AetherCoreMaintenance'"
    if(-not $svc -or $svc.State -ne 'Running' -or -not $svc.ProcessId){throw 'AetherCoreMaintenance must be installed and running for soak qualification.'}
    Get-Process -Id ([int]$svc.ProcessId) -ErrorAction Stop
}
function Sample([string]$Stage){
    $p=Service-Process
    [pscustomobject]@{
        stage=$Stage;utc=(Get-Date).ToUniversalTime().ToString('o');pid=$p.Id;
        private_bytes=[int64]$p.PrivateMemorySize64;handles=[int]$p.HandleCount;threads=[int]$p.Threads.Count
    }
}
function Run-Probe([int]$Ordinal){
    $raw=& $probe --sessions $Sessions --requests-per-session $RequestsPerProbe --timeout-ms 5000 --reconnect-every $ReconnectEvery 2>&1
    if($LASTEXITCODE -ne 0){throw "GA IPC probe iteration $Ordinal failed: $raw"}
    $line=@($raw)|Where-Object{$_ -match '^\{.*\}$'}|Select-Object -Last 1
    if(-not $line){throw "GA IPC probe iteration $Ordinal produced no JSON result."}
    $result=$line|ConvertFrom-Json
    if(-not $result.ok -or $result.request_failures -ne 0){throw "GA IPC probe iteration $Ordinal reported a request failure."}
    $result
}

# Warm the service and caches before selecting the leak baseline.
$null=Run-Probe 0
Start-Sleep -Seconds 2
$baseline=Sample 'baseline-after-warmup'
$samples=New-Object System.Collections.Generic.List[object];$samples.Add($baseline)
$started=[DateTimeOffset]::UtcNow;$deadline=$started.AddMinutes($DurationMinutes);$iterations=0L;$requests=0L;$reconnects=0L;$resets=0L
while([DateTimeOffset]::UtcNow -lt $deadline){
    $iterations++
    $r=Run-Probe ([int]$iterations)
    $requests += [int64]$r.requests;$reconnects += [int64]$r.reconnects;$resets += [int64]$r.stream_resets
    $samples.Add((Sample "iteration-$iterations"))
    if(($iterations % 10)-eq 0){Write-Host "  iteration=$iterations requests=$requests" -ForegroundColor DarkCyan}
}
Start-Sleep -Seconds 2
$final=Sample 'final';$samples.Add($final)

$privateGrowth=[math]::Max(0,[int64]$final.private_bytes-[int64]$baseline.private_bytes)
$privateGrowthMiB=[math]::Round($privateGrowth/1MB,3)
$privateGrowthPct=if($baseline.private_bytes -gt 0){[math]::Round(($privateGrowth*100.0)/$baseline.private_bytes,3)}else{100.0}
$handleGrowth=[math]::Max(0,[int]$final.handles-[int]$baseline.handles)
$threadGrowth=[math]::Max(0,[int]$final.threads-[int]$baseline.threads)
$stress=$matrix.stress
$ok=($privateGrowthMiB -le [double]$stress.max_private_bytes_growth_mib) -and
    ($privateGrowthPct -le [double]$stress.max_private_bytes_growth_percent) -and
    ($handleGrowth -le [int]$stress.max_handle_growth) -and
    ($threadGrowth -le [int]$stress.max_thread_growth) -and
    ($resets -le [int]$stress.max_stream_reset_count)

$doc=[ordered]@{
    schema='aethercore.ga-stress.v1';ok=[bool]$ok;version=$version;profile=$Profile;started_utc=$started.ToString('o');finished_utc=[DateTimeOffset]::UtcNow.ToString('o');
    duration_minutes=$DurationMinutes;sessions=$Sessions;iterations=$iterations;requests=$requests;reconnects=$reconnects;stream_resets=$resets;request_failures=0;
    baseline=$baseline;final=$final;
    growth=[ordered]@{private_bytes_mib=$privateGrowthMiB;private_bytes_percent=$privateGrowthPct;handles=$handleGrowth;threads=$threadGrowth};
    thresholds=$stress;samples=$samples
}
$doc|ConvertTo-Json -Depth 8|Set-Content $out -Encoding utf8
if(-not $ok){throw "Stress/soak leak threshold failed. Evidence: $out"}
Write-Host "Phase 16 stress/soak passed. Evidence: $out" -ForegroundColor Green
