# P36 Tranche 1 - WS B/C retry after Product.wxs launch-condition fix
$ErrorActionPreference = 'Continue'
$inc = 'C:\AetherCore-P36\incoming'
$logs = 'C:\AetherCore-P36\logs'
$evid = 'C:\AetherCore-P36\evidence'
$ws = 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery'
$env:DOTNET_ROOT = 'C:\AetherCore-P36\toolchain\dotnet'
$env:PATH = "$env:DOTNET_ROOT;$env:PATH"
$msi = 'C:\AetherCore-P36\incoming\AetherCore-0.1.0-arm64.msi'

# B1: rebuild (same deterministic ProductCode as before - same version+arch inputs)
$productCode = (Get-Content "$inc\tranche1-productcode.txt" -Raw).Replace("ProductCode=", "").Trim()
Set-Location $ws
& $env:DOTNET_ROOT\dotnet.exe tool run wix build installer\wix\Product.wxs -arch arm64 -o $msi `
    -d "PayloadDir=C:\AetherCore-P36\incoming\payload" `
    -d "ProductVersion=0.1.0" `
    -d "ProductCode=$productCode" *> "$inc\tranche1-wix-build2.log"
$ecBuild = $LASTEXITCODE
Set-Content "$inc\tranche1-stepB1.status" "EXIT=$ecBuild"
if ($ecBuild -ne 0) { Set-Content "$inc\tranche1-masterB.status" "EXIT=1 (wix build2 $ecBuild)"; exit 1 }

# B2: validate
& $env:DOTNET_ROOT\dotnet.exe tool run wix msi validate $msi *> "$inc\tranche1-wix-validate2.log"
$ecVal = $LASTEXITCODE
Set-Content "$inc\tranche1-stepB2.status" "EXIT=$ecVal"
if ($ecVal -ne 0) { Set-Content "$inc\tranche1-masterB.status" "EXIT=1 (wix validate2 $ecVal)"; exit 1 }

# C1: install with verbose log
$ilog = "$logs\tranche1-msi-install2.log"
$p = Start-Process msiexec.exe -ArgumentList "/i", "`"$msi`"", "/l*v", "`"$ilog`"", "/qn", "/norestart" -Wait -PassThru
$ec = $p.ExitCode
Set-Content "$inc\tranche1-stepC1.status" "EXIT=$ec (msiexec)"
"msiexec exit: $ec" | Set-Content "$evid\tranche1-install-result.txt"
if ($ec -ne 0) { Set-Content "$inc\tranche1-masterC.status" "EXIT=1 (msiexec $ec)"; exit 1 }

# C2: post-install verification (identical to install-verify.ps1)
$v = [ordered]@{}
$v.installExitCode = $ec
$v.installedFiles = @()
$pf = Join-Path $env:ProgramFiles 'AetherCore'
foreach ($f in @('aethercore-desktop.exe','aethercore-maintenance-service.exe','aethercore-consent-broker.exe','aethercore-update-broker.exe','aethercore-install-hardener.exe','update-trust.json')) {
    $p2 = Join-Path $pf $f
    if (Test-Path $p2) {
        $h = (Get-FileHash $p2 -Algorithm SHA256).Hash
        $ro = & 'C:\AetherCore-P36\toolchain\llvm-22.1.8\bin\llvm-readobj.exe' --file-headers $p2 2>$null | Select-String 'Machine:'
        $arch = if ($ro -and $f -like '*.exe') { $ro[0].ToString().Trim() } else { 'n/a' }
        $v.installedFiles += [ordered]@{ file = $p2; sha256 = $h; bytes = (Get-Item $p2).Length; pe = $arch }
    } else {
        $v.installedFiles += [ordered]@{ file = $p2; MISSING = $true }
    }
}
$sm = "$env:ProgramData\Microsoft\Windows\Start Menu\Programs\AetherCore\AetherCore.lnk"
$v.startMenuShortcut = [ordered]@{ path = $sm; exists = (Test-Path $sm) }
if (Test-Path $sm) {
    $sh = New-Object -ComObject WScript.Shell
    $lnk = $sh.CreateShortcut($sm)
    $v.startMenuShortcut.target = $lnk.TargetPath
    $v.startMenuShortcut.workingDir = $lnk.WorkingDirectory
}
$v.programMenuMarker = [ordered]@{}
foreach ($hive in @('Registry::HKEY_USERS\S-1-5-18\Software\AetherCore', 'HKLM:\SOFTWARE\AetherCore')) {
    if (Test-Path $hive) { $v.programMenuMarker.$hive = (Get-ItemProperty $hive -ErrorAction SilentlyContinue | Out-String).Trim() } else { $v.programMenuMarker.$hive = 'ABSENT' }
}
$trust = Get-Content (Join-Path $pf 'update-trust.json') -Raw | ConvertFrom-Json
$v.updateTrust = [ordered]@{ schema = $trust.schema; enabled = $trust.enabled; channelCount = @($trust.channels).Count }
$keyHits = Get-ChildItem $pf -Recurse -File | Select-String -Pattern 'BEGIN (RSA )?PRIVATE KEY|PRIVATE KEY BLOCK' -List -ErrorAction SilentlyContinue
$v.privateKeyScan = [ordered]@{ privateKeyMaterialFound = [bool]$keyHits; filesFlagged = @($keyHits | ForEach-Object { $_.Path }) }
$unin = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*' -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -like '*AetherCore*' }
$v.uninstallEntry = if ($unin) { [ordered]@{ found = $true; name = $unin.DisplayName; version = $unin.DisplayVersion; psChildName = $unin.PSChildName } } else { [ordered]@{ found = $false } }
$msiHash = (Get-FileHash $msi -Algorithm SHA256).Hash
$v.msi = [ordered]@{ path = $msi; sha256 = $msiHash; bytes = (Get-Item $msi).Length; productCode = $productCode; upgradeCode = '{45598C77-2C32-5BCE-8510-19C7E51EE3B8}'; version = '0.1.0'; wixArch = 'arm64'; authenticode = 'UNAVAILABLE (no certificate provisioned; recorded honestly)' }
$v | ConvertTo-Json -Depth 5 | Set-Content "$evid\tranche1-install-verify.json"
Set-Content "$inc\tranche1-masterC.status" 'EXIT=0'
Get-Content "$evid\tranche1-install-verify.json"
