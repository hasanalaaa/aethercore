[CmdletBinding()]
param(
    [switch]$LibFuzzer,
    [ValidateRange(1,3600)][int]$MaxTotalTimeSeconds = 30
)
$ErrorActionPreference = 'Stop'
$Root = Split-Path $PSScriptRoot -Parent
Set-Location $Root

# Always run the deterministic malformed-frame corpus in normal CI.
& (Join-Path $PSScriptRoot 'invoke-cargo-test-case.ps1') -Package aethercore-ipc -TestName deterministic_malformed_frame_corpus_never_panics
if ($LASTEXITCODE -ne 0) { throw 'Deterministic IPC malformed-frame test failed.' }

if ($LibFuzzer) {
    & cargo fuzz --version *> $null
    if ($LASTEXITCODE -ne 0) {
        & cargo install --locked cargo-fuzz --version 0.13.2
        if ($LASTEXITCODE -ne 0) { throw 'cargo-fuzz installation failed.' }
    }
    & cargo fuzz run ipc_frame --fuzz-dir fuzz -- -max_total_time=$MaxTotalTimeSeconds -timeout=5
    if ($LASTEXITCODE -ne 0) { throw 'IPC libFuzzer run failed.' }
}
Write-Host 'IPC fuzz/validation gate passed.' -ForegroundColor Green
