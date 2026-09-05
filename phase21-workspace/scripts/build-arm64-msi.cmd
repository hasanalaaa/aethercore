@echo off
setlocal enabledelayedexpansion
rem ===========================================================================
rem AetherCore ARM64 MSI — canonical, recorded build recipe (Phase 36 Stage A1)
rem ===========================================================================
rem WHY THIS FILE EXISTS
rem   scripts/build-release.ps1 is the production x64 release pipeline. It is
rem   NOT usable on the ARM64 qualification VM: it hard-codes -arch x64, and it
rem   requires the signed dependency-freeze baseline, the online supply-chain
rem   audit, and the Authenticode signer — none of which exist on this VM.
rem   This file is the ARM64 equivalent of the payload+package half of that
rem   pipeline, recorded so a build can never again be an ad-hoc invocation.
rem
rem TOOLCHAIN CONSTRAINT
rem   ARM64 MSVC is NOT supported by this dependency graph. The known-good
rem   environment is VsDevCmd -arch=arm64 with clang-cl as the C/C++ compiler,
rem   Ninja as the CMake generator, and libomp linked explicitly. This block is
rem   taken verbatim from the proven C:\AetherCore-P36\logs\p36_relbuild.cmd.
rem
rem INVOCATION ORDER AND WHY IT CANNOT BE ONE STEP
rem   [1] pnpm build           -> apps/ui/dist
rem   [2] cargo build --release -p <five native binaries>
rem   [3] tauri build --no-bundle -> aethercore-desktop.exe
rem   [3] cannot be merged into [2]: the Tauri CLI drives its OWN cargo
rem   invocation for apps/desktop and embeds the apps/ui/dist produced by [1]
rem   into the executable. Its cargo invocation therefore resolves features
rem   independently of [2], and it has a hard input dependency on [1].
rem   [2] is ONE invocation on purpose: cargo unifies features across every
rem   package selected in a single invocation, so the package SET is part of
rem   the recipe. Changing the set changes the emitted code. Do not split it.
rem
rem NOT A REPRODUCIBILITY CLAIM
rem   Byte-equality with any previously deployed binary is explicitly NOT a
rem   criterion. Correctness is proven functionally, by the service starting
rem   and the typed verbs returning.
rem ===========================================================================

set "SRC=C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery"
set "STAGE=C:\AetherCore-P36\build"
set "PAYLOAD=%STAGE%\payload"
set "OUT=%STAGE%\out"
rem --- version: THE SINGLE SOURCE OF TRUTH ----------------------------------
rem   [workspace.package].version in Cargo.toml is the ONE place the product
rem   version is declared. Every crate carries `version.workspace = true`, so
rem   every binary already reports it through CARGO_PKG_VERSION, and
rem   scripts/build-release.ps1 already derives from it.
rem   This script used to take an arbitrary argument defaulting to a hard-coded
rem   0.1.0. That is exactly how MSIs shipped as 0.1.2 - 0.1.6 while the
rem   aetherctl.exe inside them answered `about` with 0.1.0.
rem   An argument is still ACCEPTED so the recorded invocations in the phase
rem   docs keep working, but it must AGREE with Cargo.toml or the build stops.
rem   To change the product version, bump Cargo.toml. There is no other lever.
for /f "usebackq delims=" %%V in (`powershell -NoProfile -Command "$t=Get-Content -Raw '%SRC%\Cargo.toml'; if($t -match '(?ms)\[workspace\.package\].*?version\s*=\s*.([0-9]+\.[0-9]+\.[0-9]+).'){$Matches[1]}"`) do set "VERSION=%%V"
if not defined VERSION (
  echo ERROR: cannot read [workspace.package].version from %SRC%\Cargo.toml
  exit /b 1
)
if not "%~1"=="" (
  if not "%~1"=="%VERSION%" (
    echo ERROR: requested version %~1 disagrees with Cargo.toml %VERSION%
    echo        The product version has ONE source. Bump [workspace.package].version.
    exit /b 1
  )
)
echo Version=%VERSION%  ^(derived from Cargo.toml [workspace.package].version^)

rem --- ARM64 build environment (verbatim from p36_relbuild.cmd) --------------
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
set CARGO_INCREMENTAL=0

cd /d "%SRC%" || exit /b 1
if not exist "%PAYLOAD%" mkdir "%PAYLOAD%"
if not exist "%OUT%" mkdir "%OUT%"

rem --- [1] frontend ----------------------------------------------------------
echo === [1/7] apps/ui production build
call pnpm --dir apps/ui install --frozen-lockfile || exit /b 1
call pnpm --dir apps/ui build || exit /b 1

rem --- [2] native binaries, ONE invocation, FIXED package set ----------------
echo === [2/7] cargo release build (fixed package set)
cargo build --release ^
  -p aethercore-maintenance-service ^
  -p aethercore-consent-broker ^
  -p aethercore-update-broker ^
  -p aethercore-install-hardener ^
  -p aetherctl || exit /b 1

