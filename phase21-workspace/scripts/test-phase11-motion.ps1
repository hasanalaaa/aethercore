[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Temp = Join-Path ([System.IO.Path]::GetTempPath()) ("aethercore-phase11-motion-" + [guid]::NewGuid().ToString('N'))
$Pushed = $false
New-Item -ItemType Directory -Force -Path $Temp | Out-Null
try {
    Push-Location (Join-Path $Root 'apps/ui')
    $Pushed = $true
    & pnpm exec tsc src/design/motion/physics.ts src/design/motion/spring.ts --target ES2022 --module commonjs --lib ES2022,DOM --skipLibCheck --outDir $Temp
    if ($LASTEXITCODE -ne 0) { throw 'Phase 11 motion sources did not compile.' }
    Pop-Location
    $Pushed = $false

    & node (Join-Path $Root 'scripts/phase11-motion-tests.cjs') $Temp
    if ($LASTEXITCODE -ne 0) { throw 'Phase 11 deterministic motion tests failed.' }
} finally {
    if ($Pushed) { Pop-Location }
    Remove-Item -LiteralPath $Temp -Recurse -Force -ErrorAction SilentlyContinue
}
