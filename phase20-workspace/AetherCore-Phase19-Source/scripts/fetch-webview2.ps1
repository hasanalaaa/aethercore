[CmdletBinding()]
param(
    [string]$OutputPath = 'out\prereqs\MicrosoftEdgeWebview2Setup.exe'
)
$ErrorActionPreference = 'Stop'
$Root = Split-Path $PSScriptRoot -Parent
Set-Location $Root
if ($env:OS -ne 'Windows_NT') { throw 'WebView2 bootstrapper acquisition must run on Windows.' }

# Fixed Microsoft Evergreen Bootstrapper URL. Do not accept caller-supplied URLs in the release path.
$Uri = 'https://go.microsoft.com/fwlink/p/?LinkId=2124703'
$Destination = Join-Path $Root $OutputPath
New-Item -ItemType Directory -Force (Split-Path $Destination -Parent) | Out-Null
$tmp = "$Destination.download"
Remove-Item $tmp -Force -ErrorAction SilentlyContinue

Invoke-WebRequest -Uri $Uri -OutFile $tmp -UseBasicParsing
if (-not (Test-Path $tmp) -or (Get-Item $tmp).Length -lt 500000) {
    throw 'Downloaded WebView2 bootstrapper is unexpectedly small or missing.'
}

$sig = Get-AuthenticodeSignature -FilePath $tmp
if ($sig.Status -ne 'Valid') {
    throw "WebView2 bootstrapper Authenticode signature is not valid: $($sig.Status)"
}
if (-not $sig.SignerCertificate -or $sig.SignerCertificate.Subject -notmatch 'Microsoft Corporation') {
    throw "WebView2 bootstrapper is not signed by Microsoft Corporation. Subject: $($sig.SignerCertificate.Subject)"
}

Move-Item $tmp $Destination -Force
$hash = (Get-FileHash $Destination -Algorithm SHA256).Hash.ToLowerInvariant()
Write-Host "Verified Microsoft Evergreen WebView2 bootstrapper: $Destination" -ForegroundColor Green
Write-Host "SHA-256: $hash"
