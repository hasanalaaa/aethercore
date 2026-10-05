<#
  DBT-P55-007: verifies, after `dotnet tool restore`, that each pinned tool package carries a signature
  from an author certificate that nuget.config names as trusted.

  Why it exists: `signatureValidationMode=require` in nuget.config is enforced by some SDKs and not by
  others. A cold restore with corrupted `trustedSigners` fingerprints still exited 0 on a developer
  machine (SDK 8.0.424), so the control the project believed it had did nothing there. This does not
  depend on how the restoring SDK behaves: it asks NuGet to verify the restored package itself.

  Measured on the Windows PC (SDK 8.0.425), wix 6.0.2: `dotnet nuget verify <nupkg> --certificate-fingerprint`
  matches the AUTHOR signature only. The repository countersignature fingerprint (nuget.org) fails it with
  NU3034, so only `<author>` certificates are used here. Several fingerprints mean any of them.

  Fails closed: a config naming no author signer, a package that was not restored, or a signature that
  matches none of the fingerprints all throw. Compatible with Windows PowerShell 5.1.

  -ManifestPath and -NuGetConfigPath exist so the test can feed it a corrupted copy.
#>
[CmdletBinding()]
param(
  [string]$ManifestPath = (Join-Path $PSScriptRoot '../.config/dotnet-tools.json'),
  [string]$NuGetConfigPath = (Join-Path $PSScriptRoot '../nuget.config')
)
$ErrorActionPreference = 'Stop'

[xml]$config = Get-Content -LiteralPath $NuGetConfigPath -Raw
$fingerprints = @()
foreach ($author in @($config.configuration.trustedSigners.author)) {
  foreach ($certificate in @($author.certificate)) {
    if ($certificate.fingerprint) { $fingerprints += [string]$certificate.fingerprint }
  }
}
if ($fingerprints.Count -eq 0) {
  throw "$NuGetConfigPath names no author trusted signer, so no tool signature can be verified."
}

$line = & dotnet nuget locals global-packages --list | Where-Object { $_ -match 'global-packages:' } | Select-Object -First 1
if (-not $line) { throw 'Could not resolve the NuGet global packages folder.' }
$packages = ($line -replace '^.*global-packages:\s*', '').Trim()

$tools = (Get-Content -LiteralPath $ManifestPath -Raw | ConvertFrom-Json).tools
foreach ($property in $tools.PSObject.Properties) {
  $id = $property.Name.ToLowerInvariant()
  $version = ([string]$property.Value.version).ToLowerInvariant()
  $nupkg = Join-Path (Join-Path (Join-Path $packages $id) $version) "$id.$version.nupkg"
  if (-not (Test-Path -LiteralPath $nupkg)) {
    throw "Pinned tool $id $version is not in the restored packages ($nupkg); nothing to verify."
  }
  $verifyArgs = @('nuget', 'verify', $nupkg)
  foreach ($fingerprint in $fingerprints) { $verifyArgs += '--certificate-fingerprint'; $verifyArgs += $fingerprint }
  & dotnet @verifyArgs
  if ($LASTEXITCODE -ne 0) {
    throw "Pinned tool $id $version does not carry a signature from a trusted author certificate (dotnet nuget verify exit $LASTEXITCODE)."
  }
  Write-Host "Verified $id $version against $($fingerprints.Count) trusted author fingerprint(s)."
}
