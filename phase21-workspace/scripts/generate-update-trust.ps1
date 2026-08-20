[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][ValidateSet('stable','beta')][string[]]$Channel,
    [Parameter(Mandatory=$true)][string[]]$ManifestUrl,
    [Parameter(Mandatory=$true)][string[]]$SignatureUrl,
    [Parameter(Mandatory=$true)][string[]]$KeyId,
    [Parameter(Mandatory=$true)][string[]]$PublicKeyHex,
    [string]$OutputPath = 'out\update-trust.json'
)
$ErrorActionPreference='Stop'
if ($Channel.Count -ne $ManifestUrl.Count -or $Channel.Count -ne $SignatureUrl.Count -or $Channel.Count -ne $KeyId.Count -or $Channel.Count -ne $PublicKeyHex.Count) { throw 'Channel arrays must have identical lengths.' }
$seen=@{}
$channels=@()
for($i=0;$i -lt $Channel.Count;$i++){
    $name=$Channel[$i].ToLowerInvariant(); if($seen[$name]){throw "Duplicate update channel: $name"};$seen[$name]=$true
    foreach($url in @($ManifestUrl[$i],$SignatureUrl[$i])){ $uri=[Uri]$url; if($uri.Scheme -ne 'https' -or -not $uri.IsAbsoluteUri -or $uri.UserInfo -or $uri.Fragment){throw "Update trust URL must be credential-free HTTPS: $url"} }
    if($KeyId[$i] -notmatch '^[A-Za-z0-9._-]{1,64}$'){throw 'KeyId contains unsupported characters.'}
    if($PublicKeyHex[$i] -notmatch '^[0-9a-fA-F]{64}$'){throw 'Ed25519 public key must be exactly 32 bytes encoded as 64 hex characters.'}
    $channels += [ordered]@{channel=$name;manifestUrl=$ManifestUrl[$i];signatureUrl=$SignatureUrl[$i];keyId=$KeyId[$i];publicKeyHex=$PublicKeyHex[$i].ToLowerInvariant()}
}
$doc=[ordered]@{schema='aethercore.update-trust.v1';enabled=$true;channels=$channels}
$full=[IO.Path]::GetFullPath((Join-Path (Get-Location) $OutputPath));New-Item -ItemType Directory -Force (Split-Path $full -Parent)|Out-Null
[IO.File]::WriteAllText($full,($doc|ConvertTo-Json -Depth 6),[Text.UTF8Encoding]::new($false))
Write-Host "Update trust configuration written: $full" -ForegroundColor Green
