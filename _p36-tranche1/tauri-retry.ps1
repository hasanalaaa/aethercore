# P36 Tranche 1 - step 2 retry: UI build + Tauri desktop build (ARM64)
# Root cause of first failure: tauri.conf.json beforeBuildCommand 'pnpm --dir ../ui build'
# resolves against the workspace root; UI actually lives in apps/ui.
# Remedy (invocation-level, mirrors scripts/build-release.ps1 order):
#   1. pnpm --dir apps/ui build
#   2. tauri build --no-bundle with beforeBuildCommand overridden to empty
$ErrorActionPreference = 'Continue'
$inc = 'C:\AetherCore-P36\incoming'
$ws = 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery'

# Rebuild the ARM64 env exactly as build-payload.ps1 (rustc must re-link the desktop app)
$envDump = & cmd.exe /d /s /c "`"C:\AetherCore-P36\toolchain\vs2022\Common7\Tools\VsDevCmd.bat`" -arch=arm64 -host_arch=arm64 && set" 2>&1 | Out-String
foreach ($line in ($envDump -split "`r?`n")) {
    if ($line -match '^([A-Za-z_][A-Za-z0-9_]*)=(.*)$') {
        $k = $Matches[1]; $v = $Matches[2]
        if ($k -ieq 'PATH') { $env:PATH = "$env:PATH;$v" } else { Set-Item -Path "env:$k" -Value $v }
    }
}
$env:LIBCLANG_PATH = 'C:\AetherCore-P36\toolchain\llvm-22.1.8\bin\libclang.dll'
$env:Path = 'C:\AetherCore-P36\toolchain\cmake-4.4.2\bin;C:\AetherCore-P36\toolchain\llvm-22.1.8\bin;C:\AetherCore-P36\toolchain\ninja-1.13.2-arm64;C:\Windows\System32\config\systemprofile\.cargo\bin;C:\AetherCore-P36\toolchain\node-v24.20.0;C:\Windows\System32\config\systemprofile\AppData\Roaming\npm;' + $env:Path
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
$env:DOTNET_ROOT = 'C:\AetherCore-P36\toolchain\dotnet'
$env:PATH = "$env:DOTNET_ROOT;$env:PATH"

# Step 2a: UI production build (frozen; node_modules already restored in a prior gate)
Set-Location (Join-Path $ws 'apps\ui')
$pnpm = 'C:\Windows\System32\config\systemprofile\AppData\Roaming\npm\pnpm.cmd'
if (-not (Test-Path $pnpm)) { $pnpm = (Get-Command pnpm.cmd -ErrorAction SilentlyContinue).Source }
if (-not $pnpm) { Set-Content "$inc\tranche1-step2.status" 'EXIT=1 (pnpm.cmd not found)'; Set-Content "$inc\tranche1-master.status" 'EXIT=1 (pnpm missing)'; exit 1 }
& $pnpm build *> "$inc\tranche1-ui-build.log"
$ecui = $LASTEXITCODE
Set-Content "$inc\tranche1-step2a.status" "EXIT=$ecui"
if ($ecui -ne 0) { Set-Content "$inc\tranche1-master.status" "EXIT=1 (ui build $ecui)"; exit 1 }

# Step 2b: Tauri desktop build, beforeBuildCommand overridden (UI already built above)
Set-Location (Join-Path $ws 'apps\desktop')
$tauriCli = Join-Path $ws 'node_modules\.pnpm\@tauri-apps+cli@2.11.4\node_modules\@tauri-apps\cli\tauri.js'
$cfg = '{"build":{"beforeBuildCommand":""}}'
node $tauriCli build --no-bundle --config $cfg *> "$inc\tranche1-tauri-build.log"
$ec2 = $LASTEXITCODE
Set-Content "$inc\tranche1-step2.status" "EXIT=$ec2"
if ($ec2 -ne 0) { Set-Content "$inc\tranche1-master.status" "EXIT=1 (tauri $ec2)"; exit 1 }

# Step 3: assemble payload dir, hash + PE-arch every file
Set-Location $ws
$readobj = 'C:\AetherCore-P36\toolchain\llvm-22.1.8\bin\llvm-readobj.exe'
$exes = @('aethercore-desktop.exe','aethercore-maintenance-service.exe','aethercore-consent-broker.exe','aethercore-update-broker.exe','aethercore-install-hardener.exe')
$manifest = @(); $fail = $false
New-Item -ItemType Directory -Force "$inc\payload" | Out-Null
foreach ($e in $exes) {
    $p = Join-Path 'target\release' $e
    if (Test-Path $p) {
        Copy-Item $p "$inc\payload\$e" -Force
        $h = (Get-FileHash "$inc\payload\$e" -Algorithm SHA256).Hash
        $sz = (Get-Item "$inc\payload\$e").Length
        $ro = & $readobj --file-headers "$inc\payload\$e" 2>$null | Select-String 'Machine:'
        $arch = if ($ro) { $ro[0].ToString().Trim() } else { 'READOBJ_NO_OUTPUT' }
        $manifest += [ordered]@{ file = $e; sha256 = $h; bytes = $sz; pe = $arch }
    } else {
        $manifest += [ordered]@{ file = $e; MISSING = $true }; $fail = $true
    }
}
Copy-Item 'release\update-trust.template.json' "$inc\payload\update-trust.json" -Force
$th = (Get-FileHash "$inc\payload\update-trust.json" -Algorithm SHA256).Hash
$manifest += [ordered]@{ file = 'update-trust.json'; source = 'release\update-trust.template.json'; sha256 = $th; bytes = (Get-Item "$inc\payload\update-trust.json").Length }
$manifest | ConvertTo-Json -Depth 4 | Set-Content "$inc\payload\payload-manifest.json"
if ($fail) { Set-Content "$inc\tranche1-step3.status" 'EXIT=1'; Set-Content "$inc\tranche1-master.status" 'EXIT=1 (missing exe)'; exit 1 }
Set-Content "$inc\tranche1-step3.status" 'EXIT=0'
Set-Content "$inc\tranche1-master.status" 'EXIT=0'
Get-Content "$inc\payload\payload-manifest.json"
