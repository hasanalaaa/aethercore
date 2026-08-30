$ErrorActionPreference = 'Continue'
Write-Output '=== does the inner probe output file exist in P36Admin temp (name mismatch?) ==='
Get-ChildItem 'C:\Users\P36Admin\AppData\Local\Temp' -Force | Select-Object -First 20 | ForEach-Object { Write-Output $_.Name }
Write-Output '=== from the inner script: outFile = Join-Path $env:TEMP p36-probe2-Admin.txt ==='
Write-Output '=== check C:\Users\P36STA~1 confusion: the earlier NO_OUTPUT for probe v1 Admin suggests TEMP path differs ==='
Get-ChildItem 'C:\Users' -Directory | ForEach-Object { Write-Output $_.Name }
