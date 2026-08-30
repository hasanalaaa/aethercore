[CmdletBinding()]
param([string]$Output='out/omega-rust-coverage.json')
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $Root
& python (Join-Path $PSScriptRoot 'check-dependency-freeze.py')
if($LASTEXITCODE -ne 0){throw 'Coverage is not meaningful until the dependency freeze is approved.'}
if(-not (Get-Command cargo -ErrorAction SilentlyContinue)){throw 'cargo is required.'}
& cargo llvm-cov --workspace --all-targets --locked --json --output-path $Output
if($LASTEXITCODE -ne 0){throw 'cargo llvm-cov failed (install the reviewed/pinned llvm-cov tool on the qualification host).'}
& python (Join-Path $PSScriptRoot 'omega-coverage-gate.py') --coverage $Output
if($LASTEXITCODE -ne 0){throw 'Critical Rust coverage policy failed.'}
