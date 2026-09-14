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
    # DBT-P65-004: `--lib ES2022,DOM` unquoted. A bareword containing a comma is an
    # ARRAY LITERAL to the PowerShell parser, and run 34831042672 shows what reaches
    # tsc is a single argument with a SPACE where the comma was:
    #   error TS6046: Argument for '--lib' option must be: 'es5', 'es6', ...
    # Reproduced exactly on macOS with `--lib "ES2022 DOM"`, and NOT reproduced by
    # the two-argument form `--lib ES2022 DOM`, which gives TS6231 instead. Quoting
    # makes it one literal argument, which tsc accepts (verified: emits physics.js
    # and spring.js).
    #
    # `--ignoreConfig` is the SECOND half and neither half works alone. tsc 6 stops
    # at option parsing, so TS6046 hid TS5112 -- "tsconfig.json is present but will
    # not be loaded if files are specified on commandline" -- which this invocation
    # earns by naming files explicitly, and which emits nothing and exits non-zero.
    # The Windows log has 1 TS6046 and 0 TS5112 for exactly that reason. Fixing only
    # the quoting would have moved the wall one error, not past it. The flag is the
    # one tsc's own message names.
    & pnpm exec tsc src/design/motion/physics.ts src/design/motion/spring.ts --target ES2022 --module commonjs --lib 'ES2022,DOM' --skipLibCheck --ignoreConfig --outDir $Temp
    if ($LASTEXITCODE -ne 0) { throw 'Phase 11 motion sources did not compile.' }
    Pop-Location
    $Pushed = $false

    & node (Join-Path $Root 'scripts/phase11-motion-tests.cjs') $Temp
    if ($LASTEXITCODE -ne 0) { throw 'Phase 11 deterministic motion tests failed.' }
} finally {
    if ($Pushed) { Pop-Location }
    Remove-Item -LiteralPath $Temp -Recurse -Force -ErrorAction SilentlyContinue
}
