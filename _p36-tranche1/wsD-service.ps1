$ErrorActionPreference = 'Continue'
$evid = 'C:\AetherCore-P36\evidence'
$out = Join-Path $evid 'tranche1-service-registration.json'
$v = [ordered]@{}

Write-Output '=== service query ==='
$q = (& sc.exe query AetherCoreMaintenance 2>&1 | Out-String).Trim()
Write-Output $q
$v.query = $q

Write-Output '=== service config (qc) ==='
$qc = (& sc.exe qc AetherCoreMaintenance 2>&1 | Out-String).Trim()
Write-Output $qc
$v.config = $qc

Write-Output '=== qsidtype ==='
$qsid = (& sc.exe qsidtype AetherCoreMaintenance 2>&1 | Out-String).Trim()
Write-Output $qsid
$v.sidType = $qsid

Write-Output '=== showsid (exact service SID) ==='
$ss = (& sc.exe showsid AetherCoreMaintenance 2>&1 | Out-String).Trim()
Write-Output $ss
$v.showSid = $ss

Write-Output '=== sdshow (service security descriptor) ==='
$sd = (& sc.exe sdshow AetherCoreMaintenance 2>&1 | Out-String).Trim()
Write-Output $sd
$v.securityDescriptor = $sd

Write-Output '=== process identity: is the running process the installed exe, running as SYSTEM? ==='
$svc = Get-Service AetherCoreMaintenance
$v.status = $svc.Status.ToString()
$v.startType = $svc.StartType.ToString()
$procInfo = Get-CimInstance Win32_Service -Filter "Name='AetherCoreMaintenance'"
$v.processId = $procInfo.ProcessId
$v.processExePath = $procInfo.PathName
$v.ownProcessType = $procInfo.ProcessType
$v.account = $procInfo.StartName
if ($procInfo.ProcessId -gt 0) {
    $p = Get-Process -Id $procInfo.ProcessId -ErrorAction SilentlyContinue
    if ($p) { $v.processImagePath = $p.Path }
}

Write-Output '=== pipe exists and is owned by the service process ==='
$pipes = [System.IO.Directory]::GetFiles('\\.\pipe\') | Where-Object { $_ -like '*aether*' }
$v.pipes = @($pipes)
Write-Output ($pipes -join ', ')

Write-Output '=== service JSON log tail (structured evidence) ==='
$log = 'C:\ProgramData\AetherCore\logs\service.jsonl'
if (Test-Path $log) { $v.serviceLogTail = @(Get-Content $log -Tail 5) ; Get-Content $log -Tail 5 | ForEach-Object { Write-Output $_ } }

Write-Output '=== binary resides in machine-owned dir ==='
$v.binaryDir = 'C:\Program Files\AetherCore'
$v.binaryInProgramFiles = (Test-Path 'C:\Program Files\AetherCore\aethercore-maintenance-service.exe')

$v | ConvertTo-Json -Depth 4 | Set-Content $out
Write-Output 'SERVICE-EVIDENCE-WRITTEN'
