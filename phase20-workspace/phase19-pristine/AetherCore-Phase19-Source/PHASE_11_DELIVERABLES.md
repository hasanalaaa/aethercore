# Phase 11 Deliverables — Aether Design System & Apple Fluid Motion

## Milestone result

Phase 11 transforms the Phase 10 renderer from a monolithic presentation shell into a modular event-driven frontend and establishes a physical, interruptible interaction system without changing privileged service authority or IPC v7 trust boundaries.

## Architecture delivered

- `apps/ui/src/App.svelte` reduced to the application composition entry.
- `app/AppShell.svelte` owns shell composition, navigation shortcuts, persistent session lifecycle and transient surfaces.
- `platform/stream-state.ts` is the canonical renderer reducer for principal-scoped IPC v7 events.
- Domain UI is split across Overview, Drivers, Repair, Cleanup, Startup, Hardware, Crash and Activity feature modules.
- Drivers/Repair/Cleanup/Startup/Diagnostics controllers isolate user interaction state from service state.
- Renderer domain polling remains eliminated.

## Motion and interaction runtime

- Internal `SpringValue` with response/damping parameters and bounded 120 Hz integration substeps.
- Critically damped default spring: response 0.36, damping 1.0.
- Momentum spring: response 0.34, damping 0.82.
- Live presentation-value retargeting without velocity reset.
- `adoptPresentation()` for direct gesture→spring continuity.
- Exponential momentum projection at default deceleration rate 0.998.
- Velocity estimation, nearest-snap projection and progressive rubber-banding.
- `fluidPress` provides immediate pointer-down feedback, pointer capture, hysteresis and cancel/re-arm semantics.
- `fluidDrag` provides opt-in 1:1 direct manipulation and release-velocity handoff.

## Primitive suite

- `Pressable.svelte`
- `FluidDialog.svelte`
- `FluidPage.svelte`
- `ProgressBar.svelte`
- `MaterialSurface.svelte`
- `DragSurface.svelte`

Every raw product button is also attached to the same `fluidPress` behavior. Direct gestures remain opt-in rather than being added decoratively to maintenance workflows.

## Materials, typography and accessibility

- Four-level semantic material hierarchy over the native Windows 11 Mica backdrop.
- Focused transient material changes opacity, scale, position and blur from one live spring value.
- Command palette materialization origin mirrors between English and Arabic.
- Semantic Segoe UI Variable type scale with optical sizing and bidi-isolated technical text.
- No literal 7–11 px design-system typography.
- No `!important` UI overrides.
- No fixed-duration `transform`/`translate`/`all` transitions for interactive motion.
- Independent reduced-motion, reduced-transparency, increased-contrast and forced-colors adaptations.
- Dialog focus trap, Escape, focus return after physical close, skip link and keyboard navigation preserved.

## Verification delivered

- `scripts/phase11-design-audit.ps1`
- `scripts/test-phase11-motion.ps1`
- `scripts/phase11-motion-tests.cjs`
- `scripts/verify-phase11.ps1`
- Phase 0–11 static gate expanded with Phase 11 design/motion/layout invariants.
- Windows CI and signed release workflows now invoke `verify-phase11.ps1`.

The authoring environment validates source structure, TypeScript semantics (with dependency stubs), Svelte control-block/script syntax and deterministic compiled motion physics. Full Svelte package checking/build and Windows visual/runtime qualification remain authoritative through `verify-phase11.ps1` because this Linux authoring runtime has no installed pnpm dependency graph/WebView2/Windows presentation stack.

## Required physical Windows qualification

Before GA promotion, exercise Phase 11 on supported Windows 11 hardware with:

- 60 Hz, 120 Hz and 144 Hz displays;
- 100%, 125%, 150%, 175% and 200% DPI;
- mixed-DPI multi-monitor movement;
- keyboard-only navigation and Narrator;
- reduced motion;
- transparency disabled/reduced;
- forced colors/high contrast;
- English LTR and Arabic RTL with mixed Latin technical evidence;
- repeated mid-flight dialog open/close reversal and rapid page navigation;
- pointer, touch and pen where available.

These physical checks judge presentation quality and platform integration; they do not weaken or replace the inherited Phase 0–10 security, mutation, installer and release gates.
