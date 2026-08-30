[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$Path,
    [switch]$RequireEnabled
)
$ErrorActionPreference='Stop'
$resolved=(Resolve-Path $Path).Path
$raw=Get-Content $resolved -Raw
if([Text.Encoding]::UTF8.GetByteCount($raw) -gt 65536){throw 'update-trust.json exceeds 64 KiB.'}
try{$doc=$raw|ConvertFrom-Json}catch{throw "update-trust.json is invalid JSON: $($_.Exception.Message)"}
if($doc.schema -ne 'aethercore.update-trust.v1'){throw 'Unsupported update trust schema.'}
if($RequireEnabled -and -not [bool]$doc.enabled){throw 'Signed production packaging requires enabled in-app update trust.'}
$channels=@($doc.channels)
if([bool]$doc.enabled){
    if($channels.Count -lt 1 -or $channels.Count -gt 4){throw 'Enabled update trust requires 1-4 channels.'}
    $seen=@{}
    foreach($entry in $channels){
        $name=[string]$entry.channel
        if($name -notin @('stable','beta')){throw "Unsupported update channel: $name"}
        if($seen[$name]){throw "Duplicate update channel: $name"};$seen[$name]=$true
        foreach($field in @('manifestUrl','signatureUrl')){
            $value=[string]$entry.$field
            $uri=$null
            if(-not [Uri]::TryCreate($value,[UriKind]::Absolute,[ref]$uri) -or $uri.Scheme -ne 'https' -or $uri.UserInfo -or $uri.Fragment){throw "$field must be absolute credential-free HTTPS."}
        }
        if(([string]$entry.keyId) -notmatch '^[A-Za-z0-9._-]{1,64}$'){throw 'Invalid update trust keyId.'}
        if(([string]$entry.publicKeyHex) -notmatch '^[0-9a-fA-F]{64}$'){throw 'Update trust Ed25519 public key must be 32 bytes encoded as hex.'}
        if(([string]$entry.publicKeyHex) -match '^0{64}$'){throw 'All-zero update public key is forbidden.'}
    }
    if(-not $seen['stable']){throw 'Enabled production update trust must define the stable channel.'}
}
Write-Host "Update trust validated: $resolved" -ForegroundColor Green
