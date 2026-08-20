[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$ReleaseRoot,
    [switch]$AcknowledgeDisposableMachine,
    [string]$OutputPath = 'out\ga-evidence\installer-lifecycle.json'
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
$service='AetherCoreMaintenance';$installDir=Join-Path $env:ProgramW6432 'AetherCore';$dataDir=Join-Path $env:ProgramData 'AetherCore'
if(Get-Service $service -ErrorAction SilentlyContinue){throw 'Refusing GA lifecycle: AetherCore service already exists.'}
if(Test-Path $installDir){throw 'Refusing GA lifecycle: AetherCore install directory already exists.'}
$out=[IO.Path]::GetFullPath((Join-Path $Root $OutputPath));New-Item -ItemType Directory -Force (Split-Path $out -Parent)|Out-Null
$steps=New-Object System.Collections.Generic.List[object]
function Run-Step([string]$Name,[scriptblock]$Body){$t=[DateTimeOffset]::UtcNow;try{&$Body;$steps.Add([pscustomobject]@{name=$Name;ok=$true;started_utc=$t.ToString('o');finished_utc=[DateTimeOffset]::UtcNow.ToString('o')})}catch{$steps.Add([pscustomobject]@{name=$Name;ok=$false;error=$_.Exception.Message;started_utc=$t.ToString('o');finished_utc=[DateTimeOffset]::UtcNow.ToString('o')});throw}}
function Run-Process([string]$File,[string[]]$Args,[string]$Label){$p=Start-Process $File -ArgumentList $Args -PassThru -Wait;if($p.ExitCode -notin @(0,3010)){throw "$Label failed with exit code $($p.ExitCode)."}}
$sentinel=Join-Path $dataDir 'phase16-ga-preserve.sentinel';$installed=$false
try{
    Run-Step 'burn-install' { Run-Process $bundle @('/install','/quiet','/norestart') 'Burn install';$script:installed=$true }
    Run-Step 'installed-security-boundaries' { & (Join-Path $PSScriptRoot 'verify-installer-security.ps1') -MsiPath $msi -VerifyInstalledStateOnly -RequireSignedArtifacts;if($LASTEXITCODE -ne 0){throw 'Installed state verifier failed.'} }
    Run-Step 'program-data-preservation-sentinel' { 'phase16-preserve'|Set-Content $sentinel -Encoding ascii }
    Run-Step 'repair-closes-acl-drift' {
        & "$env:SystemRoot\System32\icacls.exe" $installDir /grant '*S-1-1-0:(OI)(CI)M' /Q|Out-Null
        if($LASTEXITCODE -ne 0){throw 'Unable to inject ACL drift.'}
        Run-Process msiexec.exe @('/fa',"`"$msi`"",'/qn','/norestart') 'MSI repair'
        & (Join-Path $PSScriptRoot 'verify-installer-security.ps1') -MsiPath $msi -VerifyInstalledStateOnly -RequireSignedArtifacts
        if($LASTEXITCODE -ne 0){throw 'Post-repair state verifier failed.'}
    }
    Run-Step 'burn-uninstall' { Run-Process $bundle @('/uninstall','/quiet','/norestart') 'Burn uninstall';$script:installed=$false }
    Run-Step 'uninstall-clean-state' {
        if(Get-Service $service -ErrorAction SilentlyContinue){throw 'Service remains after Burn uninstall.'}
        if(Test-Path (Join-Path $installDir 'aethercore-desktop.exe')){throw 'Desktop binary remains after Burn uninstall.'}
        if(-not(Test-Path $sentinel)){throw 'ProgramData preservation sentinel was removed.'}
        Remove-Item $sentinel -Force
    }
} finally {
    if($installed){try{Run-Process $bundle @('/uninstall','/quiet','/norestart') 'cleanup Burn uninstall'}catch{Write-Warning $_}}
}
$doc=[ordered]@{schema='aethercore.ga-installer-lifecycle.v1';ok=$true;version=$version;windows_build=[Environment]::OSVersion.Version.Build;architecture=$env:PROCESSOR_ARCHITECTURE;executed_utc=[DateTimeOffset]::UtcNow.ToString('o');steps=$steps}
$doc|ConvertTo-Json -Depth 6|Set-Content $out -Encoding utf8
Write-Host "Phase 16 Burn/MSI lifecycle passed. Evidence: $out" -ForegroundColor Green
