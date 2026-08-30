[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$ReleaseRoot,
    [string]$EvidenceDirectory = 'out\ga-evidence',
    [string]$CertificateThumbprint = $env:AETHERCORE_CODESIGN_THUMBPRINT,
    [ValidateSet('CurrentUser','LocalMachine')][string]$CertificateStore = 'CurrentUser',
    [switch]$SkipCertificateChainCheck
)
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path;Set-Location $Root
if($env:OS -ne 'Windows_NT'){throw 'GA sealing requires Windows.'}
if([string]::IsNullOrWhiteSpace($CertificateThumbprint)){throw 'GA sealing requires the protected code-signing certificate thumbprint.'}
$release=(Resolve-Path $ReleaseRoot).Path;$evidence=(Resolve-Path $EvidenceDirectory).Path
$matrix=Get-Content (Join-Path $Root 'release\ga-matrix.json') -Raw|ConvertFrom-Json
$metaPath=Join-Path $release 'RELEASE-METADATA.json';$meta=Get-Content $metaPath -Raw|ConvertFrom-Json
$version=$meta.version
if(-not $meta.signing_required){throw 'GA seal refuses an unsigned release candidate.'}

function Read-JsonEvidence([string]$Schema){
    @(Get-ChildItem $evidence -Filter '*.json' -File|ForEach-Object{try{$j=Get-Content $_.FullName -Raw|ConvertFrom-Json;if($j.schema -eq $Schema){[pscustomobject]@{Path=$_.FullName;Doc=$j}}}catch{}})
}
function Require-Set([string]$Name,[object[]]$Required,[object[]]$Actual){
    $actualText=@($Actual|ForEach-Object{"$_"}|Select-Object -Unique)
    $missing=@($Required|ForEach-Object{"$_"}|Where-Object{$_ -notin $actualText})
    if($missing.Count){throw "GA coverage missing $Name: $($missing -join ', ')"}
}
function Verify-ShaFile([string]$Path){
    foreach($line in Get-Content $Path){if($line -notmatch '^([0-9a-fA-F]{64})  (.+)$'){throw "Malformed SHA256SUMS line: $line"};$expected=$Matches[1].ToLowerInvariant();$rel=$Matches[2] -replace '/','\\';$file=Join-Path $release $rel;if(-not(Test-Path $file)){throw "Release hash target missing: $rel"};$actual=(Get-FileHash $file -Algorithm SHA256).Hash.ToLowerInvariant();if($actual -ne $expected){throw "Release hash mismatch: $rel"}}
}

$hosts=Read-JsonEvidence 'aethercore.ga-host.v1';if($hosts.Count -lt $matrix.required_os_lanes.Count){throw 'Insufficient GA host evidence.'};foreach($h in $hosts){if($h.Doc.version -ne $version){throw "Host evidence version mismatch: $($h.Path)"}}
foreach($lane in $matrix.required_os_lanes){if(-not($hosts|Where-Object{$_.Doc.ok -and $_.Doc.lane -eq $lane.id})){throw "Missing PASS evidence for OS lane: $($lane.id)"}}
Require-Set 'locales' $matrix.required_coverage.locales @($hosts.Doc.locale)
Require-Set 'directions' $matrix.required_coverage.directions @($hosts.Doc.direction)
Require-Set 'DPI' $matrix.required_coverage.dpi_percent @($hosts.Doc.dpi_percent)
Require-Set 'refresh rates' $matrix.required_coverage.refresh_hz @($hosts.Doc.refresh_hz)
Require-Set 'accessibility' $matrix.required_coverage.accessibility @($hosts.Doc.accessibility|ForEach-Object{$_})
Require-Set 'input' $matrix.required_coverage.input @($hosts.Doc.input|ForEach-Object{$_})
Require-Set 'update channels' $matrix.required_coverage.update_channels @($hosts.Doc.update_channels|ForEach-Object{$_})

$stress=Read-JsonEvidence 'aethercore.ga-stress.v1';foreach($s in $stress){if($s.Doc.version -ne $version){throw "Stress evidence version mismatch: $($s.Path)"}}
$requiredStress=@($stress|Where-Object{$_.Doc.ok -and $_.Doc.profile -eq $matrix.stress.required_profile -and [int]$_.Doc.duration_minutes -ge [int]$matrix.stress.minimum_duration_minutes})
if(-not $requiredStress.Count){throw 'GA requires at least one PASS extended soak meeting the configured minimum duration.'}
foreach($s in $requiredStress){if([int]$s.Doc.request_failures -gt [int]$matrix.stress.max_request_failure_count){throw 'GA soak contains request failures.'};if([int]$s.Doc.stream_resets -gt [int]$matrix.stress.max_stream_reset_count){throw 'GA soak contains unexpected stream resets.'}}
$resilience=Read-JsonEvidence 'aethercore.ga-resilience.v1';foreach($r in $resilience){if($r.Doc.version -ne $version){throw "Resilience evidence version mismatch: $($r.Path)"}};if(-not($resilience|Where-Object{$_.Doc.ok -and [int]$_.Doc.service_restart_cycles -ge 12})){throw 'GA requires PASS adversarial resilience evidence with at least 12 service restart cycles.'}
$lifecycle=Read-JsonEvidence 'aethercore.ga-installer-lifecycle.v1';foreach($l in $lifecycle){if($l.Doc.version -ne $version){throw "Installer lifecycle evidence version mismatch: $($l.Path)"}};if(-not($lifecycle|Where-Object{$_.Doc.ok})){throw 'GA requires PASS Burn/MSI lifecycle evidence.'}
$enterpriseStress=Read-JsonEvidence 'aethercore.enterprise-stress-evidence.v1';foreach($e in $enterpriseStress){if($e.Doc.version -ne $version){throw "Enterprise stress evidence version mismatch: $($e.Path)"}};if(-not($enterpriseStress|Where-Object{$_.Doc.ok -and $_.Doc.profile -eq 'release' -and [int]$_.Doc.targeted_regressions -ge 17})){throw 'GA requires PASS Enterprise release stress evidence with all targeted regressions.'}
$resourceTrend=Read-JsonEvidence 'aethercore.enterprise-resource-trend.v1';foreach($t in $resourceTrend){if($t.Doc.version -ne $version){throw "Enterprise resource-trend evidence version mismatch: $($t.Path)"}};if(-not($resourceTrend|Where-Object{$_.Doc.ok -and $_.Doc.profile -eq 'release' -and [int]$_.Doc.duration_minutes -ge 1440})){throw 'GA requires PASS Enterprise sustained resource-trend evidence from the extended soak.'}

