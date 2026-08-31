# P36 Gate-A verification. Runs as SYSTEM via prlctl exec. Read-only: it mutates
# nothing except its own output files under the share.
# Usage: verify-install.ps1 <label>
param([string]$Label = 'unlabeled')
$ErrorActionPreference = 'Continue'
$OutRoot = '\\Mac\dev\p36-stage\out'
$IF = 'C:\Program Files\AetherCore'
$r = [ordered]@{ label=$Label; captured_utc=(Get-Date).ToUniversalTime().ToString('o') }

# --- 1/2. service state, account, start type, service SID -------------------
$r.sc_query   = (& "$env:SystemRoot\System32\sc.exe" query AetherCoreMaintenance 2>&1 | Out-String).Trim()
$r.sc_qc      = (& "$env:SystemRoot\System32\sc.exe" qc AetherCoreMaintenance 2>&1 | Out-String).Trim()
$r.sc_qsidtype= (& "$env:SystemRoot\System32\sc.exe" qsidtype AetherCoreMaintenance 2>&1 | Out-String).Trim()
$r.sc_sdshow  = (& "$env:SystemRoot\System32\sc.exe" sdshow AetherCoreMaintenance 2>&1 | Out-String).Trim()

# --- 3. pipe DACL --------------------------------------------------------
# Get-Acl fails on a pipe, and [IO.File]::Open goes through FileStream, which
# refuses a non-file device ("FileStream was asked to open a device that was
# not a file"). NamedPipeClientStream is the method that actually returns the
# descriptor, and it is the method that produced the tranche-1/2 evidence.
$r.pipe_sddl = 'ERROR: not read'
try {
    $pc = New-Object System.IO.Pipes.NamedPipeClientStream(
        '.', 'AetherCore.Maintenance.v7', [IO.Pipes.PipeDirection]::In)
    $pc.Connect(5000)
    $r.pipe_sddl = $pc.GetAccessControl().GetSecurityDescriptorSddlForm('All')
    $pc.Dispose()
} catch { $r.pipe_sddl = 'ERROR: ' + $_.Exception.Message }
$r.pipe_present = [bool](Get-ChildItem '\\.\pipe\' -EA SilentlyContinue |
    Where-Object { $_.Name -like 'AetherCore.Maintenance*' })

# --- 4. install-dir ACLs ----------------------------------------------------
$r.installdir_icacls = (& "$env:SystemRoot\System32\icacls.exe" $IF 2>&1 | Out-String).Trim()

# --- 5/6. payload presence, ipc_probe absence -------------------------------
$files = @()
if (Test-Path $IF) {
    foreach ($f in (Get-ChildItem $IF -File -Recurse)) {
        $files += [ordered]@{ name=$f.Name; size=$f.Length
                              sha256=(Get-FileHash $f.FullName -Algorithm SHA256).Hash.ToLower() }
    }
}
$r.installdir_files  = $files
$r.installdir_exists = (Test-Path $IF)
$r.libomp_present    = (Test-Path (Join-Path $IF 'libomp140.aarch64.dll'))
$r.ipc_probe_absent  = -not (Get-ChildItem $IF -Recurse -Filter 'ipc_probe*' -EA SilentlyContinue)

# --- registration / ARP -----------------------------------------------------
$r.arp = @(Get-ChildItem 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall' -EA SilentlyContinue |
    ForEach-Object { Get-ItemProperty $_.PSPath -EA SilentlyContinue } |
    Where-Object { $_.DisplayName -like '*AetherCore*' } |
    ForEach-Object { [ordered]@{ key=$_.PSChildName; name=$_.DisplayName; version=$_.DisplayVersion } })
$r.hklm_installversion = (Get-ItemProperty 'HKLM:\SOFTWARE\AetherCore' -Name InstallVersion -EA SilentlyContinue).InstallVersion

# --- ProgramData survival ---------------------------------------------------
$r.programdata = @()
foreach ($p in @('C:\ProgramData\AetherCore','C:\ProgramData\AetherCore\state','C:\ProgramData\AetherCore\logs','C:\ProgramData\AetherCore\support-staging')) {
    $r.programdata += [ordered]@{ path=$p; exists=(Test-Path $p)
        count=@(Get-ChildItem $p -Recurse -File -EA SilentlyContinue).Count }
}

$json = $r | ConvertTo-Json -Depth 6
Set-Content -LiteralPath (Join-Path $OutRoot ("verify-$Label.json")) -Value $json -Encoding utf8
Write-Output "VERIFY_WRITTEN=verify-$Label.json"
Write-Output ($r.sc_query -split "`n" | Where-Object { $_ -match 'STATE' })
Write-Output ("PIPE_SDDL=" + $r.pipe_sddl)
Write-Output ("LIBOMP=" + $r.libomp_present + " IPC_PROBE_ABSENT=" + $r.ipc_probe_absent + " FILES=" + $files.Count)
