$ErrorActionPreference = 'Continue'
Write-Output '=== P36Admin temp dir probe files ==='
Get-ChildItem 'C:\Users\P36Admin\AppData\Local\Temp' -Filter 'p36*' -ErrorAction SilentlyContinue | ForEach-Object { Write-Output $_.Name }
Get-ChildItem 'C:\Windows\Temp' -Filter 'p36-probe2*' -ErrorAction SilentlyContinue | ForEach-Object { Write-Output $_.Name }
Write-Output '=== P36Admin account state ==='
net user P36Admin | Select-String -Pattern 'Account active|Local Group|Password last' | ForEach-Object { Write-Output $_ }
