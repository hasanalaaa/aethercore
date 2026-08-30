# AetherCore UI/UX architecture — Phase 11

Phase 11 supersedes the Phase 7 fixed-duration presentation model. The current authoritative design/interaction contract is [`AETHER_DESIGN_SYSTEM.md`](AETHER_DESIGN_SYSTEM.md) and ADR 0013.

The retained product intent is restrained Windows-native luxury: Windows 11 Mica as the long-lived native backdrop, Segoe UI Variable typography, compact information density, clear evidence hierarchy, and no decorative behavior that weakens maintenance clarity.

Key evolution from Phase 7:

- `App.svelte` is no longer the eight-surface monolith; the renderer is shell + event reducer + feature modules.
- renderer polling is removed; persistent IPC v7 events drive live state;
- fixed-duration transform transitions are replaced by an internal response/damping spring runtime;
- pointer-down tactile response is standard across product buttons;
- dialogs use presentation-value interruptibility, focus retention and source-aware materialization;
- direct manipulation has reusable velocity estimation, projection, snap and rubber-band mechanics but is enabled only where the gesture has semantic utility;
- the legacy `foundation.css` / `luxury.css` override stack is removed;
- reduced motion, reduced transparency, increased contrast and forced colors are independent adaptations;
- Mica remains the base rather than being simulated with a permanent WebView blur.

Do not add new interaction timing to this file. Add or modify behavior in `apps/ui/src/design/motion`, `apps/ui/src/design/primitives`, semantic design tokens, and the Phase 11 verification gates.
