# ADR 0013 — Aether Design System and Fluid Motion Runtime

## Status

Accepted — Phase 11.

## Context

By Phase 10 the backend was event-driven and modular, while the renderer still concentrated all eight product surfaces in one large Svelte component and relied on accumulated Phase 7 CSS transitions/override layers. That structure made interaction state, presentation state and domain state unnecessarily coupled and made physical interruption/velocity continuity impractical.

## Decision

1. Keep the Windows-native visual identity: Tauri/WebView2, Windows 11 Mica, Segoe UI Variable and Windows keyboard/accessibility conventions.
2. Decompose the renderer into a small shell, a canonical IPC-event reducer, feature modules and feature-local UI controllers.
3. Ban renderer polling for domain state; persistent IPC v7 events are the live source of truth.
4. Own an internal scalar spring runtime instead of adding an animation dependency to the release supply chain.
5. Default interactive springs to damping ratio 1.0; reserve under-damping for momentum-derived direct manipulation.
6. Retarget from live presentation values and preserve velocity. Direct gestures may explicitly adopt presentation value and hand release velocity into the spring.
7. Use Apple-style exponential momentum projection and progressive rubber-banding only where a drag interaction is semantically useful.
8. Centralize tactile controls and transient surfaces in reusable primitives.
9. Use Mica as the native base and a small semantic material hierarchy above it. Do not stack decorative translucent surfaces without hierarchy.
10. Treat reduced motion, reduced transparency, increased contrast and forced colors as separate adaptations.
11. Keep technical LTR evidence bidi-isolated. Full product text extraction/localization is deferred to Phase 12.

## Consequences

- `App.svelte` is no longer a domain monolith.
- Motion behavior is deterministic and testable independently of Svelte components.
- All product buttons receive pointer-down tactile response.
- Dialogs can be reversed while in flight and return focus only after their closing presentation settles.
- Feature progress no longer depends on CSS durations; determinate motion follows native telemetry and unknown progress stays explicitly indeterminate.
- The project carries a small amount of in-house physics code, so deterministic numerical regression tests are mandatory.
- Static/compiler gates still cannot replace physical 60/120/144 Hz motion review on Windows hardware.
