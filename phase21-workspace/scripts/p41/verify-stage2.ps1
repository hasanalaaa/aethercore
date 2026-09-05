# P41 Gate 2 LIVE verification - READ ONLY. Installs nothing, changes nothing.
$OutDir = 'C:\AetherCore-P41\logs'
$log = Join-Path $OutDir 'gate2-live-verify.log'
function W($s){ Add-Content -Path $log -Value $s -Encoding UTF8; Write-Output $s }
Set-Content -Path $log -Value ("GATE2 LIVE VERIFY " + (Get-Date -Format o)) -Encoding UTF8
$IF = 'C:\Program Files\AetherCore'

W "=== [2] installed files ==="
$files = @()
foreach ($f in (Get-ChildItem $IF -File -Recurse)) {
    $files += New-Object psobject -Property @{ name = $f.Name; size = $f.Length; sha256 = (Get-FileHash $f.FullName -Algorithm SHA256).Hash.ToLower() }
}
W ("INSTALL_FILE_COUNT=" + $files.Count)
foreach ($f in ($files | Sort-Object name)) { W ("  " + $f.name.PadRight(45) + " " + ([string]$f.size).PadLeft(14) + "  " + $f.sha256) }
W ("VCOMP140_PRESENT=" + (Test-Path (Join-Path $IF 'vcomp140.dll')))
W ("LIBOMP_AARCH64_PRESENT=" + (Test-Path (Join-Path $IF 'libomp140.aarch64.dll')))
$devPattern = '(?i)(^|[\\/_-])(p\d+_|probe|attack|fuzz|bench|example|_test|-test)'
$dev = @(Get-ChildItem $IF -Recurse -ErrorAction SilentlyContinue | Where-Object { $_.Name -match $devPattern })
if ($dev.Count -gt 0) { W ("DEV_BINARY_IN_INSTALL_IMAGE=" + (($dev | ForEach-Object { $_.Name }) -join ',')) } else { W "DEV_BINARY_IN_INSTALL_IMAGE=NO" }

W "=== [3] service ==="
$sc = Join-Path $env:SystemRoot 'System32\sc.exe'
W ("--- sc query ---`n"     + ((& $sc query      AetherCoreMaintenance 2>&1 | Out-String).Trim()))
W ("--- sc qc ---`n"        + ((& $sc qc         AetherCoreMaintenance 2>&1 | Out-String).Trim()))
W ("--- sc qsidtype ---`n"  + ((& $sc qsidtype   AetherCoreMaintenance 2>&1 | Out-String).Trim()))
W ("--- sc showsid ---`n"   + ((& $sc showsid    AetherCoreMaintenance 2>&1 | Out-String).Trim()))
W ("--- sc sdshow ---`n"    + ((& $sc sdshow     AetherCoreMaintenance 2>&1 | Out-String).Trim()))

W "=== [4] pipe DACL - both documented read methods ==="
try {
    $pc = New-Object System.IO.Pipes.NamedPipeClientStream('.', 'AetherCore.Maintenance.v7', [IO.Pipes.PipeDirection]::In)
    $pc.Connect(5000)
    W ("PIPE_SDDL_NAMEDPIPECLIENT=" + $pc.GetAccessControl().GetSecurityDescriptorSddlForm('All'))
    $pc.Dispose()
} catch { W ('PIPE_SDDL_NAMEDPIPECLIENT=ERROR: ' + $_.Exception.Message) }
try {
    $fs = [System.IO.File]::Open('\\.\pipe\AetherCore.Maintenance.v7', [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
    W ("PIPE_SDDL_FILESTREAM=" + $fs.GetAccessControl().GetSecurityDescriptorSddlForm('All'))
    $fs.Dispose()
} catch { W ('PIPE_SDDL_FILESTREAM=ERROR: ' + $_.Exception.Message) }
W ("PIPE_PRESENT=" + [bool](Get-ChildItem '\\.\pipe\' -ErrorAction SilentlyContinue | Where-Object { $_.Name -like 'AetherCore.Maintenance*' }))

W "=== [5] install-dir ACLs ==="
W ((& (Join-Path $env:SystemRoot 'System32\icacls.exe') $IF 2>&1 | Out-String).Trim())

W "=== [6] registration ==="
$arp = @(Get-ChildItem 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall' -ErrorAction SilentlyContinue |
    ForEach-Object { Get-ItemProperty $_.PSPath -ErrorAction SilentlyContinue } |
    Where-Object { $_.DisplayName -like '*AetherCore*' })
foreach ($a in $arp) { W ("ARP: " + $a.PSChildName + " | " + $a.DisplayName + " | " + $a.DisplayVersion + " | InstallDate=" + $a.InstallDate) }
W ("HKLM_InstallVersion=" + (Get-ItemProperty 'HKLM:\SOFTWARE\AetherCore' -Name InstallVersion -ErrorAction SilentlyContinue).InstallVersion)

W "=== [7] verbs against the installed aetherctl ==="
$ctl  = Join-Path $IF 'aetherctl.exe'
$vdir = Join-Path $OutDir 'verbs-live'
New-Item -ItemType Directory -Force $vdir | Out-Null
function Run-Verb([string]$label, [string[]]$argv, [int]$timeoutMs) {
    if (-not $timeoutMs) { $timeoutMs = 30000 }
    W ("--- VERB " + $label + " : aetherctl " + ($argv -join ' ') + " ---")
    $so = Join-Path $vdir ($label + '.out')
    $se = Join-Path $vdir ($label + '.err')
    try {
        $pp = Start-Process -FilePath $ctl -ArgumentList $argv -PassThru -NoNewWindow -RedirectStandardOutput $so -RedirectStandardError $se
        $null = $pp.Handle
        $done = $pp.WaitForExit($timeoutMs)
        if ($done) { W 'RESULT=RETURNED'; W ('EXIT_CODE=' + $pp.ExitCode) }
        else { try { $pp.Kill() } catch { }; W ('RESULT=TIMED_OUT_' + $timeoutMs + 'MS') }
    } catch { W ('RESULT=LAUNCH_FAILED: ' + $_.Exception.Message) }
    $o = Get-Content $so -Raw -ErrorAction SilentlyContinue
    $e = Get-Content $se -Raw -ErrorAction SilentlyContinue
    if ($o) { W ('STDOUT: ' + $o.Trim()) } else { W 'STDOUT: (empty)' }
    if ($e) { W ('STDERR: ' + $e.Trim()) }
}
W ('CTL_SHA256=' + (Get-FileHash $ctl -Algorithm SHA256).Hash.ToLower())
Run-Verb 'servicedetect'  @('--output','json','service','detect')      30000
Run-Verb 'doctor'         @('--output','json','doctor')                60000
Run-Verb 'optimizestatus' @('--output','json','optimize','status')     30000
Run-Verb 'scanstatus'     @('--output','json','scan','status')         30000
Run-Verb 'insightslist'   @('--output','json','insights','list')      180000
Run-Verb 'selfcheck'      @('--output','json','self-check')            60000

W "=== [8] engineLabel ==="
$ins = Get-Content (Join-Path $vdir 'insightslist.out') -Raw -ErrorAction SilentlyContinue
if ($ins -and $ins -match '"engineLabel"\s*:\s*"([^"]+)"') { W ('ENGINE_LABEL=' + $Matches[1]) }
else { W 'ENGINE_LABEL=NOT_FOUND_IN_OUTPUT' }
W 'GATE2_LIVE_VERIFY_DONE'