$hashFile=Join-Path $release 'SHA256SUMS.txt';if(-not(Test-Path $hashFile)){throw 'Release SHA256SUMS.txt is missing.'};Verify-ShaFile $hashFile
$payload=Join-Path $release 'payload';$artifacts=Join-Path $release 'artifacts'
foreach($file in @(Get-ChildItem $payload -File)+@(Get-ChildItem $artifacts -File)){if($file.Extension -in @('.exe','.msi')){$sig=Get-AuthenticodeSignature $file.FullName;if($sig.Status -ne 'Valid'){throw "GA artifact signature invalid: $($file.Name) ($($sig.Status))"}}}
& (Join-Path $PSScriptRoot 'validate-update-trust.ps1') -Path (Join-Path $payload 'update-trust.json') -RequireEnabled
if($LASTEXITCODE -ne 0){throw 'Packaged update trust is not production-enabled.'}
if(-not(Get-ChildItem (Join-Path $release 'evidence\sbom') -File -Recurse -ErrorAction SilentlyContinue)){throw 'Release SBOM evidence is missing.'}

$evidenceManifest=Join-Path $release 'GA-EVIDENCE-SHA256SUMS.txt'
$manifestLines=Get-ChildItem $evidence -File -Recurse|Sort-Object FullName|ForEach-Object{"$((Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant())  $([IO.Path]::GetRelativePath($evidence,$_.FullName) -replace '\\','/')"}
$manifestLines|Set-Content $evidenceManifest -Encoding ascii
$evidenceRootHash=(Get-FileHash $evidenceManifest -Algorithm SHA256).Hash.ToLowerInvariant()
$releaseHash=(Get-FileHash $hashFile -Algorithm SHA256).Hash.ToLowerInvariant()
$sourceCommit=$meta.source_commit
$sealPath=Join-Path $release 'GA-SEAL.json';$sigPath=Join-Path $release 'GA-SEAL.p7s'
$seal=[ordered]@{
    schema='aethercore.ga-seal.v1';product='AetherCore';version=$version;ga=$true;sealed_utc=[DateTimeOffset]::UtcNow.ToString('o');
    source_commit=$sourceCommit;release_sha256s_sha256=$releaseHash;evidence_manifest_sha256=$evidenceRootHash;
    matrix_sha256=(Get-FileHash (Join-Path $Root 'release\ga-matrix.json') -Algorithm SHA256).Hash.ToLowerInvariant();
    host_evidence_count=$hosts.Count;os_lanes=@($matrix.required_os_lanes.id);stress_profile=$matrix.stress.required_profile;minimum_soak_minutes=[int]$matrix.stress.minimum_duration_minutes;
    signer_thumbprint=($CertificateThumbprint -replace '\s','').ToUpperInvariant();claims=@('all-phase-gates-pass','enterprise-convergence-audit-pass','signed-release','burn-msi-lifecycle-pass','ga-host-matrix-covered','extended-soak-pass','enterprise-resource-trend-pass','enterprise-targeted-regressions-pass','adversarial-resilience-pass','sbom-present','update-trust-enabled')
}
$seal|ConvertTo-Json -Depth 6|Set-Content $sealPath -Encoding utf8

$store="Cert:\$CertificateStore\My";$thumb=($CertificateThumbprint -replace '\s','').ToUpperInvariant();$cert=Get-ChildItem $store|Where-Object{$_.Thumbprint -eq $thumb}|Select-Object -First 1
if(-not $cert -or -not $cert.HasPrivateKey){throw 'GA signing certificate with private key was not found in the selected store.'}
Add-Type -AssemblyName System.Security.Cryptography.Pkcs
$content=[IO.File]::ReadAllBytes($sealPath);$ci=[System.Security.Cryptography.Pkcs.ContentInfo]::new($content);$cms=[System.Security.Cryptography.Pkcs.SignedCms]::new($ci,$true);$signer=[System.Security.Cryptography.Pkcs.CmsSigner]::new($cert);$signer.IncludeOption=[System.Security.Cryptography.X509Certificates.X509IncludeOption]::EndCertOnly;$cms.ComputeSignature($signer);[IO.File]::WriteAllBytes($sigPath,$cms.Encode())
$check=[System.Security.Cryptography.Pkcs.SignedCms]::new([System.Security.Cryptography.Pkcs.ContentInfo]::new($content),$true);$check.Decode([IO.File]::ReadAllBytes($sigPath));$check.CheckSignature([bool]$SkipCertificateChainCheck)
Write-Host "AetherCore GA seal created and cryptographically verified: $sealPath" -ForegroundColor Green
