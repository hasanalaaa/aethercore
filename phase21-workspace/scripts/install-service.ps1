#Requires -RunAsAdministrator
[CmdletBinding()]
param([ValidateSet('Debug','Release')][string]$Configuration='Release')
$ErrorActionPreference='Stop'
$Root=Split-Path $PSScriptRoot -Parent
Set-Location $Root
$profile = if ($Configuration -eq 'Release') {'release'} else {'debug'}
$cargoBuildArgs = @('build', '-p', 'aethercore-maintenance-service', '-p', 'aethercore-consent-broker')
if ($Configuration -eq 'Release') { $cargoBuildArgs += '--release' }

& cargo @cargoBuildArgs
if ($LASTEXITCODE -ne 0) { throw 'Build failed' }

$serviceName = 'AetherCoreMaintenance'
$servicePrincipal = "NT SERVICE\$serviceName"
$binDir = Join-Path $env:ProgramFiles 'AetherCore'
$productDataDir = Join-Path $env:ProgramData 'AetherCore'
$dataDir = Join-Path $productDataDir 'state'
$mutationLock = Join-Path $dataDir 'machine-mutation.lock'
New-Item -ItemType Directory -Force $binDir,$dataDir | Out-Null
$lockStream = [System.IO.File]::Open($mutationLock,[System.IO.FileMode]::OpenOrCreate,[System.IO.FileAccess]::ReadWrite,[System.IO.FileShare]::ReadWrite -bor [System.IO.FileShare]::Delete)
$lockStream.Dispose()

$existing = Get-Service $serviceName -ErrorAction SilentlyContinue
if ($existing) {
    if ($existing.Status -ne 'Stopped') { Stop-Service $serviceName -Force -ErrorAction SilentlyContinue }
    & sc.exe delete $serviceName | Out-Null
    Start-Sleep -Seconds 1
}

Copy-Item "target\$profile\aethercore-maintenance-service.exe" $binDir -Force
Copy-Item "target\$profile\aethercore-consent-broker.exe" $binDir -Force

$exe = Join-Path $binDir 'aethercore-maintenance-service.exe'
& sc.exe create $serviceName binPath= ('"'+$exe+'"') start= delayed-auto obj= LocalSystem DisplayName= 'AetherCore Maintenance Service' | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Service creation failed' }
& sc.exe sidtype $serviceName unrestricted | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Unable to configure unrestricted service SID' }
& sc.exe description $serviceName 'Privileged typed-operation engine for AetherCore.' | Out-Null

# The service-specific SID remains an ACL identity for AetherCore-owned resources. It is
# deliberately UNRESTRICTED because this maintenance service must mutate Windows and third-party
# resources whose ACLs cannot be rewritten safely to include the AetherCore service SID.
& icacls $binDir /inheritance:r /grant:r `
    '*S-1-5-18:(OI)(CI)F' `
    '*S-1-5-32-544:(OI)(CI)F' `
    '*S-1-5-32-545:(OI)(CI)RX' `
    "${servicePrincipal}:(OI)(CI)RX" | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Binary directory ACL hardening failed' }

& icacls $productDataDir /inheritance:r /grant:r `
    '*S-1-5-18:(OI)(CI)F' `
    '*S-1-5-32-544:(OI)(CI)F' `
    "${servicePrincipal}:(OI)(CI)F" | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Data directory ACL hardening failed' }

& icacls $mutationLock /inheritance:r /grant:r `
    '*S-1-5-18:F' `
    '*S-1-5-32-544:F' `
    "${servicePrincipal}:F" | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Machine mutation lock ACL hardening failed' }

Start-Service $serviceName
Write-Host "$serviceName installed with an unrestricted service SID and started." -ForegroundColor Green
