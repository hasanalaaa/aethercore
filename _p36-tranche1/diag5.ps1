# P36 - check system-wide presence of the service's non-system DLL imports, then test fix
$ErrorActionPreference = 'Continue'
Write-Output '=== System32 presence ==='
foreach ($d in @('MSVCP140.dll','VCRUNTIME140.dll','libomp140.aarch64.dll')) {
    Write-Output ("$d in System32: " + (Test-Path "C:\Windows\System32\$d"))
}
Write-Output '=== FIX TEST: copy libomp beside service exe in diag dir, rerun --console ==='
$diag = 'C:\AetherCore-P36\incoming\diag'
Copy-Item 'C:\AetherCore-P36\toolchain\vs2022\VC\Redist\MSVC\14.44.35112\debug_nonredist\arm64\Microsoft.VC143.OpenMP.LLVM\libomp140.aarch64.dll' "$diag\libomp140.aarch64.dll" -Force
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = "$diag\aethercore-maintenance-service.exe"
$psi.Arguments = '--console'
$psi.UseShellExecute = $false
$psi.RedirectStandardError = $true
$psi.RedirectStandardOutput = $true
$proc = [System.Diagnostics.Process]::Start($psi)
Start-Sleep 10
$exited = $proc.HasExited
if ($exited) { Write-Output ("EXITED code=0x" + ('{0:X8}' -f $proc.ExitCode)) } else { Write-Output ("STILL RUNNING pid=" + $proc.Id + " -> fix works; stopping") ; Stop-Process -Id $proc.Id -Force }
Start-Sleep 2
Write-Output '=== pipes now? ==='
[System.IO.Directory]::GetFiles('\\.\pipe\') | Where-Object { $_ -like '*aether*' } | ForEach-Object { Write-Output $_ }
Write-Output '=== service.jsonl? ==='
if (Test-Path 'C:\ProgramData\AetherCore\logs\service.jsonl') { Get-Content 'C:\ProgramData\AetherCore\logs\service.jsonl' -Tail 6 | ForEach-Object { Write-Output $_ } } else { Write-Output 'none' }
Write-Output '=== leftover service.exe process killed; diag dir contents ==='
Get-ChildItem $diag | ForEach-Object { Write-Output ($_.Name + '  ' + $_.Length) }
