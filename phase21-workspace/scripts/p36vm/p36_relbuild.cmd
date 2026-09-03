@echo off
call "C:\AetherCore-P36\toolchain\vs2022\Common7\Tools\VsDevCmd.bat" -arch=arm64 -host_arch=arm64 >nul
set PATH=C:\AetherCore-P36\toolchain\cmake-4.4.2\bin;C:\AetherCore-P36\toolchain\ninja-1.13.2-arm64;C:\AetherCore-P36\toolchain\llvm-22.1.8\bin;%PATH%
set LIBCLANG_PATH=C:\AetherCore-P36\toolchain\llvm-22.1.8\bin
set CC_aarch64_pc_windows_msvc=clang-cl
set CXX_aarch64_pc_windows_msvc=clang-cl
set CMAKE_GENERATOR=Ninja
set CXXFLAGS=/EHsc
set CXXFLAGS_aarch64_pc_windows_msvc=/EHsc
set RUSTFLAGS=-C link-arg=libomp.lib
set LIB=C:\Program Files (x86)\Windows Kits\10\Assessment and Deployment Kit\Deployment Tools\SDKs\DismApi\Lib\arm64;%LIB%
cd /d "C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery"
cargo build --release -p aethercore-maintenance-service -p aetherctl
if errorlevel 1 exit /b 1
