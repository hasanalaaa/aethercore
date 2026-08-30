$ErrorActionPreference = 'Continue'
$src = 'C:\Users\P36Admin\AppData\Local\Temp\p36-probe2-StandardUser.txt'
$dst = 'C:\AetherCore-P36\evidence\tranche1-userprobe2-Admin.txt'
if (Test-Path $src) {
    Copy-Item $src $dst -Force
    Write-Output '=== Admin probe output (label was hardcoded to StandardUser; whoami line proves context) ==='
    Get-Content $dst
} else {
    Write-Output 'STILL MISSING'
}
