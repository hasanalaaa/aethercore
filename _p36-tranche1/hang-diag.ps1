$ErrorActionPreference = 'Continue'
Write-Output '=== stuck aetherctl process under StandardUser ==='
$p = Get-Process -Name aetherctl -ErrorAction SilentlyContinue
if ($p) {
    $p | ForEach-Object {
        Write-Output ("pid=" + $_.Id + " start=" + $_.StartTime)
        $owner = Invoke-CimMethod -Query "SELECT Handle FROM Win32_Process WHERE ProcessId=$($_.Id)" -MethodName GetOwner -ErrorAction SilentlyContinue
        if ($owner) { Write-Output ("owner=" + $owner.Domain + '\' + $owner.User) }
    }
} else { Write-Output '(none)' }
Write-Output '=== remaining probe processes ==='
Get-Process -Name powershell -ErrorAction SilentlyContinue | ForEach-Object { Write-Output ("pid=" + $_.Id + " start=" + $_.StartTime) }
Write-Output '=== evidence file tail (last write time) ==='
(Get-Item 'C:\AetherCore-P36\evidence\tranche1-userprobe-StandardUser.txt').LastWriteTime
