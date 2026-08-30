# P36 Tranche 1 - Workstream B: real MSI build (arm64) from reviewed Product.wxs + real payload
# Faithful to scripts/build-installer.ps1's deterministic ProductCode scheme; only the
# arch input differs (arm64 qualification MSI). Exact command recorded for evidence.
$ErrorActionPreference = 'Continue'
$inc = 'C:\AetherCore-P36\incoming'
$ws = 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery'
$env:DOTNET_ROOT = 'C:\AetherCore-P36\toolchain\dotnet'
$env:PATH = "$env:DOTNET_ROOT;$env:PATH"

# Deterministic ProductCode (same namespace + scheme as build-installer.ps1, arch=arm64)
$version = '0.1.0'
$namespace = [Text.Encoding]::UTF8.GetBytes('AetherCore/MSI/ProductCode/v1')
$value = [Text.Encoding]::UTF8.GetBytes("AetherCore/$version/arm64")
$sha = [Security.Cryptography.SHA256]::Create()
try { $hash = $sha.ComputeHash($namespace + $value) } finally { $sha.Dispose() }
$productCode = ([Guid]::new([byte[]]$hash[0..15])).ToString('B').ToUpperInvariant()
"ProductCode=$productCode" | Set-Content "$inc\tranche1-productcode.txt"

Set-Location $ws
$msi = 'C:\AetherCore-P36\incoming\AetherCore-0.1.0-arm64.msi'
& $env:DOTNET_ROOT\dotnet.exe tool run wix build installer\wix\Product.wxs -arch arm64 -o $msi `
    -d "PayloadDir=C:\AetherCore-P36\incoming\payload" `
    -d "ProductVersion=$version" `
    -d "ProductCode=$productCode" *> "$inc\tranche1-wix-build.log"
$ecBuild = $LASTEXITCODE
Set-Content "$inc\tranche1-stepB1.status" "EXIT=$ecBuild"
if ($ecBuild -ne 0) { Set-Content "$inc\tranche1-masterB.status" "EXIT=1 (wix build $ecBuild)"; exit 1 }

& $env:DOTNET_ROOT\dotnet.exe tool run wix msi validate $msi *> "$inc\tranche1-wix-validate.log"
$ecVal = $LASTEXITCODE
Set-Content "$inc\tranche1-stepB2.status" "EXIT=$ecVal"
if ($ecVal -ne 0) { Set-Content "$inc\tranche1-masterB.status" "EXIT=1 (wix validate $ecVal)"; exit 1 }

$msiHash = (Get-FileHash $msi -Algorithm SHA256).Hash
$msiBytes = (Get-Item $msi).Length
$ro = & 'C:\AetherCore-P36\toolchain\llvm-22.1.8\bin\llvm-readobj.exe' --file-headers $msi 2>$null | Select-String 'Machine:'
$mach = if ($ro) { $ro[0].ToString().Trim() } else { 'not-a-pe-or-no-output' }
[ordered]@{
    msi = $msi; sha256 = $msiHash; bytes = $msiBytes
    productCode = $productCode; upgradeCode = '{45598C77-2C32-5BCE-8510-19C7E51EE3B8}'
    version = $version; wixArch = 'arm64'; summary_machine_probe = $mach
    payloadDir = 'C:\AetherCore-P36\incoming\payload'
    build_command = "dotnet tool run wix build installer\wix\Product.wxs -arch arm64 -o $msi -d PayloadDir=C:\AetherCore-P36\incoming\payload -d ProductVersion=0.1.0 -d ProductCode=$productCode"
    validate_command = "dotnet tool run wix msi validate $msi"
    authenticode_signing = 'UNAVAILABLE (no certificate provisioned in this VM; recorded honestly, not claimed)'
} | ConvertTo-Json -Depth 3 | Set-Content "$inc\tranche1-msi-evidence.json"
Set-Content "$inc\tranche1-masterB.status" 'EXIT=0'
Get-Content "$inc\tranche1-msi-evidence.json"
