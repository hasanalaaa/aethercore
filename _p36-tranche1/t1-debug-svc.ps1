$ErrorActionPreference = 'Continue'
$inc = 'C:\AetherCore-P36\incoming'
$ws = 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery'
$envDump = & cmd.exe /d /s /c "`"C:\AetherCore-P36\toolchain\vs2022\Common7\Tools\VsDevCmd.bat`" -arch=arm64 -host_arch=arm64 && set" 2>&1 | Out-String
foreach ($line in ($envDump -split "`r?`n")) {
    if ($line -match '^([A-Za-z_][A-Za-z0-9_]*)=(.*)$') {
        $k = $Matches[1]; $v = $Matches[2]
        if ($k -ieq 'PATH') { $env:PATH = "$env:PATH;$v" } else { Set-Item -Path "env:$k" -Value $v }
    }
}
$env:LIBCLANG_PATH = 'C:\AetherCore-P36\toolchain\llvm-22.1.8\bin\libclang.dll'
$env:Path = 'C:\AetherCore-P36\toolchain\cmake-4.4.2\bin;C:\AetherCore-P36\toolchain\llvm-22.1.8\bin;C:\AetherCore-P36\toolchain\ninja-1.13.2-arm64;C:\Windows\System32\config\systemprofile\.cargo\bin;' + $env:Path
$ompDir = 'C:\AetherCore-P36\toolchain\vs2022\VC\Redist\MSVC\14.44.35112\debug_nonredist\arm64\Microsoft.VC143.OpenMP.LLVM'
if (Test-Path $ompDir) { $env:Path = "$ompDir;$env:Path" }
$env:LIB = 'C:\Program Files (x86)\Windows Kits\10\Assessment and Deployment Kit\Deployment Tools\SDKs\DismApi\Lib\arm64;' + $env:LIB
$env:RUSTFLAGS = '-C link-arg=libomp.lib'
$env:CARGO_INCREMENTAL = '0'
Set-Location $ws
Write-Output '=== rebuild service (debug) to get server-side logs, or check release binary matches? ==='
# Instrument: run a debug-build service in console mode and drive one request at it
cargo build --locked -p aethercore-maintenance-service *> "$inc\tranche1-svc-debug-build.log"
if ($LASTEXITCODE -ne 0) { Write-Output 'BUILD FAILED'; Get-Content "$inc\tranche1-reqprobe-build.log" -Tail 15; exit 1 }
Write-Output 'debug service built OK'
Write-Output '=== stop real service (to free the pipe name), start console service ==='
& sc.exe stop AetherCoreMaintenance 2>&1 | Out-Null
Start-Sleep 5
# console mode uses the same pipe; run it as a background process with stderr visible
$svcPsi = New-Object System.Diagnostics.ProcessStartInfo
$svcPsi.FileName = 'target\debug\aethercore-maintenance-service.exe'
$svcPsi.Arguments = '--console'
$svcPsi.UseShellExecute = $false
$svcPsi.RedirectStandardOutput = $true
$svcPsi.RedirectStandardError = $true
$svcPsi.WorkingDirectory = $ws
$svc = [System.Diagnostics.Process]::Start($svcPsi)
Start-Sleep 8
if ($svcPsi) {}
Write-Output ("service console running: " + (-not $svcPsi.HasExited))
Write-Output '=== drive one request at the console service ==='
& 'target\release\examples\ipc_request_probe.exe' 2>&1 | ForEach-Object { Write-Output $_ }
Start-Sleep 2
Write-Output '=== service console stderr (first 30 lines) ==='
$svcPsi2 = $null
