[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Ui = Join-Path $Root 'apps/ui/src'

function Require-File([string]$Relative) {
    $Path = Join-Path $Root $Relative
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "Missing Phase 11 artifact: $Relative" }
}
function Require-Text([string]$Relative, [string[]]$Needles) {
    Require-File $Relative
    $Text = Get-Content -LiteralPath (Join-Path $Root $Relative) -Raw
    foreach ($Needle in $Needles) { if (-not $Text.Contains($Needle)) { throw "$Relative missing invariant: $Needle" } }
}

$Required = @(
    'apps/ui/src/app/AppShell.svelte','apps/ui/src/app/PlanDialogs.svelte','apps/ui/src/app/shell-state.ts',
    'apps/ui/src/platform/stream-state.ts','apps/ui/src/platform/kernel-session.ts',
    'apps/ui/src/design/motion/spring.ts','apps/ui/src/design/motion/physics.ts','apps/ui/src/design/motion/fluid-press.ts','apps/ui/src/design/motion/fluid-drag.ts',
    'apps/ui/src/design/primitives/Pressable.svelte','apps/ui/src/design/primitives/FluidDialog.svelte','apps/ui/src/design/primitives/ProgressBar.svelte','apps/ui/src/design/primitives/MaterialSurface.svelte','apps/ui/src/design/primitives/DragSurface.svelte',
    'apps/ui/src/design/styles/base.css','apps/ui/src/design/styles/materials.css','apps/ui/src/design/styles/navigation.css','apps/ui/src/design/styles/motion.css','apps/ui/src/design/styles/typography.css','apps/ui/src/design/styles/responsive.css',
    'apps/ui/src/features/overview/OverviewPage.svelte','apps/ui/src/features/drivers/DriversPage.svelte','apps/ui/src/features/repair/RepairPage.svelte','apps/ui/src/features/cleanup/CleanupPage.svelte','apps/ui/src/features/startup/StartupPage.svelte','apps/ui/src/features/diagnostics/HardwarePage.svelte','apps/ui/src/features/diagnostics/CrashPage.svelte','apps/ui/src/features/activity/ActivityPage.svelte'
)
$Required | ForEach-Object { Require-File $_ }

$appLines = (Get-Content -LiteralPath (Join-Path $Ui 'App.svelte')).Count
if ($appLines -gt 20) { throw "App.svelte regressed to a monolith ($appLines lines)." }
if (Test-Path (Join-Path $Ui 'luxury.css')) { throw 'Legacy luxury.css override layer must remain removed.' }
if (Test-Path (Join-Path $Ui 'foundation.css')) { throw 'Legacy foundation.css layer must remain removed.' }

$Product = Get-ChildItem -Path $Ui -Recurse -File | Where-Object { $_.Extension -in '.ts','.svelte','.css' }
$Text = ($Product | ForEach-Object { Get-Content -LiteralPath $_.FullName -Raw }) -join "`n"
if ($Text -match '\bsetInterval\s*\(' -or $Text -match '\bclearInterval\s*\(') { throw 'Renderer polling timer reintroduced.' }
if ($Text.Contains('!important')) { throw '!important is forbidden in the Phase 11 UI design system.' }
if ($Text -match 'transition\s*:[^;]*(transform|all)') { throw 'Fixed-duration transform/all transitions are forbidden for interactive motion.' }
if ($Text -match 'font-size\s*:\s*(7|8|9|10|11)px') { throw 'Sub-12px literal UI typography reintroduced.' }
# DBT-P65-002: this swept $Text, which includes .css. A `<button>` ELEMENT cannot
# be authored in a stylesheet, so every match there is prose -- and four of them
# were: three in feature-layout.css's comment about Pressable nesting and one in
# motion.css's, all written in P48 on 2026-09-05. Run 34827684392 failed step 18
# here, at `phase11-design-audit.ps1:40`, on a comment. The sweep is scoped to the
# file types that can contain markup; the checks above keep .css because
# `!important`, fixed-duration transitions and sub-12px type are CSS defects.
# Residual, stated rather than fixed: `<button>` inside a .svelte or .ts COMMENT
# would still trip this. There are none today (measured: 80 tags, 0 violations),
# so stripping comments would be machinery for a case that does not exist.
$Markup = ($Product | Where-Object { $_.Extension -in '.ts','.svelte' } |
    ForEach-Object { Get-Content -LiteralPath $_.FullName -Raw }) -join "`n"
$ButtonTags = [regex]::Matches($Markup, '<button\b.*?>', [System.Text.RegularExpressions.RegexOptions]::Singleline)
foreach ($Tag in $ButtonTags) {
    if (-not $Tag.Value.Contains('use:fluidPress')) { throw "Raw button without pointer-down spring tactility: $($Tag.Value.Substring(0, [Math]::Min(120, $Tag.Value.Length)))" }
}

Require-Text 'apps/ui/src/design/motion/spring.ts' @('damping: 1','response: 0.36','adoptPresentation','retarget(target','Semi-implicit Euler')
Require-Text 'apps/ui/src/design/motion/physics.ts' @('decelerationRate = 0.998','projectedEndpoint','rubberband','estimateVelocity','nearestSnapPoint')
Require-Text 'apps/ui/src/design/motion/fluid-press.ts' @('pointerdown','setPointerCapture','hysteresis ?? 10','data-pressed','retarget')
Require-Text 'apps/ui/src/design/motion/fluid-drag.ts' @('projectedEndpoint','estimateVelocity','rubberband','adoptPresentation','retarget(target, velocity)')
Require-Text 'apps/ui/src/design/primitives/FluidDialog.svelte' @('aria-modal="true"','dialogKeydown','returnFocus','onClosed','spring.retarget')
Require-Text 'apps/ui/src/lib/window-ux.ts' @('prefers-reduced-motion','prefers-reduced-transparency','prefers-contrast','forced-colors')
Require-Text 'apps/ui/src/design/styles/materials.css' @('var(--ac-material-structural)','backdrop-filter','[data-transparency="reduced"]','ac-material-focused')
Require-Text 'apps/ui/src/design/styles/typography.css' @('font-optical-sizing','letter-spacing','[dir="rtl"]','unicode-bidi:isolate')
Require-Text 'apps/ui/src/platform/kernel-session.ts' @('aethercore://kernel-event','aethercore://session-state','aethercore://stream-reset','start_ipc_session')

Write-Host 'Phase 11 design-system source audit passed.' -ForegroundColor Green
