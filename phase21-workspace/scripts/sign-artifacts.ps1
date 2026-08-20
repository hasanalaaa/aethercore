[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string[]]$Path,
    [string]$CertificateThumbprint = $env:AETHERCORE_CODESIGN_THUMBPRINT,
    [ValidateSet('CurrentUser','LocalMachine')][string]$CertificateStore = 'CurrentUser',
    [string]$TimestampUrl = $env:AETHERCORE_TIMESTAMP_URL,
    [switch]$RequireSigning
)
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'Authenticode signing is Windows-only.' }

if (-not $CertificateThumbprint) {
    if ($RequireSigning) { throw 'Signing is required but AETHERCORE_CODESIGN_THUMBPRINT was not configured.' }
    Write-Warning 'Code-signing certificate is not configured; leaving artifacts unsigned.'
    return
}
if (-not $TimestampUrl) {
    if ($RequireSigning) { throw 'Signing is required but AETHERCORE_TIMESTAMP_URL was not configured.' }
    throw 'A timestamp URL is required whenever signing is enabled.'
}
if ($TimestampUrl -notmatch '^https://') { throw 'Timestamp URL must use HTTPS.' }

function Find-SignTool {
    $cmd = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    $kits = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
    if (Test-Path $kits) {
        $found = Get-ChildItem $kits -Filter signtool.exe -Recurse -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -match '\\x64\\signtool\.exe$' } |
            Sort-Object FullName -Descending | Select-Object -First 1
        if ($found) { return $found.FullName }
    }
    throw 'signtool.exe was not found. Install the Windows SDK signing tools.'
}

$SignTool = Find-SignTool
$thumb = ($CertificateThumbprint -replace '\s','').ToUpperInvariant()
foreach ($item in $Path) {
    $resolved = (Resolve-Path $item).Path
    $args = @('sign','/sha1',$thumb,'/fd','SHA256','/tr',$TimestampUrl,'/td','SHA256','/v')
    if ($CertificateStore -eq 'LocalMachine') { $args += '/sm' }
    $args += $resolved
    & $SignTool @args
    if ($LASTEXITCODE -ne 0) { throw "signtool sign failed for $resolved" }
    & $SignTool verify /pa /all /v $resolved
    if ($LASTEXITCODE -ne 0) { throw "signtool verification failed for $resolved" }
}
