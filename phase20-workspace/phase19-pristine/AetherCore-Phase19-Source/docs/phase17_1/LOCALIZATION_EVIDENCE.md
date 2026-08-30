# Phase 17.1 EN/AR and UI Semantic Evidence

Phase 17.1 does not add a new dashboard or product surface. It extends the existing Finding card only where required to represent lifecycle and correlation truth.

New semantic content includes localized EN/AR messages for:

- not rechecked;
- verification unavailable;
- resolution confirmed;
- authoritative-absence and explicit-healthy resolution reasons;
- strong/moderate/weak correlation strength;
- time distance and shared scope;
- uncertainty/conflicting evidence;
- driver-change/crash and hardware/crash correlation rationales.

The production catalogs preserve exact key parity. The executed Phase 12 localization gates report:

- EN entries: 1218
- AR entries: 1218
- parity: 1218
- checks: 32/32 PASS

The UI change uses the existing material/interaction primitives rather than introducing a new motion path. Verification state is conveyed by text and semantics, not color alone. Correlation details remain expandable technical information. No new gesture-driven CSS keyframe path is introduced.

Runtime keyboard, screen-reader, reduced-motion, reduced-transparency, contrast and Windows WebView behavior remain covered by existing native qualification debt (`P17-QD-009`).
