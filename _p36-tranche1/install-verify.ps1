# P36 Tranche 1 - Workstream C: controlled install + post-install verification
# Context: SYSTEM (machine bootstrap per P36-D009). User-context semantics are probed separately.
$ErrorActionPreference = 'Continue'
$inc = 'C:\AetherCore-P36\incoming'
$logs = 'C:\AetherCore-P36\logs'
$evid = 'C:\AetherCore-P36\evidence'
$msi = 'C:\AetherCore-P36\incoming\AetherCore-0.1.0-arm64.msi'
$ilog = "$logs\tranche1-msi-install.log"

# Install with verbose log
$p = Start-Process msiexec.exe -ArgumentList "/i", "`"$msi`"", "/l*v", "`"$ilog`"", "/qn", "/norestart" -Wait -PassThru
$ec = $p.ExitCode
Set-Content "$inc\tranche1-stepC1.status" "EXIT=$ec (msiexec)"
"msiexec exit: $ec" | Set-Content "$evid\tranche1-install-result.txt"
if ($ec -ne 0) { Set-Content "$inc\tranche1-masterC.status" "EXIT=1 (msiexec $ec)"; exit 1 }

# Post-install verification
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

# Start Menu shortcut (All Users profile under per-machine)
$sm = "$env:ProgramData\Microsoft\Windows\Start Menu\Programs\AetherCore\AetherCore.lnk"
$v.startMenuShortcut = [ordered]@{ path = $sm; exists = (Test-Path $sm) }
if (Test-Path $sm) {
    $sh = New-Object -ComObject WScript.Shell
    $lnk = $sh.CreateShortcut($sm)
    $v.startMenuShortcut.target = $lnk.TargetPath
    $v.startMenuShortcut.workingDir = $lnk.WorkingDirectory
}

# HKCU ProgramMenu marker (written to the INSTALLING context's hive = SYSTEM here; recorded honestly)
$v.programMenuMarker = [ordered]@{}
foreach ($hive in @('HKLM:\SOFTWARE\AetherCore', 'Registry::HKEY_USERS\S-1-5-18\Software\AetherCore')) {
    if (Test-Path $hive) {
        $v.programMenuMarker.$hive = (Get-ItemProperty $hive | Out-String).Trim()
    } else {
        $v.programMenuMarker.$hive = 'ABSENT'
    }
}

# Trust material: template must be enabled=false, no keys, no private key material anywhere in install dir
$trust = Get-Content (Join-Path $pf 'update-trust.json') -Raw | ConvertFrom-Json
$v.updateTrust = [ordered]@{ schema = $trust.schema; enabled = $trust.enabled; channelCount = @($trust.channels).Count }
$v.privateKeyScan = [ordered]@{}
$keyHits = Get-ChildItem $pf -Recurse -File | Select-String -Pattern 'BEGIN (RSA )?PRIVATE KEY|PRIVATE KEY BLOCK' -List -ErrorAction SilentlyContinue
$v.privateKeyScan.privateKeyMaterialFound = [bool]$keyHits
$v.privateKeyScan.filesFlagged = @($keyHits | ForEach-Object { $_.Path })

# Registry product entry
$unin = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*' -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -like '*AetherCore*' }
$v.uninstallEntry = if ($unin) { [ordered]@{ found = $true; name = $unin.DisplayName; version = $unin.DisplayVersion; psChildName = $unin.PSChildName } } else { [ordered]@{ found = $false } }

$v | ConvertTo-Json -Depth 5 | Set-Content "$evid\tranche1-install-verify.json"
Set-Content "$inc\tranche1-masterC.status" 'EXIT=0'
Get-Content "$evid\tranche1-install-verify.json"
