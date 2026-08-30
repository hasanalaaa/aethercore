[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$Root = Split-Path $PSScriptRoot -Parent
Set-Location $Root
$devData = Join-Path $Root '.devdata'
New-Item -ItemType Directory -Force -Path $devData | Out-Null
$env:AETHERCORE_DATA_DIR = $devData
$env:RUST_LOG = 'info'
# Give this debug launch a private 128-bit rendezvous name. Release builds ignore this path.
$devPipeToken = [Guid]::NewGuid().ToString('N').ToLowerInvariant()
$env:AETHERCORE_DEV_PIPE_TOKEN = $devPipeToken

& cargo build -p aethercore-maintenance-service -p aethercore-consent-broker
if ($LASTEXITCODE -ne 0) { throw 'Native build failed' }

$serviceExe = Join-Path $Root 'target\debug\aethercore-maintenance-service.exe'
Write-Host 'Starting the maintenance service console elevated. The Tauri desktop remains non-elevated.' -ForegroundColor Cyan
$serviceArgs = @('--console', '--data-dir', ('"{0}"' -f $devData), '--dev-pipe-token', $devPipeToken)
$service = Start-Process -FilePath $serviceExe -ArgumentList $serviceArgs -Verb RunAs -PassThru
try {
    Start-Sleep -Milliseconds 600
    Push-Location (Join-Path $Root 'apps\desktop')
    try {
        $tauri = Join-Path $Root 'apps\ui\node_modules\.bin\tauri.cmd'
        & $tauri dev
    } finally { Pop-Location }
} finally {
    Remove-Item Env:AETHERCORE_DEV_PIPE_TOKEN -ErrorAction SilentlyContinue
    if ($service -and -not $service.HasExited) { Stop-Process -Id $service.Id -Force -ErrorAction SilentlyContinue }
}
