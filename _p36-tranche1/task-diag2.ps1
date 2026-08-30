$ErrorActionPreference = 'Continue'
Write-Output '=== Public dir probe files ==='
Get-ChildItem 'C:\Users\Public' -Filter 'p36-probe*' -Force -ErrorAction SilentlyContinue | ForEach-Object { Write-Output ($_.Name + '  ' + $_.Length + '  ' + $_.LastWriteTime) }
Write-Output '=== driver/test task remnants ==='
Get-ChildItem 'C:\Users\Public' -Filter 'probe-driver*' -Force -ErrorAction SilentlyContinue | ForEach-Object { Write-Output $_.Name }
Write-Output '=== scheduled tasks (P36*) ==='
schtasks /Query /FO LIST 2>$null | Select-String -Pattern 'P36' | ForEach-Object { Write-Output $_.Line }
Write-Output '=== P36StandardUser batch logon right check (who can batch logon) ==='
& secedit /export /cfg C:\Users\Public\secpol-now.inf 2>&1 | Out-Null
Get-Content 'C:\Users\Public\secpol-now.inf' | Select-String -Pattern 'SeBatchLogonRight|SeServiceLogonRight' | ForEach-Object { Write-Output $_.Line }
Remove-Item 'C:\Users\Public\secpol-now.inf' -Force -ErrorAction SilentlyContinue
