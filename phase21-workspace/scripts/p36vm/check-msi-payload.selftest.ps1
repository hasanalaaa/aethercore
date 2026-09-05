# Self-test for scripts/check-msi-payload.ps1. Windows-only: reading an MSI File
# table needs the WindowsInstaller COM object.
#
# A build check that has only ever been seen to PASS is not evidence of anything.
# This exercises both failure branches as well, against a real package.
#   Usage: check-msi-payload.selftest.ps1 -Msi <path-to-a-built.msi>
param(
    [Parameter(Mandatory = $true)][string]$Msi,
    [string]$Wxs
)
$ErrorActionPreference = 'Stop'
$root = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent   # ...\phase21-workspace
$chk = Join-Path $root 'scripts\check-msi-payload.ps1'
if (-not $Wxs) { $Wxs = Join-Path $root 'installer\wix\Product.wxs' }
$tmp = Join-Path $env:TEMP ('msi-payload-selftest-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force $tmp | Out-Null
$failures = 0
function Case([string]$name, [int]$want, [string]$useWxs) {
    & powershell -NoProfile -ExecutionPolicy Bypass -File $chk -Msi $Msi -Wxs $useWxs
    $got = $LASTEXITCODE
    $ok = ($got -eq $want)
    Write-Output ("CASE $name WANT=$want GOT=$got " + $(if ($ok) { 'OK' } else { 'FAILED' }))
    if (-not $ok) { $script:failures++ }
}
try {
    Case 'clean-package' 0 $Wxs

    # A probe authored into the installer must be refused even though it would then
    # be, technically, "authorized".
    $w2 = Join-Path $tmp 'authored-probe.wxs'
    (Get-Content -Raw $Wxs) -replace '(?s)(<File Id="AetherCtlExe"[^>]*/>)', '$1
          <File Id="ProbeExe" Source="$(var.PayloadDir)\p39_pipe_attack.exe" />' | Set-Content $w2
    Case 'probe-authored-in-wxs' 1 $w2

    # A file present in the package that no File element authors.
    $w3 = Join-Path $tmp 'missing-author.wxs'
    (Get-Content -Raw $Wxs) -replace 'Source="\$\(var\.PayloadDir\)\\aetherctl\.exe"', 'Source="$(var.PayloadDir)\something-else.exe"' | Set-Content $w3
    Case 'unauthored-file-in-msi' 1 $w3
} finally {
    Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output "SELFTEST_FAILURES=$failures"
exit $(if ($failures -eq 0) { 0 } else { 1 })
