$ErrorActionPreference = 'Continue'
$inc = 'C:\AetherCore-P36\incoming'
$ws = 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery'
Set-Location $ws
Write-Output '=== build verb probe ==='
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
cargo build --locked --release -p aethercore-ipc --example ipc_verb_probe *> "$inc\tranche1-verbprobe-build.log"
if ($LASTEXITCODE -ne 0) { Write-Output 'BUILD FAILED'; Get-Content "$inc\tranche1-verbprobe-build.log" -Tail 12; exit 1 }
Copy-Item "$inc\payload\libomp140.aarch64.dll" 'target\release\examples\' -Force
Write-Output '=== verb: optstatus (immediate Ok, no provider work) ==='
& 'target\release\examples\ipc_verb_probe.exe' optstatus 2>&1 | ForEach-Object { Write-Output $_ }
Write-Output '=== verb: carestatus (state read) ==='
& 'target\release\examples\ipc_verb_probe.exe' carestatus 2>&1 | ForEach-Object { Write-Output $_ }
