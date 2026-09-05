<#
.SYNOPSIS
  Fails the build when the MSI carries a file the installer author never declared —
  in particular an example, probe or other developer binary.

.WHY
  Third occurrence of one class of mistake:
    DBT-P36-001  six diagnostic probes compiled from inside product source
    DBT-P36-008  ipc_probe.exe found in the INSTALLED C:\Program Files\AetherCore
    DBT-P40-001  p39_pipe_attack.rs sitting in the shipping crate's examples/
  Twice the file was believed harmless because "examples are not packaged". That is
  a belief, not a measurement. This is the measurement, and it runs every build.

.HOW
  The allowlist is not a list anyone has to maintain: it is derived from
  installer/wix/Product.wxs, the file that IS the authorization to ship something.
  Every row of the MSI File table must correspond to a File element authored there.
  Anything else — however it got in — is a failure. Product.wxs is itself checked
  against the developer-artefact name patterns, so a probe cannot be laundered by
  authoring it.

.USAGE
  powershell -File scripts\check-msi-payload.ps1 -Msi <path.msi> [-Wxs installer\wix\Product.wxs]
  Exit 0 = clean, 1 = a disallowed file is in the payload, 2 = could not check.
#>
param(
    [Parameter(Mandatory = $true)][string]$Msi,
    [string]$Wxs
)
$ErrorActionPreference = 'Stop'

if (-not $Wxs) { $Wxs = Join-Path (Split-Path $PSScriptRoot -Parent) 'installer\wix\Product.wxs' }
foreach ($p in @($Msi, $Wxs)) {
    if (-not (Test-Path $p)) { Write-Output "PAYLOAD_CHECK=ERROR missing: $p"; exit 2 }
}

# Names that are never product payload, whatever the extension.
$devPattern = '(?i)(^|[\\/_-])(p\d+_|probe|attack|fuzz|bench|example|_test|-test)'

# --- authorized set, straight out of the WiX authoring -----------------------
$authored = @{}
foreach ($m in [regex]::Matches((Get-Content -Raw $Wxs), 'Source="[^"]*[\\/]([^"\\/]+)"')) {
    $name = $m.Groups[1].Value
    $authored[$name.ToLowerInvariant()] = $true
    if ($name -match $devPattern) {
        Write-Output "PAYLOAD_CHECK=FAIL developer artefact AUTHORED in ${Wxs}: $name"
        exit 1
    }
}
Write-Output ("AUTHORED_FILES=" + $authored.Count)

# --- what the package actually contains --------------------------------------
$wi = New-Object -ComObject WindowsInstaller.Installer
$db = $wi.GetType().InvokeMember('OpenDatabase', 'InvokeMethod', $null, $wi, @($Msi, 0))
$vw = $db.GetType().InvokeMember('OpenView', 'InvokeMethod', $null, $db, @('SELECT `File`,`FileName` FROM `File`'))
$vw.GetType().InvokeMember('Execute', 'InvokeMethod', $null, $vw, $null)

$rows = 0
$bad = @()
while ($true) {
    $rec = $vw.GetType().InvokeMember('Fetch', 'InvokeMethod', $null, $vw, $null)
    if ($null -eq $rec) { break }
    $key = $rec.GetType().InvokeMember('StringData', 'GetProperty', $null, $rec, 1)
    $fn = $rec.GetType().InvokeMember('StringData', 'GetProperty', $null, $rec, 2)
    # FileName is "short|long" when a long name needs an 8.3 alias.
    $long = ($fn -split '\|')[-1]
    $rows++
    if ($long -match $devPattern -or -not $authored.ContainsKey($long.ToLowerInvariant())) {
        $bad += "$key -> $long"
    }
}
Write-Output ("MSI_FILE_ROWS=" + $rows)
if ($rows -eq 0) { Write-Output 'PAYLOAD_CHECK=ERROR File table is empty'; exit 2 }

if ($bad.Count -gt 0) {
    Write-Output 'PAYLOAD_CHECK=FAIL unauthorized files in the MSI payload:'
    $bad | ForEach-Object { Write-Output "  $_" }
    exit 1
}
Write-Output 'PAYLOAD_CHECK=PASS every MSI file is authored in Product.wxs; no developer artefacts'
exit 0
