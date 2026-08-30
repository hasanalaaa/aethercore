# ADR 0020 — Zenith interaction resilience and additive qualification

**Status:** Accepted — Zenith convergence

## Context

A post-Enterprise adversarial pass found interaction defects that were not privilege-boundary bugs but could still violate user agency: reduced-motion dialog completion could be duplicated, pointer-captured buttons could visually cancel yet still activate, system pointer cancellation inherited momentum, and several toggle/progress/RTL states were visually expressed without equally strong semantic behavior.

## Decision

1. `FluidDialog` has one idempotent completion path. Spring settlement is the completion authority for both normal and reduced-motion synchronization.
2. `fluidPress` retains pointer capture and hysteresis but fences the native click when pointer-up occurs while disarmed. The fence is one-shot and does not interfere with keyboard activation.
3. `fluidDrag` distinguishes intentional release from cancellation/lost capture. Only intentional release inherits velocity.
4. Product toggles expose selected state through `aria-pressed`; progress uses the shared ARIA-aware transform-based primitive.
5. Spatial action cues and progress origins explicitly mirror in RTL while technical identifiers retain isolated LTR semantics.
6. Zenith verification is additive. Historical Enterprise/Phase-16 gates remain in force; a new source audit is enforced in CI, before signed release packaging, and before GA sealing.

## Consequences

The renderer gains slightly more explicit event-state bookkeeping but removes ambiguous activation and duplicate progress behavior. No privileged authority, protocol, schema, update trust, cryptography, diagnostic interpretation, or background mutation contract changes.

## Qualification boundary

Source-level tests prove the intended event/state structure only. Real WebView2 pointer capture/click synthesis, accessibility APIs, refresh-rate behavior, Windows native code, installer/signing, and stress/soak behavior remain Windows execution gates.
