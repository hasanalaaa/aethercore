[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][ValidateSet('stable','beta')][string]$Channel,
    [Parameter(Mandatory=$true)][UInt64]$Sequence,
    [Parameter(Mandatory=$true)][ValidatePattern('^\d+\.\d+\.\d+$')][string]$Version,
    [Parameter(Mandatory=$true)][string]$ReleaseId,
    [Parameter(Mandatory=$true)][string]$BundlePath,
    [Parameter(Mandatory=$true)][string]$BundleUrl,
    [Parameter(Mandatory=$true)][string]$PrivateKeyPath,
    [Parameter(Mandatory=$true)][string]$KeyId,
    [string]$NotesMessageKey='update.notes.release',
    [UInt32]$MinimumWindowsBuild=22621,
    [int]$ValidityDays=14,
    [string]$OutputDirectory='out\update-manifest'
)
$ErrorActionPreference='Stop';$Root=Split-Path $PSScriptRoot -Parent;Set-Location $Root
if($ReleaseId -notmatch '^[A-Za-z0-9._-]{1,96}$'){throw 'ReleaseId is not safe.'}
$uri=[Uri]$BundleUrl;if($uri.Scheme -ne 'https' -or -not $uri.IsAbsoluteUri -or $uri.UserInfo -or $uri.Fragment){throw 'BundleUrl must be credential-free HTTPS.'}
$bundle=(Resolve-Path $BundlePath).Path;$private=(Resolve-Path $PrivateKeyPath).Path
$now=[DateTimeOffset]::UtcNow;$hash=(Get-FileHash $bundle -Algorithm SHA256).Hash.ToLowerInvariant();$size=(Get-Item $bundle).Length
$doc=[ordered]@{schema='aethercore.update-manifest.v1';channel=$Channel;sequence=$Sequence;generatedUnixMs=$now.ToUnixTimeMilliseconds();expiresUnixMs=$now.AddDays($ValidityDays).ToUnixTimeMilliseconds();releases=@([ordered]@{releaseId=$ReleaseId;version=$Version;publishedUnixMs=$now.ToUnixTimeMilliseconds();notesMessageKey=$NotesMessageKey;minimumWindowsBuild=$MinimumWindowsBuild;package=[ordered]@{kind='burn';url=$BundleUrl;sizeBytes=$size;sha256=$hash}})}
$out=[IO.Path]::GetFullPath((Join-Path $Root $OutputDirectory));New-Item -ItemType Directory -Force $out|Out-Null;$manifest=Join-Path $out "$Channel.json";$signature=Join-Path $out "$Channel.sig.json"
[IO.File]::WriteAllText($manifest,($doc|ConvertTo-Json -Depth 8 -Compress),[Text.UTF8Encoding]::new($false))
$env:AETHERCORE_UPDATE_KEY_ID=$KeyId
& cargo run --locked --release -p aethercore-update-manifest-tool -- --manifest $manifest --private-key $private --out $signature
if($LASTEXITCODE -ne 0){throw 'Update manifest signing failed.'}
Write-Host "Signed update manifest: $manifest" -ForegroundColor Green
