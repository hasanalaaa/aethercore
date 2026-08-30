[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$ReleaseRoot,[switch]$SkipCertificateChainCheck)
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT'){throw 'GA seal verification requires Windows.'}
$release=(Resolve-Path $ReleaseRoot).Path;$sealPath=Join-Path $release 'GA-SEAL.json';$sigPath=Join-Path $release 'GA-SEAL.p7s';$evidenceManifest=Join-Path $release 'GA-EVIDENCE-SHA256SUMS.txt'
foreach($f in @($sealPath,$sigPath,$evidenceManifest)){if(-not(Test-Path $f)){throw "GA seal artifact missing: $f"}}
$seal=Get-Content $sealPath -Raw|ConvertFrom-Json;if($seal.schema -ne 'aethercore.ga-seal.v1' -or -not $seal.ga){throw 'Invalid GA seal document.'}
if((Get-FileHash (Join-Path $release 'SHA256SUMS.txt') -Algorithm SHA256).Hash.ToLowerInvariant() -ne $seal.release_sha256s_sha256){throw 'GA seal release hash commitment mismatch.'}
if((Get-FileHash $evidenceManifest -Algorithm SHA256).Hash.ToLowerInvariant() -ne $seal.evidence_manifest_sha256){throw 'GA seal evidence hash commitment mismatch.'}
Add-Type -AssemblyName System.Security.Cryptography.Pkcs
$content=[IO.File]::ReadAllBytes($sealPath);$cms=[System.Security.Cryptography.Pkcs.SignedCms]::new([System.Security.Cryptography.Pkcs.ContentInfo]::new($content),$true);$cms.Decode([IO.File]::ReadAllBytes($sigPath));$cms.CheckSignature([bool]$SkipCertificateChainCheck)
$signer=$cms.SignerInfos[0].Certificate;if(-not $signer -or $signer.Thumbprint -ne $seal.signer_thumbprint){throw 'GA seal signer thumbprint mismatch.'}
Write-Host "GA seal verified: AetherCore $($seal.version)" -ForegroundColor Green
