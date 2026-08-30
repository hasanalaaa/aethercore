# P36 Tranche 1 - FINAL MSI build: embeds all fixes (launch condition, OpenMP DLL, hardener ACL, pipe SD service)
$ErrorActionPreference = 'Continue'
$inc = 'C:\AetherCore-P36\incoming'
$logs = 'C:\AetherCore-P36\logs'
$evid = 'C:\AetherCore-P36\evidence'
$ws = 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery'
$env:DOTNET_ROOT = 'C:\AetherCore-P36\toolchain\dotnet'
$env:PATH = "$env:DOTNET_ROOT;$env:PATH"
$msi = 'C:\AetherCore-P36\incoming\AetherCore-0.1.0-arm64.msi'
$productCode = (Get-Content "$inc\tranche1-productcode.txt" -Raw).Replace("ProductCode=", "").Trim()

Set-Location $ws
& $env:DOTNET_ROOT\dotnet.exe tool run wix build installer\wix\Product.wxs -arch arm64 -o $msi `
    -d "PayloadDir=C:\AetherCore-P36\incoming\payload" -d "ProductVersion=0.1.0" -d "ProductCode=$productCode" *> "$inc\tranche1-wix-build3.log"
$ecB = $LASTEXITCODE
Set-Content "$inc\tranche1-stepB1.status" "EXIT=$ecB"
if ($ecB -ne 0) { Write-Output "wix build FAILED"; Get-Content "$inc\tranche1-wix-build3.log" -Tail 10; exit 1 }

& $env:DOTNET_ROOT\dotnet.exe tool run wix msi validate $msi *> "$inc\tranche1-wix-validate3.log"
$ecV = $LASTEXITCODE
Set-Content "$inc\tranche1-stepB2.status" "EXIT=$ecV"
if ($ecV -ne 0) { Write-Output "wix validate FAILED"; Get-Content "$inc\tranche1-wix-validate3.log" -Tail 10; exit 1 }

# FINAL INSTALL: uninstall previous, then fresh install with verbose log
$u = Start-Process msiexec.exe -ArgumentList "/x", "`"$msi`"", "/qn", "/norestart", "/l*v", "`"$logs\tranche1-msi-final-uninstall.log`"" -Wait -PassThru
Write-Output "pre-uninstall exit=$($u.ExitCode)"
$ilog = "$logs\tranche1-msi-final-install.log"
$p = Start-Process msiexec.exe -ArgumentList "/i", "`"$msi`"", "/l*v", "`"$ilog`"", "/qn", "/norestart" -Wait -PassThru
$ec = $p.ExitCode
Set-Content "$inc\tranche1-stepC1.status" "EXIT=$ec (msiexec)"
Write-Output "FINAL INSTALL exit=$ec"
if ($ec -ne 0) { exit 1 }

# verify installed service exe hash matches the fixed build
$h = (Get-FileHash 'C:\Program Files\AetherCore\aethercore-maintenance-service.exe' -Algorithm SHA256).Hash
Write-Output "installed service exe sha=$h"
Start-Sleep 8
& sc.exe query AetherCoreMaintenance 2>&1 | Select-String 'STATE' | ForEach-Object { Write-Output $_.Line.Trim() }
$msiHash = (Get-FileHash $msi -Algorithm SHA256).Hash
Write-Output "FINAL MSI sha256=$msiHash bytes=$((Get-Item $msi).Length)"
