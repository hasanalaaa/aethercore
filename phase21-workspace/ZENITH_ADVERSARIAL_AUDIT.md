# AetherCore Zenith Adversarial Audit

## Scope and method

The supplied Enterprise repository was treated as authoritative. The archive manifest was verified before edits, top-level workspace/UI/service/installer/security/release evidence was inventoried, and historical platform-neutral audits were rerun before the Zenith changes. The pass then independently challenged UI motion, pointer cancellation, reduced-motion state transitions, ARIA state exposure, RTL spatial semantics, release-gate coverage, resource boundedness, and known Windows-only boundaries.

The audit did **not** assume that historical Phase/Enterprise PASS records prove current runtime correctness. A finding was changed only when a concrete failure mechanism or measurable maintainability/UX invariant was identified.

## Confirmed findings

1. **ZEN-001 — duplicate reduced-motion dialog completion.** `FluidDialog` had two close-completion paths. This was elevated because callers can bind completion to workflow finalization rather than mere decoration.
2. **ZEN-002 — pointer-captured drag-away activation.** `fluidPress` visually disarmed outside hysteresis but did not suppress the captured native click. This contradicted the required cancel-by-dragging-away behavior.
3. **ZEN-003 — cancellation treated as momentum release.** `pointercancel` and lost capture inherited drag velocity; system abort is not a user flick.
4. **ZEN-004/005 — selected/progress state insufficiently exposed.** System Care channel controls and Driver filters relied on visual state; update progress duplicated a layout-width bar instead of the existing semantic compositor-friendly primitive.
5. **ZEN-006/007 — incomplete RTL spatial semantics.** Diagnostic action arrows and progress direction needed explicit mirroring.
6. **ZEN-008 — regression-governance gap.** Historical source audits could not prevent recurrence of defects discovered after they were authored.
7. **ZEN-009 — Startup choice semantics.** The adversarial breaker pass found the three mutually exclusive startup decisions were visually selected but not represented as a radio-choice model to assistive technology.

All confirmed findings were corrected in source and encoded into the additive Zenith audit.

## Independent findings considered and rejected

- **Overlapped named-pipe I/O migration:** not performed merely because it is newer. The current architecture has dedicated writer pumps, frame and byte budgets, per-session admission limits, and fail-closed saturation. A synchronous Windows writer can still block on a malicious/non-reading peer, so Windows stress remains a residual qualification item. A transport rewrite without native cancellation/race evidence would increase risk.
- **App-lifetime `MutationObserver`:** retained. Initialization is singleton-guarded and the observer intentionally lives for the renderer lifetime; adding teardown ownership would not reduce process-lifetime retention.
- **Unused drag primitive:** retained without inventing a feature to justify it. The Zenith directive explicitly forbids refactoring for novelty.
- **Missing lockfiles:** not fabricated. Existing release governance intentionally treats dependency freeze as a trusted, fail-closed prerequisite; generating lock state without approved online resolution/review would undermine supply-chain evidence.

## Security and trust-boundary review

No Zenith change expands privileged inputs, caller-selected machine operations, update elevation, cryptographic authority, support-bundle contents, persistent schema, or scheduler mutation authority. The pointer-cancellation correction reduces accidental UI activation but is not treated as an authorization control; service-side authorization remains mandatory.

## Failure-state review

The pass specifically re-reasoned pointer cancellation, lost capture, preference changes while a dialog is already closed, close reversal, keyboard activation after pointer cancellation, reduced motion, RTL rendering, and progress updates. Windows-native failure cases (pipe teardown, WebView2 pointer semantics, DPI/Narrator, signing/installer lifecycle) remain delegated to existing native gates plus the new Zenith wrapper.
