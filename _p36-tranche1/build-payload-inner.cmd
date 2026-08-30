@echo off
REM P36 Tranche 1 - REAL ARM64 payload build (release). Runs detached on the VM as SYSTEM.
REM Step status files: C:\AetherCore-P36\incoming\tranche1-step<N>.status
setlocal EnableExtensions
call "C:\AetherCore-P36\toolchain\vs2022\Common7\Tools\VsDevCmd.bat" -arch=arm64 -host_arch=arm64
if errorlevel 1 (((echo EXIT=1))> "C:\AetherCore-P36\incoming\tranche1-step0.status" & exit /b 1)
((echo EXIT=0))> "C:\AetherCore-P36\incoming\tranche1-step0.status"

powershell.exe -NoProfile -ExecutionPolicy Bypass -Command ^
  "$ErrorActionPreference='Stop';" ^
  "$env:LIBCLANG_PATH='C:\AetherCore-P36\toolchain\llvm-22.1.8\bin\libclang.dll';" ^
  "$env:Path='C:\AetherCore-P36\toolchain\cmake-4.4.2\bin;C:\AetherCore-P36\toolchain\llvm-22.1.8\bin;C:\AetherCore-P36\toolchain\ninja-1.13.2-arm64;C:\Windows\System32\config\systemprofile\.cargo\bin;C:\AetherCore-P36\toolchain\node-v24.20.0;C:\Windows\System32\config\systemprofile\AppData\Roaming\npm;' + $env:Path;" ^
  "$ompDir='C:\AetherCore-P36\toolchain\vs2022\VC\Redist\MSVC\14.44.35112\debug_nonredist\arm64\Microsoft.VC143.OpenMP.LLVM';" ^
  "if(Test-Path $ompDir){$env:Path=$ompDir+';'+$env:Path};" ^
  "$dism='C:\Program Files (x86)\Windows Kits\10\Assessment and Deployment Kit\Deployment Tools\SDKs\DismApi\Lib\arm64';" ^
  "$env:LIB=$dism+';'+$env:LIB;" ^
  "$env:RUSTFLAGS='-C link-arg=libomp.lib';" ^
  "$env:CARGO_INCREMENTAL='0';" ^
  "$env:CMAKE_GENERATOR='Ninja';$env:CMAKE_C_COMPILER='clang-cl';$env:CMAKE_CXX_COMPILER='clang-cl';" ^
  "$env:CC_aarch64_pc_windows_msvc='clang-cl';$env:CXX_aarch64_pc_windows_msvc='clang-cl';" ^
  "$env:CXXFLAGS_aarch64_pc_windows_msvc='/EHsc';$env:CXXFLAGS='/EHsc';" ^
  "Set-Location 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery';" ^
  "$env:DOTNET_ROOT='C:\AetherCore-P36\toolchain\dotnet';$env:PATH=$env:DOTNET_ROOT+';'+$env:PATH;" ^
  "Write-Host ('ENV RUSTFLAGS='+$env:RUSTFLAGS);" ^
  "cargo build --locked --release -p aethercore-maintenance-service -p aethercore-consent-broker -p aethercore-update-broker -p aethercore-install-hardener *> 'C:\AetherCore-P36\incoming\tranche1-cargo-release.log';" ^
  "$ec=$LASTEXITCODE; Set-Content 'C:\AetherCore-P36\incoming\tranche1-step1.status' ('EXIT='+$ec);" ^
  "if($ec -ne 0){exit $ec};" ^
  "Set-Location 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery\apps\desktop';" ^
  "$tauriCli='C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery\node_modules\.pnpm\@tauri-apps+cli@2.11.4\node_modules\@tauri-apps\cli\tauri.js';" ^
  "node $tauriCli build --no-bundle *> 'C:\AetherCore-P36\incoming\tranche1-tauri-build.log';" ^
  "$ec2=$LASTEXITCODE; Set-Content 'C:\AetherCore-P36\incoming\tranche1-step2.status' ('EXIT='+$ec2);" ^
  "if($ec2 -ne 0){exit $ec2};" ^
  "Set-Location 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery';" ^
  "$exes=@('aethercore-desktop.exe','aethercore-maintenance-service.exe','aethercore-consent-broker.exe','aethercore-update-broker.exe','aethercore-install-hardener.exe');" ^
  "$manifest=@();$fail=$false;" ^
  "foreach($e in $exes){$p=Join-Path 'target\release' $e; if(Test-Path $p){$h=(Get-FileHash $p -Algorithm SHA256).Hash;$sz=(Get-Item $p).Length;$arch='';$ro=& 'C:\AetherCore-P36\toolchain\llvm-22.1.8\bin\llvm-readobj.exe' --file-headers $p 2>$null | Select-String 'Machine:'; if($ro){$arch=$ro[0].ToString().Trim()};$manifest+=[ordered]@{file=$e;sha256=$h;bytes=$sz;pe=$arch}}else{$manifest+=[ordered]@{file=$e;MISSING=$true};$fail=$true}}" ^
  "New-Item -ItemType Directory -Force 'C:\AetherCore-P36\incoming\payload' | Out-Null;" ^
  "Copy-Item 'release\update-trust.template.json' 'C:\AetherCore-P36\incoming\payload\update-trust.json' -Force;" ^
  "$th=(Get-FileHash 'C:\AetherCore-P36\incoming\payload\update-trust.json' -Algorithm SHA256).Hash;" ^
  "$manifest+=[ordered]@{file='update-trust.json';source='release\update-trust.template.json';sha256=$th;bytes=83};" ^
  "$manifest | ConvertTo-Json -Depth 4 | Set-Content 'C:\AetherCore-P36\incoming\payload\payload-manifest.json';" ^
  "if($fail){Set-Content 'C:\AetherCore-P36\incoming\tranche1-step3.status' 'EXIT=1';exit 1}else{Set-Content 'C:\AetherCore-P36\incoming\tranche1-step3.status' 'EXIT=0'};" ^
  "Get-Content 'C:\AetherCore-P36\incoming\payload\payload-manifest.json'"
if errorlevel 1 (((echo EXIT=1))> "C:\AetherCore-P36\incoming\tranche1-master.status" & exit /b 1)
((echo EXIT=0))> "C:\AetherCore-P36\incoming\tranche1-master.status"
