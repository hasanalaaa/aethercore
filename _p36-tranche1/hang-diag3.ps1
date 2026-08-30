$ErrorActionPreference = 'Continue'
Write-Output '=== remaining powershell processes ==='
Get-Process -Name powershell -ErrorAction SilentlyContinue | ForEach-Object {
    $owner = Invoke-CimMethod -Query "SELECT ProcessId,CommandLine FROM Win32_Process WHERE ProcessId=$($_.Id)" -ErrorAction SilentlyContinue
    Write-Output ("pid=" + $_.Id + " cmd=" + $owner.CommandLine)
}
Write-Output '=== any P36 task still registered ==='
Get-ScheduledTask -TaskName 'P36*' -ErrorAction SilentlyContinue | ForEach-Object { Write-Output ($_.TaskName + ' state=' + $_.State) }
