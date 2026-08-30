$ErrorActionPreference = 'Continue'
Write-Output '=== refresh payload with fixed service exe ==='
Copy-Item 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery\target\release\aethercore-maintenance-service.exe' 'C:\AetherCore-P36\incoming\payload\aethercore-maintenance-service.exe' -Force
$ws = 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery'
# update payload manifest
$inc = 'C:\AetherCore-P36\incoming'
$readobj = 'C:\AetherCore-P36\toolchain\llvm-22.1.8\bin\llvm-readobj.exe'
$exes = @('aethercore-desktop.exe','aethercore-maintenance-service.exe','aethercore-consent-broker.exe','aethercore-update-broker.exe','aethercore-install-hardener.exe')
$manifest = @()
foreach ($e in $exes) {
    $p = Join-Path "$inc\payload" $e
    $h = (Get-FileHash $p -Algorithm SHA256).Hash
    $sz = (Get-Item $p).Length
    $ro = & $readobj --file-headers $p 2>$null | Select-String 'Machine:'
    $arch = if ($ro) { $ro[0].ToString().Trim() } else { '?' }
    $manifest += [ordered]@{ file = $e; sha256 = $h; bytes = $sz; pe = $arch }
}
$h = (Get-FileHash "$inc\payload\libomp140.aarch64.dll" -Algorithm SHA256).Hash
$manifest += [ordered]@{ file = 'libomp140.aarch64.dll'; sha256 = $h; bytes = (Get-Item "$inc\payload\libomp140.aarch64.dll").Length; pe = 'dll' }
$h = (Get-FileHash "$inc\payload\update-trust.json" -Algorithm SHA256).Hash
$manifest += [ordered]@{ file = 'update-trust.json'; sha256 = $h; bytes = (Get-Item "$inc\payload\update-trust.json").Length }
$manifest | ConvertTo-Json -Depth 4 | Set-Content "$inc\payload\payload-manifest.json"
Get-Content "$inc\payload\payload-manifest.json"
