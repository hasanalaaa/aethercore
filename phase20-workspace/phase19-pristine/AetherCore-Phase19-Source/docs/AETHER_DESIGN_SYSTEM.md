# Aether Design System — Phase 11

## Purpose

Phase 11 replaces the accumulated Phase 7 presentation overrides with a maintainable interaction system. The design target is not a macOS imitation: AetherCore remains a Windows 11 desktop application using Mica, Segoe UI Variable, native keyboard conventions and WebView2. The Apple Fluid Interfaces influence is behavioral: immediate response, direct manipulation, interruptible presentation-value motion, velocity continuity, spatial consistency and restraint.

The privileged trust model is unchanged. The design system runs entirely in the non-elevated Tauri/WebView renderer and consumes the principal-bound IPC v7 event stream introduced in Phase 10.

## Frontend architecture

`App.svelte` is intentionally only the composition entry point. `app/AppShell.svelte` owns navigation, global shortcuts, transient surfaces and the persistent kernel session. Product state is reduced in `platform/stream-state.ts`; feature-local interaction state stays in each feature controller.

Feature boundaries:

- `features/overview`
- `features/drivers`
- `features/repair`
- `features/cleanup`
- `features/startup`
- `features/diagnostics`
- `features/activity`

No feature owns a polling timer. `platform/kernel-session.ts` subscribes to `aethercore://kernel-event`, `aethercore://session-state` and `aethercore://stream-reset`; `reduceKernelEvent` is the canonical renderer state transition boundary. One-shot invoke calls remain valid for user-triggered commands and immediate request/response confirmation; they are not used as a polling mechanism.

## Physical motion model

The internal runtime lives under `design/motion` and has no external animation dependency.

### Spring defaults

`SpringValue` uses designer-facing response and damping values. The normal UI default is critically damped:

- response: `0.36 s`
- damping ratio: `1.0`

Momentum-driven direct manipulation uses:

- response: `0.34 s`
- damping ratio: `0.82`

The runtime integrates with bounded semi-implicit Euler substeps no larger than 1/120 s. A stalled renderer cannot integrate an arbitrarily large frame delta.

### Interruptibility

`retarget()` changes only the target. It does not reset the live presentation value or current velocity. `adoptPresentation()` lets a pointer-driven gesture hand a directly manipulated position and measured velocity into the same spring. This removes the drag/release seam and makes mid-flight reversal continuous.

### Momentum projection

`physics.ts` uses exponential scroll-style projection:

`projected = current + (velocity / 1000) * rate / (1 - rate)`

with a default deceleration rate of `0.998`. `fluidDrag` projects the release endpoint, selects the nearest valid snap point, and passes the measured release velocity to the spring. Bounds use progressive rubber-banding rather than hard stopping.

Direct manipulation is deliberately opt-in. Maintenance controls that do not gain meaning from dragging are not given decorative swipe behavior.

## Tactile primitives

- `Pressable.svelte` / `fluidPress` — pointer-down response, pointer capture, 10 px hysteresis, drag-away cancellation/re-arm, keyboard parity and critically damped release.
- `FluidDialog.svelte` — reversible live spring, source-aware transform origin, focus trap, Escape, focus restoration, closing lifetime and materialization blur.
- `FluidPage.svelte` — restrained spring entrance for route content.
- `ProgressBar.svelte` — spring interpolation only when the native engine provides a real percentage; unknown progress remains indeterminate.
- `MaterialSurface.svelte` — semantic material level.
- `DragSurface.svelte` / `fluidDrag` — reusable direct-manipulation primitive with velocity handoff and projected snapping.

Every raw product `<button>` is attached to `fluidPress`; components may use `Pressable` where a reusable semantic wrapper is more appropriate.

## Material hierarchy

The native Tauri window owns long-lived Windows 11 Mica. Web content then uses four semantic material levels:

1. structural — navigation / major chrome;
2. base — ordinary content grouping;
3. elevated — floating but non-modal utility surfaces;
4. focused — modal review and command surfaces.

The focused dialog material scales, translates, changes opacity and changes blur from one live spring presentation value. The scrim and underlying material arrive together. Persistent nested glass is avoided; ordinary feature cards remain lighter-weight surfaces over the Mica-backed page.

When transparency is reduced, CSS removes backdrop filtering and switches surfaces to near-solid backgrounds. Forced colors use Windows system colors.

## Typography

The type system is semantic rather than page-specific:

- display
- title
- headline
- body
- callout
- caption
- technical

Segoe UI Variable Display/Text are preferred and `font-optical-sizing` is enabled. Large headings use tighter tracking; small text does not inherit display tracking. Technical evidence uses Cascadia Mono/Code when available and is bidi-isolated.

The Phase 11 gate rejects literal 7–11 px font sizes in the design-system styles.

## Accessibility and adaptability

The renderer observes four independent conditions:

- `prefers-reduced-motion`
- `prefers-reduced-transparency`
- `prefers-contrast: more`
- `forced-colors: active`

Reduced motion removes spring travel/overshoot but keeps state feedback. Reduced transparency removes blur rather than making the entire UI visually disappear. High contrast and forced colors replace subtle glass boundaries with explicit system-color separation.

Dialogs retain keyboard focus, trap Tab/Shift+Tab, close with Escape, and restore the prior focus target after the closing spring settles. The command palette remains keyboard-first. Primary navigation remains Arrow/Home/End navigable.

## RTL and technical evidence

Phase 11 uses logical properties for the new shell, material and navigation layers. The command palette changes its materialization origin in Arabic. Technical values remain `direction:ltr` with `unicode-bidi:isolate` so hashes, versions and crash identifiers are not corrupted by bidi reordering.

Phase 11 originally deferred complete feature-body translation to Phase 12. Phase 12 now supersedes that limitation with typed full-product EN/AR catalogs and bidi-isolated technical evidence; the Phase 11 design-system behavior remains unchanged.

## Motion performance rules

- User-driven position/scale motion is owned by `requestAnimationFrame` springs.
- Interactive transforms are not prescribed by fixed-duration CSS transitions.
- Transform/opacity/translate are used for frame-sensitive presentation motion.
- Repeating keyframes are reserved for unknown-progress/scanning status and are disabled under reduced motion.
- No renderer polling timers are allowed.
- UI progress events are transient Phase 10 telemetry, not SQLite frame-by-frame writes.

## Verification

`verify-phase11.ps1` inherits the complete Phase 10 native/security/release gate, then runs:

1. `phase11-design-audit.ps1`;
2. deterministic compiled spring/momentum/interruption tests;
3. `svelte-check --fail-on-warnings` through the package script;
4. production UI build;
5. the Phase 0–11 platform-neutral invariant gate.

Static/source gates cannot certify subjective motion quality. Release qualification still requires Windows 11 physical review at 60/120/144 Hz, keyboard-only navigation, Narrator, reduced motion/transparency, forced colors, 100–200% DPI, mixed-DPI monitor movement and Arabic RTL.

## Zenith interaction resilience

- Pointer-captured pressables implement cancel-by-dragging-away: the visible `data-pressed` state and the eventual native activation are one state machine, not separate behaviors.
- `pointercancel` and lost capture settle direct-manipulation primitives without projecting stale momentum. Only an actual user release hands velocity into a spring.
- Reduced-motion dialog synchronization uses the same idempotent close-completion path as animated settlement, preventing duplicate callbacks and focus restoration.
- Progress presentation uses transform-based spring motion with explicit LTR/RTL transform origins.
- Gesture primitives remain interruptible; no animation-running flag blocks new input.
