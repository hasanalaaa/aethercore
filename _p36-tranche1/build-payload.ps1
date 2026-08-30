# P36 Tranche 1 - REAL ARM64 payload build (release). Launched detached as SYSTEM on the VM.
$ErrorActionPreference = 'Continue'
$inc = 'C:\AetherCore-P36\incoming'

# Step 0: VsDevCmd environment (ARM64)
$vsdevcmd = 'C:\AetherCore-P36\toolchain\vs2022\Common7\Tools\VsDevCmd.bat'
if (-not (Test-Path $vsdevcmd)) { Set-Content "$inc\tranche1-step0.status" 'EXIT=1 (VsDevCmd missing)'; exit 1 }
# Import VsDevCmd env by running cmd and capturing SET output
$envDump = & cmd.exe /d /s /c "`"$vsdevcmd`" -arch=arm64 -host_arch=arm64 && set" 2>&1 | Out-String
if ($LASTEXITCODE -ne 0) { Set-Content "$inc\tranche1-step0.status" "EXIT=1 (VsDevCmd failed $LASTEXITCODE)"; exit 1 }
foreach ($line in ($envDump -split "`r?`n")) {
    if ($line -match '^([A-Za-z_][A-Za-z0-9_]*)=(.*)$') {
        $k = $Matches[1]; $v = $Matches[2]
        if ($k -ieq 'PATH') { $env:PATH = "$env:PATH;$v" } else { Set-Item -Path "env:$k" -Value $v }
    }
}
Set-Content "$inc\tranche1-step0.status" 'EXIT=0'

# Pinned ARM64 toolchain environment (handoff section 7 - reused verbatim)
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

Set-Location 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery'

# Step 1: cargo release build of the five native payload components
cargo build --locked --release -p aethercore-maintenance-service -p aethercore-consent-broker -p aethercore-update-broker -p aethercore-install-hardener *> "$inc\tranche1-cargo-release.log"
$ec = $LASTEXITCODE
Set-Content "$inc\tranche1-step1.status" "EXIT=$ec"
if ($ec -ne 0) { Set-Content "$inc\tranche1-master.status" "EXIT=1 (cargo $ec)"; exit 1 }

# Step 2: Tauri desktop build (frontend dist must exist; build.rs embeds it)
Set-Location 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery\apps\desktop'
$tauriCli = 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery\node_modules\.pnpm\@tauri-apps+cli@2.11.4\node_modules\@tauri-apps\cli\tauri.js'
node $tauriCli build --no-bundle *> "$inc\tranche1-tauri-build.log"
$ec2 = $LASTEXITCODE
Set-Content "$inc\tranche1-step2.status" "EXIT=$ec2"
if ($ec2 -ne 0) { Set-Content "$inc\tranche1-master.status" "EXIT=1 (tauri $ec2)"; exit 1 }

# Step 3: assemble payload dir, hash + PE-arch every file
Set-Location 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery'
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
