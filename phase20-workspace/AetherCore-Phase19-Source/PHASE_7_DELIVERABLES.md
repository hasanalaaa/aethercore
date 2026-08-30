# AetherCore Phase 7 Deliverables

## Milestone

**Phase 7: Luxury UI/UX Polish, Design Tokens, Accessibility & Localization Readiness**

Phase 7 is a frontend/desktop-shell milestone only. It does not change Protobuf protocol v6, maintenance-service privileges, mutation-plan semantics, diagnostic truthfulness, or any Phase 0–6 safety boundary.

## Delivered implementation

### Unified design system

- Semantic `--ac-*` design tokens for typography, text hierarchy, Mica-compatible surfaces, borders, accent/status colors, geometry, spacing, elevation, focus, and motion.
- Layered stylesheet architecture: token system → retained structural legacy CSS → authoritative Phase 7 visual overrides.
- Consistent Fluent-style cards, controls, metrics, panels, dialogs, status pills, input treatments, and spacing across all eight destinations.
- Segoe UI Variable / Cascadia Mono typography strategy with no external font dependency.

### Navigation and product shell

- Single typed registry for all eight navigation destinations.
- New `NavigationRail.svelte` with local SVG iconography, active-page semantics, descriptions, service state, locale switcher, and keyboard roving behavior.
- New `CommandPalette.svelte` opened with `Ctrl+K`.
- Direct `Ctrl+Shift+1..8` destination shortcuts.
- Compact responsive icon-only rail for narrower logical windows.

### Native Windows desktop integration

- Tauri main window configured for Windows 11 Mica.
- Transparent WebView/window path with a solid/semitransparent fallback design layer.
- `noRedirectionBitmap` enabled to reduce transparent-window white-flash risk.
- Fluent overlay scrollbars requested where supported by WebView2.
- Window scale-factor and theme-change observers added through the Tauri window API.
- Logical minimum window reduced to 900×560 for High-DPI work areas.

### Accessibility

- Skip-to-main-content link.
- Explicit focus-visible token/system.
- Screen-reader current-page and service-state semantics.
- Alert live semantics for errors and polite announcements for shell navigation.
- Modal focus entry, Tab/Shift+Tab trapping, Escape behavior, and focus restoration.
- Reduced-motion handling.
- High-contrast / forced-colors fallbacks.
- Strict Phase 7 Svelte gate configured to fail on compiler warnings.

### Localization / Arabic readiness

- Typed English/Arabic shell catalog.
- Runtime `lang` and `dir` switching.
- Persisted locale preference.
- RTL logical-property shell layout.
- LTR isolation for bugchecks, hashes, SMART raw values, and technical identifiers.
- Architecture explicitly documents that legacy feature-body copy still requires a future full catalog extraction/translation pass.

### Performance discipline

- Mica delegated to the native compositor rather than simulated as a permanent full-window CSS blur.
- Permanent sidebar avoids heavy backdrop blur.
- Entrance animations use opacity/transform and honor reduced motion.
- Inline local SVG icons; no remote assets or icon library runtime.
- Existing backend polling cadence unchanged.

## New/changed key files

- `apps/ui/src/design-tokens.css`
- `apps/ui/src/luxury.css` (renamed in Phase 9)
- `apps/ui/src/foundation.css` (renamed in Phase 9)
- `apps/ui/src/styles.css`
- `apps/ui/src/components/AppIcon.svelte`
- `apps/ui/src/components/NavigationRail.svelte`
- `apps/ui/src/components/CommandPalette.svelte`
- `apps/ui/src/lib/navigation.ts`
- `apps/ui/src/lib/i18n.ts`
- `apps/ui/src/lib/window-ux.ts`
- `apps/ui/src/App.svelte`
- `apps/ui/index.html`
- `apps/desktop/tauri.conf.json`
- `scripts/verify-phase7.ps1`
- `scripts/static_validate.py`
- `.github/workflows/ci.yml`
- `docs/LUXURY_UI_UX.md`
- `docs/ACCESSIBILITY_LOCALIZATION.md`
- `docs/VALIDATION.md`

## Verification contract

Windows authoritative gate:

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\bootstrap.ps1 -InstallPrerequisites
.\scripts\verify-phase7.ps1
```

Optional read-only Phase 6 hardware probe can still be included:

```powershell
.\scripts\verify-phase7.ps1 -LiveTelemetry
```

`verify-phase7.ps1` preserves the full Phase 0–6 native/security gate, then adds strict Svelte accessibility/type diagnostics, production UI build, and Phase 7 static design/localization invariants.

The authoring runtime used for this source package is not Windows and cannot render WebView2/Mica or exercise physical multi-monitor DPI changes. Production sign-off therefore still requires Windows 11 visual, frame-pacing, Narrator, High Contrast, Arabic RTL, and multi-monitor tests.
