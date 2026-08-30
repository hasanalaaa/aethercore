# Compatibility entry point retained for older Phase 10 notes.
# The authoritative source gate is phase10-architecture-audit.ps1.
& (Join-Path $PSScriptRoot 'phase10-architecture-audit.ps1') @args
exit $LASTEXITCODE
