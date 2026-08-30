# AetherCore Zenith UI/UX Reconstruction

## Interaction model

Zenith applies the supplied fluid-interface specification as an engineering invariant rather than a visual theme. Pointer-down feedback remains immediate; pointer capture preserves continuous tracking; gesture-driven motion is spring-based and interruptible; release momentum is preserved only for intentional releases; cancellation settles without synthetic fling.

The press primitive now models the complete desktop tap contract: press immediately, remain armed inside hysteresis, disarm when dragged away, allow re-entry, and suppress the native activation if release occurs while disarmed. Keyboard Enter/Space remains native and is never blocked by the pointer-cancellation fence.

## Springs and reduced motion

Existing designer-facing spring parameters remain unchanged: critical damping for ordinary motion, slight under-damping only where momentum justifies it. `FluidDialog` now has exactly one close-completion source and an idempotent close guard, so reduced-motion hard synchronization cannot double-fire workflow callbacks. Reduced motion continues to remove spatial spring movement rather than removing state feedback.

## Materials and rendering performance

No new decorative glass, bounce, gradients, or card layers were added. Update progress was moved to the shared `ProgressBar`, which uses `transform: scaleX()` rather than animating layout width. This keeps frequent progress updates compositor-oriented and consolidates motion/accessibility behavior in one primitive.

## Accessibility

- Stable/Beta update channel buttons expose `aria-pressed`.
- Driver filter toggles expose `aria-pressed`.
- Startup decisions expose a true ARIA radio group with one tab stop, Arrow/Home/End navigation, protected-choice filtering, and RTL-aware horizontal arrow semantics.
- Update progress exposes `role=progressbar`, min/max/current value, and a localized label through the shared primitive.
- Dialog focus restoration remains preserved, now with exactly-once close finalization.
- Pointer cancellation never suppresses subsequent keyboard activation.

## RTL and Arabic parity

Guided-action spatial arrows are generated separately from localized content and explicitly mirror under RTL. Progress growth uses explicit LTR-left / RTL-right transform origins. Technical identifiers remain isolated by the existing technical-text/bidi system rather than being directionally mirrored as prose.

## Screen-level restraint

The pass intentionally did not reskin already coherent Overview/Repair/Cleanup/Startup/Activity surfaces. The only screen changes are those with a proven semantic or motion defect: System Care, Drivers, Hardware Diagnostics, and Crash Diagnostics. This follows the baseline-preservation rule: craft improvements are made where measurable, not for novelty.
