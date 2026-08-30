[CmdletBinding()]param()
$ErrorActionPreference='Stop';$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path;Set-Location $Root
Write-Host 'Running Phase 15 signed-manifest, upload, mutation and support-bundle tamper regressions...' -ForegroundColor Cyan
& cargo test --locked -p aethercore-update-engine -p aethercore-update-download -p aethercore-support-bundle
if($LASTEXITCODE -ne 0){throw 'Phase 15 cryptographic/unit regressions failed.'}
& cargo check --locked -p aethercore-update-manifest-tool -p aethercore-support-bundle-verify -p aethercore-update-broker -p aethercore-desktop
if($LASTEXITCODE -ne 0){throw 'Phase 15 update/export tool compile gate failed.'}
Write-Host 'Phase 15 cryptographic and orchestration regressions passed.' -ForegroundColor Green
