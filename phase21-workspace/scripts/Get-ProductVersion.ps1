[CmdletBinding()]
param()
# ---------------------------------------------------------------------------
# THE product version.
#
# [workspace.package].version in Cargo.toml is the single source of truth.
# Every crate carries `version.workspace = true`, so every binary reports that
# value through CARGO_PKG_VERSION. The MSI, the ARP entry, the CLI archive and
# the desktop bundle must all derive from here so a shipped binary can never
# report a different version than the installer that placed it.
#
# This file exists so the derivation is written ONCE. Before it, the regex
# lived only in build-release.ps1 and three other surfaces each decided the
# version independently -- which is how MSIs shipped as 0.1.2-0.1.6 carrying an
# aetherctl.exe that answered `about` with 0.1.0.
# ---------------------------------------------------------------------------
$ErrorActionPreference = 'Stop'
$cargoToml = Get-Content (Join-Path (Split-Path $PSScriptRoot -Parent) 'Cargo.toml') -Raw
if ($cargoToml -notmatch '(?ms)\[workspace\.package\].*?version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"') {
    throw 'Unable to read [workspace.package].version from Cargo.toml.'
}
$Matches[1]
