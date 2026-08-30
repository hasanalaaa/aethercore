# P36 Tranche 1 - rebuild hardener after ACL defect fix, refresh payload, then MSI+install
$ErrorActionPreference = 'Continue'
$inc = 'C:\AetherCore-P36\incoming'
$ws = 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery'

# ARM64 env (same as payload build)
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
$dism = 'C:\Program Files (x86)\Windows Kits\10\Assessment and Deployment Kit\Deployment Tools\SDKs\DismApi\Lib\arm64'
$env:LIB = "$dism;$env:LIB"
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
cargo build --locked --release -p aethercore-install-hardener *> "$inc\tranche1-hardener-rebuild.log"
$ec = $LASTEXITCODE
Set-Content "$inc\tranche1-stepH.status" "EXIT=$ec"
if ($ec -ne 0) { Write-Output "hardener rebuild FAILED"; Get-Content "$inc\tranche1-hardener-rebuild.log" -Tail 15; exit 1 }

Copy-Item 'target\release\aethercore-install-hardener.exe' "$inc\payload\aethercore-install-hardener.exe" -Force
$h = (Get-FileHash "$inc\payload\aethercore-install-hardener.exe" -Algorithm SHA256).Hash
$ro = & 'C:\AetherCore-P36\toolchain\llvm-22.1.8\bin\llvm-readobj.exe' --file-headers "$inc\payload\aethercore-install-hardener.exe" 2>$null | Select-String 'Machine:'
Write-Output ("hardener rebuilt: SHA256=$h " + $ro[0].ToString().Trim())
Write-Output 'PAYLOAD-READY'
