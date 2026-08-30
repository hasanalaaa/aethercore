$cli = 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery\target\release\aetherctl.exe'
Write-Output '=== aetherctl doctor as SYSTEM ==='
& $cli doctor 2>&1 | Out-String | Write-Output
Write-Output ("exit=" + $LASTEXITCODE)
Write-Output '=== aetherctl about (offline sanity) ==='
& $cli about 2>&1 | Out-String | Write-Output