rem --- [3] Tauri desktop, separate invocation by necessity -------------------
rem   tauri.conf.json declares beforeBuildCommand "pnpm --dir ../ui build". The
rem   Tauri CLI runs that command from its own discovered app directory, not
rem   from the tauri.conf.json directory, so "../ui" does not resolve here and
rem   the build fails with ENOENT on <root>\ui. Step [1] has already produced
rem   apps/ui/dist, so the pre-build hook is redundant: it is disabled with a
rem   recorded config overlay rather than by changing tauri.conf.json, which is
rem   shared with the x64 release pipeline. frontendDist ("../ui/dist") is a
rem   config path and IS resolved relative to tauri.conf.json, so it still works.
echo === [3/7] tauri desktop build
pushd "%SRC%\apps\desktop" || exit /b 1
call "%SRC%\apps\ui\node_modules\.bin\tauri.cmd" build --no-bundle --config "%SRC%\installer\tauri.no-before-build.json"
if errorlevel 1 (popd & exit /b 1)
popd

rem --- [4] stage the MSI payload --------------------------------------------
rem   The MSI payload is the executables + libomp + update-trust.json authored in
rem   installer/wix/Product.wxs. The assets tree (model, licenses, vulndb) is NOT
rem   copied here — it is passed to wix as AssetsDir straight from the source tree.
echo === [4/7] stage payload
rem   P37 Stage 1: aetherctl.exe is now an authored MSI component. It was built in
rem   step [2] all along; it was simply never staged or authored, so a clean install
rem   put seven files on disk and the CLI was absent for a real user.
for %%F in (aethercore-desktop.exe aethercore-maintenance-service.exe aethercore-consent-broker.exe aethercore-update-broker.exe aethercore-install-hardener.exe aetherctl.exe) do (
  copy /y "%SRC%\target\release\%%F" "%PAYLOAD%\%%F" >nul || exit /b 1
)
rem   ARM64 OpenMP runtime. The clang-cl build imports libomp140.aarch64.dll;
rem   it must sit beside the service or SCM start fails 0xC0000135 / Error 1053.
copy /y "C:\AetherCore-P36\toolchain\vs2022\VC\Redist\MSVC\14.44.35112\debug_nonredist\arm64\Microsoft.VC143.OpenMP.LLVM\libomp140.aarch64.dll" "%PAYLOAD%\libomp140.aarch64.dll" >nul || exit /b 1
rem   Update trust ships DISABLED with zero channels. No private key is ever
rem   installed. Production substitutes a reviewed pinned channel config.
copy /y "%SRC%\release\update-trust.template.json" "%PAYLOAD%\update-trust.json" >nul || exit /b 1
rem   P37 Stage 2: the plain-language uninstall statement, installed beside the product.
copy /y "%SRC%\release\UNINSTALL.txt" "%PAYLOAD%\UNINSTALL.txt" >nul || exit /b 1

rem --- [5] package ----------------------------------------------------------
rem   ProductCode is a deterministic function of version and architecture,
rem   identical to the scheme in scripts/build-installer.ps1:
rem     first 16 bytes of SHA256("AetherCore/MSI/ProductCode/v1" +
rem                              "AetherCore/<version>/arm64")
rem   read as a .NET Guid. Same version => same ProductCode => a rebuild of the
rem   same version can only be a REINSTALL, never a major upgrade.
echo === [5/7] wix build -arch arm64
for /f "usebackq delims=" %%G in (`powershell -NoProfile -Command "$s=[Security.Cryptography.SHA256]::Create();$h=$s.ComputeHash([Text.Encoding]::UTF8.GetBytes('AetherCore/MSI/ProductCode/v1')+[Text.Encoding]::UTF8.GetBytes('AetherCore/%VERSION%/arm64'));([Guid]::new([byte[]]$h[0..15])).ToString('B').ToUpperInvariant()"`) do set "PRODUCTCODE=%%G"
echo ProductCode=%PRODUCTCODE%
set "MSI=%OUT%\AetherCore-%VERSION%-arm64.msi"
call dotnet tool restore || exit /b 1
rem   AssetsDir is sourced straight from the repo tree rather than copied into the
rem   payload: the embedded model alone is 1.07 GB and copying it per build buys
rem   nothing. Product.wxs reads it read-only at package time.
call dotnet tool run wix build installer\wix\Product.wxs -arch arm64 -o "%MSI%" -d "PayloadDir=%PAYLOAD%" -d "AssetsDir=%SRC%\assets" -d "ProductVersion=%VERSION%" -d "ProductCode=%PRODUCTCODE%" || exit /b 1
echo === [6/7] wix msi validate (zero ICE required, no suppression)
call dotnet tool run wix msi validate "%MSI%" || exit /b 1

rem --- [7] payload check ----------------------------------------------------
rem   Third occurrence of one class of mistake (DBT-P36-001, DBT-P36-008,
rem   DBT-P40-001): a developer binary believed harmless because "examples are
rem   not packaged". Measure it instead of believing it. Every File row must be
rem   authored in Product.wxs; anything else fails the build here, not in the
rem   field.
echo === [7/7] MSI payload check (no example / developer binaries)
powershell -NoProfile -ExecutionPolicy Bypass -File "%SRC%\scripts\check-msi-payload.ps1" -Msi "%MSI%" -Wxs "%SRC%\installer\wix\Product.wxs" || exit /b 1

rem --- manifest -------------------------------------------------------------
powershell -NoProfile -Command "Get-ChildItem '%PAYLOAD%','%OUT%' -File | ForEach-Object { '{0}  {1}  {2}' -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower(), $_.Length, $_.Name }" > "%STAGE%\MANIFEST.txt"
type "%STAGE%\MANIFEST.txt"
echo === BUILD OK: %MSI%
exit /b 0
