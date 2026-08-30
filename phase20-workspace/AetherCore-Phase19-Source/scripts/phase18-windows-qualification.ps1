param(
    [ValidateSet('all','machine-context','provider-network','authenticode','staging-acl','install-recovery','post-verify','gpu-utilities','direct-trusted','driver-ui')]
    [string]$Scenario = 'all'
)
$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'Phase 18 native qualification must run on Windows.' }

$repo = Split-Path -Parent $PSScriptRoot
Push-Location $repo
try {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw 'cargo is required for Phase 18 qualification.' }
    if (-not (Get-Command python -ErrorAction SilentlyContinue)) { throw 'python is required for Phase 18 source qualification.' }

    python scripts/phase18-driver-authority-audit.py
    cargo test -p aethercore-driver-authority
    cargo test -p aethercore-driver-acquisition
    cargo test -p aethercore-driver-hub
    cargo test -p aethercore-driver-install
    cargo test -p aethercore-windows-update
    cargo test -p aethercore-pc-intelligence
    cargo test -p aethercore-persistence
    cargo test -p aethercore-maintenance-service
    cargo test -p aethercore-contracts

    if (Get-Command npm -ErrorAction SilentlyContinue) {
        Push-Location (Join-Path $repo 'apps/ui')
        try { npm run check } finally { Pop-Location }
    } else {
        throw 'npm is required for Phase 18 UI source qualification.'
    }

    if ($Scenario -in @('all','machine-context')) {
        $bios = Get-ItemProperty 'HKLM:\HARDWARE\DESCRIPTION\System\BIOS'
        $nt = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
        [pscustomobject]@{
            SystemManufacturer = $bios.SystemManufacturer
            SystemProductName = $bios.SystemProductName
            SystemFamily = $bios.SystemFamily
            BaseBoardProduct = $bios.BaseBoardProduct
            ProductName = $nt.ProductName
            CurrentBuildNumber = $nt.CurrentBuildNumber
            DisplayVersion = $nt.DisplayVersion
        } | Format-List
        Write-Host 'Machine-context probe intentionally excludes serial number, UUID and asset-tag fields.' -ForegroundColor Cyan
    }

    if ($Scenario -in @('all','provider-network')) {
        $session = New-Object -ComObject Microsoft.Update.Session
        $searcher = $session.CreateUpdateSearcher()
        $result = $searcher.Search("IsInstalled=0 and Type='Driver'")
        Write-Host ("Live WUA driver offers discovered: {0}" -f $result.Updates.Count) -ForegroundColor Cyan
        Write-Host 'Capture offer IDs/revisions and compare them with AetherCore Driver Hub evidence before marking this scenario PASS.' -ForegroundColor Yellow
    }

    Write-Host "SOURCE/NATIVE TEST COMMANDS COMPLETED for scenario '$Scenario'." -ForegroundColor Green
    Write-Host 'This script does not by itself qualify Authenticode publisher identity, staging ACL attacks, real driver mutation/recovery, reboot, post-verification, GPU utility behavior, DirectTrusted handoff or WebView2 accessibility.' -ForegroundColor Yellow
    Write-Host 'Attach operator evidence matching QUALIFICATION_DEBT.json before converting any Phase 18 debt item to native PASS.' -ForegroundColor Yellow
} finally { Pop-Location }

# Phase 18.1 deferred native qualification hooks (NOT EXECUTED by source closure):
# - P18.1-QD-001 signer identity extraction / certificate identity evidence
# - P18.1-QD-002 provider-specific signer allowlist and chain policy
# - P18.1-QD-003 live OEM authority behavior and availability
# - P18.1-QD-004 live NVIDIA/AMD/Intel management-vs-update evidence
# - P18.1-QD-005 device-aware authority completeness under provider failure/offline states
# - P18.1-QD-006 staging ACL/reparse/hardlink/privileged revalidation adversarial tests
