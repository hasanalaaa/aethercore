# P36 - rebuild service + ipc tests, run ipc targeted tests, restart service, re-probe pipe
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
$env:CMAKE_GENERATOR = 'Ninja'
$env:CMAKE_C_COMPILER = 'clang-cl'
$env:CMAKE_CXX_COMPILER = 'clang-cl'
$env:CC_aarch64_pc_windows_msvc = 'clang-cl'
$env:CXX_aarch64_pc_windows_msvc = 'clang-cl'
$env:CXXFLAGS_aarch64_pc_windows_msvc = '/EHsc'
$env:CXXFLAGS = '/EHsc'
Set-Location $ws

Write-Output '=== cargo test -p aethercore-ipc --locked ==='
cargo test -p aethercore-ipc --locked *> "$inc\tranche1-ipc-test2.log"
$ecT = $LASTEXITCODE
Write-Output "ipc tests exit=$ecT"
Get-Content "$inc\tranche1-ipc-test2.log" | Select-String 'test result' | Select-Object -Last 3 | ForEach-Object { Write-Output $_.Line }
if ($ecT -ne 0) { Get-Content "$inc\tranche1-ipc-test2.log" -Tail 30 | ForEach-Object { Write-Output $_ }; exit 1 }

Write-Output '=== rebuild service release ==='
cargo build --locked --release -p aethercore-maintenance-service *> "$inc\tranche1-svc-rebuild.log"
$ecS = $LASTEXITCODE
Write-Output "service rebuild exit=$ecS"
if ($ecS -ne 0) { Get-Content "$inc\tranche1-svc-rebuild.log" -Tail 20 | ForEach-Object { Write-Output $_ }; exit 1 }
$ro = & 'C:\AetherCore-P36\toolchain\llvm-22.1.8\bin\llvm-readobj.exe' --file-headers 'target\release\aethercore-maintenance-service.exe' 2>$null | Select-String 'Machine:'
Write-Output ($ro[0].ToString().Trim())

Write-Output '=== stop service, swap binary, restart ==='
& sc.exe stop AetherCoreMaintenance 2>&1 | Out-Null
Start-Sleep 5
Copy-Item 'target\release\aethercore-maintenance-service.exe' 'C:\Program Files\AetherCore\aethercore-maintenance-service.exe' -Force
& sc.exe start AetherCoreMaintenance 2>&1 | Out-String | Write-Output
Start-Sleep 6
& sc.exe query AetherCoreMaintenance 2>&1 | Select-String 'STATE' | ForEach-Object { Write-Output $_.Line.Trim() }

Write-Output '=== SYSTEM pipe probe (exact client mask) ==='
& 'target\release\examples\ipc_probe.exe' 2>&1 | Out-String | Write-Output
