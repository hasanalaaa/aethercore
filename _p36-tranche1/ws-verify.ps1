# P36 Tranche 1 - workspace byte verification + tool env probe (SYSTEM context, read-only)
$ErrorActionPreference = 'Continue'
$ws = 'C:\AetherCore-P36\workspace\AetherCore-Phase35-Master-Delivery'
$evid = 'C:\AetherCore-P36\evidence'
$logs = 'C:\AetherCore-P36\logs'
$out = Join-Path $logs 'tranche1-workspace-verify.txt'
"=== P36 TRANCHE1 WORKSPACE VERIFY ===" | Out-File $out -Encoding utf8
"Context: $(whoami)  Date: $(Get-Date -Format o)" | Out-File $out -Append -Encoding utf8

$files = @(
  'installer\wix\Product.wxs',
  'release\update-trust.template.json',
  'scripts\build-installer.ps1',
  'Cargo.lock',
  'services\maintenance-service\src\main.rs',
  'services\maintenance-service\src\router.rs',
  'services\maintenance-service\src\protocol.rs',
  'apps\desktop\icons\icon.ico'
)
foreach ($f in $files) {
  $p = Join-Path $ws $f
  if (Test-Path $p) {
    $h = (Get-FileHash $p -Algorithm SHA256).Hash
    $sz = (Get-Item $p).Length
    "$f  SHA256=$h  BYTES=$sz" | Out-File $out -Append -Encoding utf8
  } else {
    "$f  MISSING" | Out-File $out -Append -Encoding utf8
  }
}

"`n--- dotnet / wix probe ---" | Out-File $out -Append -Encoding utf8
$dotnet = 'C:\AetherCore-P36\toolchain\dotnet\dotnet.exe'
if (Test-Path $dotnet) { "dotnet found: $dotnet" | Out-File $out -Append -Encoding utf8; & $dotnet --version 2>&1 | Out-File $out -Append -Encoding utf8 } else { "dotnet NOT at $dotnet" | Out-File $out -Append -Encoding utf8; Get-Command dotnet -ErrorAction SilentlyContinue | Out-String | Out-File $out -Append -Encoding utf8 }

"`n--- wix tool probe (workspace dotnet-tools.json) ---" | Out-File $out -Append -Encoding utf8
Push-Location $ws
$env:DOTNET_ROOT = 'C:\AetherCore-P36\toolchain\dotnet'
$env:PATH = "$env:DOTNET_ROOT;$env:PATH"
& $dotnet tool restore 2>&1 | Select-Object -Last 3 | Out-File $out -Append -Encoding utf8
& $dotnet tool run wix --version 2>&1 | Out-File $out -Append -Encoding utf8
"wix_exit=$LASTEXITCODE" | Out-File $out -Append -Encoding utf8
Pop-Location

"`n--- cargo metadata version probe ---" | Out-File $out -Append -Encoding utf8
$gt = Join-Path $ws 'Cargo.toml'
if (Test-Path $gt) { (Get-Content $gt | Select-String -Pattern '^\s*version\s*=' | Select-Object -First 3) | Out-File $out -Append -Encoding utf8 }

"=== END WORKSPACE VERIFY ===" | Out-File $out -Append -Encoding utf8
Get-Content $out
