$ErrorActionPreference = 'Continue'
$pf = 'C:\Program Files\AetherCore'
Write-Output '=== simulate MSI: copy payload to Program Files\AetherCore ==='
New-Item -ItemType Directory -Force $pf | Out-Null
Copy-Item 'C:\AetherCore-P36\incoming\payload\*' $pf -Force -Exclude 'payload-manifest.json'
Get-ChildItem $pf | ForEach-Object { Write-Output ($_.Name + '  ' + $_.Length) }

Write-Output '=== create ProgramData state dirs (as MSI CreateFolders would) ==='
New-Item -ItemType Directory -Force 'C:\ProgramData\AetherCore\state','C:\ProgramData\AetherCore\logs' | Out-Null

Write-Output '=== run REAL hardener apply (as MSI custom action would) ==='
& "$pf\aethercore-install-hardener.exe" apply 2>&1 | Out-String | Write-Output
Write-Output ("hardener exit: " + $LASTEXITCODE)

Write-Output '=== create service exactly as MSI ServiceInstall does ==='
& sc.exe create AetherCoreMaintenance binPath= "`"$pf\aethercore-maintenance-service.exe`"" type= own start= auto error= normal obj= LocalSystem DisplayName= "AetherCore Maintenance Service" 2>&1 | Out-String | Write-Output
& sc.exe description AetherCoreMaintenance "Privileged typed-operation engine for AetherCore." 2>&1 | Out-String | Write-Output

Write-Output '=== hardener SCM ops (sidtype/config/sdset), verbatim ==='
& sc.exe sidtype AetherCoreMaintenance unrestricted 2>&1 | Out-String | Write-Output
& sc.exe config AetherCoreMaintenance start= delayed-auto obj= LocalSystem 2>&1 | Out-String | Write-Output
& sc.exe sdset AetherCoreMaintenance "D:(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWLOCRRC;;;AU)" 2>&1 | Out-String | Write-Output

Write-Output '=== service file ACL after hardener ==='
& icacls "$pf\aethercore-maintenance-service.exe" | Out-String | Write-Output

Write-Output '=== start ==='
& sc.exe start AetherCoreMaintenance 2>&1 | Out-String | Write-Output
Start-Sleep 6
& sc.exe query AetherCoreMaintenance 2>&1 | Out-String | Write-Output
[System.IO.Directory]::GetFiles('\\.\pipe\') | Where-Object { $_ -like '*aether*' } | ForEach-Object { Write-Output $_ }
