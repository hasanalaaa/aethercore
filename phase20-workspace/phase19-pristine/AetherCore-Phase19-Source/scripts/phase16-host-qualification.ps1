[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$Lane,
    [Parameter(Mandatory=$true)][string]$WitnessPath,
    [Parameter(Mandatory=$true)][string]$ReleaseRoot,
    [ValidateSet('none','quick','standard','extended')][string]$StressProfile='none',
    [switch]$RunResilience,
    [switch]$LiveReadOnlyCollectors,
    [switch]$LibFuzzer,
    [string]$OutputPath
)
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path;Set-Location $Root
if($env:OS -ne 'Windows_NT'){throw 'Phase 16 host qualification requires Windows.'}
$matrix=Get-Content (Join-Path $Root 'release\ga-matrix.json') -Raw|ConvertFrom-Json
$laneSpec=@($matrix.required_os_lanes|Where-Object{$_.id -eq $Lane})
if($laneSpec.Count -ne 1){throw "Unknown GA lane: $Lane"}
$build=[Environment]::OSVersion.Version.Build
if($build -lt [int]$laneSpec[0].build_min -or $build -gt [int]$laneSpec[0].build_max){throw "Windows build $build is outside lane $Lane ($($laneSpec[0].build_min)-$($laneSpec[0].build_max))."}
if(-not [Environment]::Is64BitOperatingSystem){throw 'GA host must be x64 Windows.'}
$witness=Get-Content (Resolve-Path $WitnessPath) -Raw|ConvertFrom-Json
if($witness.schema -ne 'aethercore.ga-witness.v1'){throw 'Unsupported GA witness schema.'}
if([string]::IsNullOrWhiteSpace($witness.operator) -or [string]::IsNullOrWhiteSpace($witness.executed_utc)){throw 'GA witness must identify the operator and execution time.'}
if($witness.locale -notin @($matrix.required_coverage.locales)){throw "Witness locale is outside GA matrix: $($witness.locale)"}
if($witness.direction -notin @($matrix.required_coverage.directions)){throw "Witness direction is outside GA matrix: $($witness.direction)"}
if([int]$witness.dpi_percent -notin @($matrix.required_coverage.dpi_percent|ForEach-Object{[int]$_})){throw "Witness DPI is outside GA matrix: $($witness.dpi_percent)"}
if([int]$witness.refresh_hz -notin @($matrix.required_coverage.refresh_hz|ForEach-Object{[int]$_})){throw "Witness refresh rate is outside GA matrix: $($witness.refresh_hz)"}
foreach($surface in $matrix.required_manual_surfaces){$v=$witness.surfaces.PSObject.Properties[$surface];if(-not $v -or $v.Value -ne 'pass'){throw "Witness surface is not PASS: $surface"}}
foreach($prop in $witness.checks.PSObject.Properties){if($prop.Value -ne 'pass'){throw "Witness check is not PASS: $($prop.Name)"}}
$release=(Resolve-Path $ReleaseRoot).Path;$meta=Get-Content (Join-Path $release 'RELEASE-METADATA.json') -Raw|ConvertFrom-Json;$version=$meta.version;$msi=Join-Path $release "artifacts\AetherCore-$version-x64.msi"
if($witness.version -ne $version){throw "Witness version $($witness.version) does not match release $version."}
$installedVersion=(Get-ItemProperty 'HKLM:\Software\AetherCore' -Name InstallVersion -ErrorAction Stop).InstallVersion
if($installedVersion -ne $version){throw "Installed version $installedVersion does not match release $version."}
& (Join-Path $PSScriptRoot 'verify-installer-security.ps1') -MsiPath $msi -VerifyInstalledStateOnly -RequireSignedArtifacts
if($LASTEXITCODE -ne 0){throw 'Installed security/elevation verification failed.'}

$stressPath=$null;$resiliencePath=$null
if($StressProfile -ne 'none'){$stressPath="out\ga-evidence\stress-$Lane-$($witness.locale)-$($witness.dpi_percent).json";& (Join-Path $PSScriptRoot 'phase16-stress-soak.ps1') -Profile $StressProfile -OutputPath $stressPath;if($LASTEXITCODE -ne 0){throw 'Host stress qualification failed.'}}
if($RunResilience){$resiliencePath="out\ga-evidence\resilience-$Lane.json";& (Join-Path $PSScriptRoot 'phase16-resilience-matrix.ps1') -LiveReadOnlyCollectors:$LiveReadOnlyCollectors -LibFuzzer:$LibFuzzer -OutputPath $resiliencePath;if($LASTEXITCODE -ne 0){throw 'Host resilience qualification failed.'}}

if(-not $OutputPath){$OutputPath="out\ga-evidence\host-$Lane-$($witness.locale)-$($witness.dpi_percent)-$($witness.refresh_hz).json"}
$out=[IO.Path]::GetFullPath((Join-Path $Root $OutputPath));New-Item -ItemType Directory -Force (Split-Path $out -Parent)|Out-Null
$ubr=(Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion' -Name UBR -ErrorAction SilentlyContinue).UBR
$doc=[ordered]@{
    schema='aethercore.ga-host.v1';ok=$true;lane=$Lane;version=$version;windows=[ordered]@{build=$build;ubr=$ubr;architecture=$env:PROCESSOR_ARCHITECTURE};
    witness_sha256=(Get-FileHash (Resolve-Path $WitnessPath) -Algorithm SHA256).Hash.ToLowerInvariant();witness_operator_present=$true;executed_utc=$witness.executed_utc;
    locale=$witness.locale;direction=$witness.direction;dpi_percent=[int]$witness.dpi_percent;refresh_hz=[int]$witness.refresh_hz;
    accessibility=@($witness.accessibility);input=@($witness.input);update_channels=@($witness.update_channels);manual_surfaces=@($matrix.required_manual_surfaces);
    stress_evidence=$stressPath;resilience_evidence=$resiliencePath
}
$doc|ConvertTo-Json -Depth 7|Set-Content $out -Encoding utf8
Write-Host "Phase 16 host qualification passed. Evidence: $out" -ForegroundColor Green
