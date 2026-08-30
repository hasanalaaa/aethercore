$ErrorActionPreference = 'Continue'
$evid = 'C:\AetherCore-P36\evidence'
$out = Join-Path $evid 'tranche1-acl-evidence.json'
$v = [ordered]@{}
$v.collectedAt = (Get-Date -Format o)
$v.collectorContext = 'SYSTEM (prlctl exec) - read-only ACL inspection'

function Add-Acl($v, $name, $path) {
    if (Test-Path $path) {
        $acl = Get-Acl $path
        $v[$name] = [ordered]@{
            path = $path
            sddl = $acl.Sddl
            owner = $acl.Owner
            icacls = (& icacls.exe $path 2>&1 | Out-String).Trim()
        }
    } else {
        $v[$name] = [ordered]@{ path = $path; MISSING = $true }
    }
}

Write-Output '=== install dir + critical executables ==='
Add-Acl $v 'installDir' 'C:\Program Files\AetherCore'
Add-Acl $v 'serviceExe' 'C:\Program Files\AetherCore\aethercore-maintenance-service.exe'
Add-Acl $v 'desktopExe' 'C:\Program Files\AetherCore\aethercore-desktop.exe'
Add-Acl $v 'consentBrokerExe' 'C:\Program Files\AetherCore\aethercore-consent-broker.exe'
Add-Acl $v 'updateBrokerExe' 'C:\Program Files\AetherCore\aethercore-update-broker.exe'
Add-Acl $v 'installHardenerExe' 'C:\Program Files\AetherCore\aethercore-install-hardener.exe'
Add-Acl $v 'openmpDll' 'C:\Program Files\AetherCore\libomp140.aarch64.dll'

Write-Output '=== update-trust material ==='
Add-Acl $v 'updateTrustJson' 'C:\Program Files\AetherCore\update-trust.json'

Write-Output '=== ProgramData AetherCore (machine mutation/state surfaces) ==='
Add-Acl $v 'programDataDir' 'C:\ProgramData\AetherCore'
Add-Acl $v 'programDataState' 'C:\ProgramData\AetherCore\state'
Add-Acl $v 'machineMutationLock' 'C:\ProgramData\AetherCore\state\machine-mutation.lock'
Add-Acl $v 'programDataLogs' 'C:\ProgramData\AetherCore\logs'

Write-Output '=== service security descriptor (SCM) ==='
$v.serviceSd = (& sc.exe sdshow AetherCoreMaintenance 2>&1 | Out-String).Trim()

Write-Output '=== named pipe presence ==='
$pipes = [System.IO.Directory]::GetFiles('\\.\pipe\') | Where-Object { $_ -like '*aether*' }
$v.pipes = @($pipes)

Write-Output '=== service-exe rules visible to Users/service SID ==='
$svcAcl = Get-Acl 'C:\Program Files\AetherCore\aethercore-maintenance-service.exe'
$v.serviceExeRules = @($svcAcl.Access | ForEach-Object { $_.IdentityReference.ToString() + ' :: ' + $_.FileSystemRights.ToString() + ' :: ' + $_.AccessControlType.ToString() })

$v | ConvertTo-Json -Depth 6 | Set-Content $out
Write-Output 'ACL-EVIDENCE-WRITTEN'
Write-Output ("installDir SDDL: " + $v['installDir'].sddl)
Write-Output ("serviceExe SDDL: " + $v['serviceExe'].sddl)
Write-Output ("updateTrust SDDL: " + $v['updateTrustJson'].sddl)
Write-Output ("programDataDir SDDL: " + $v['programDataDir'].sddl)
Write-Output ("machineMutationLock SDDL: " + $v['machineMutationLock'].sddl)
