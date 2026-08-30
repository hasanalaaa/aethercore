$ErrorActionPreference = 'Continue'
# Root cause candidates: probe2-inner exits early (Add-Type or Add-Type 32/64?) or errors silently.
# Wrapper already captured 'inner-ok' so the script RAN to completion. But no output file.
# => Out-File $outFile at top should ALWAYS create the file... unless $env:TEMP is not writable
# or the args[0] was empty (label missing -> file named p36-probe2-.txt)
Write-Output '=== check for p36-probe2-.txt (empty label) ==='
Get-ChildItem 'C:\Users\P36Admin\AppData\Local\Temp' -Filter 'p36-probe2*' -Force | ForEach-Object { Write-Output $_.Name }
Get-ChildItem 'C:\Windows\Temp' -Filter 'p36-probe2*' -Force | ForEach-Object { Write-Output $_.Name }
Write-Output '=== check script content reaching the VM (encoding of args handling) ==='
Get-Content 'C:\Users\Public\probe2-inner.ps1' -Total 6
