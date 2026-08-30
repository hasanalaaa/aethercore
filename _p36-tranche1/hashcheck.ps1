$h = (Get-FileHash 'C:\AetherCore-P36\incoming\payload\aethercore-install-hardener.exe' -Algorithm SHA256).Hash
$src = (Get-FileHash 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery\apps\install-hardener\src\main.rs' -Algorithm SHA256).Hash
Write-Output ("hardener exe SHA256=" + $h)
Write-Output ("hardener main.rs VM SHA256=" + $src)
