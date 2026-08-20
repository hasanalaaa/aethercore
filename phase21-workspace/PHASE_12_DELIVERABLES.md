# Phase 12 Deliverables — Full Localization & RTL Perfection

## Milestone result

Phase 12 converts localization from a shell-level string table into a typed product-wide presentation contract. English and Arabic now cover all eight feature surfaces, operation/recovery copy, driver servicing, repair/cleanup/startup execution, hardware telemetry, crash diagnostics and service errors while preserving raw technical evidence exactly.

## Typed catalog architecture

- `catalog.en.ts` is the canonical `MessageKey` source.
- `catalog.ar.ts` is statically constrained to `Record<MessageKey, string>`.
- Literal `t()` calls infer required placeholders from the English template at compile time.
- Runtime semantic maps use `td()` with an already typed key rather than arbitrary strings.
- `Intl.PluralRules` drives count-sensitive copy.
- Arabic plural entries provide all six CLDR categories: zero, one, two, few, many and other.
- Catalog verification rejects missing keys, extra keys, placeholder drift and missing plural forms.

## Deep technical localization

AetherCore-owned findings, state labels, risk/severity/confidence, repair stages, cleanup descriptions, startup evidence/recommendations, Windows Update/driver-install progress, recovery records and diagnostic triage guidance have semantic English/Arabic mappings.

Raw evidence is deliberately not translated: device and vendor names, paths, GUIDs, hashes, bugcheck/HRESULT/WUA codes, addresses, INF/SYS/DMP names, firmware/driver versions and identifiers remain exact.

## Bidirectional rendering

- `TechnicalText.svelte` isolates technical values as LTR with Unicode bidi isolation.
- `LocalizedOwnedText.svelte` segments known localized prose so technical placeholders inside Arabic sentences are isolated individually.
- CSS uses logical direction properties rather than physical left/right layout rules.
- Arabic optical typography removes Latin casing/tracking treatments and uses Arabic-appropriate leading and weight.
- Command palette/navigation direction continues to mirror from the Phase 11 design system.

## Verification delivered

- `scripts/test-phase12-localization.py`
- `scripts/phase12-localization-audit.ps1`
- `scripts/verify-phase12.ps1`
- Phase 0–12 static localization/RTL invariants.
- Windows CI and signed release workflow invoke the Phase 12 gate.
- Signed release packaging is deferred until all Phase 12 localization, renderer, Rust bridge, and static gates pass; a static invariant rejects premature inherited packaging.

The platform-neutral authoring checks validate catalog parity, placeholders, plural structure, visible copy, technical isolation, logical CSS, semantic mapping coverage, TypeScript source semantics, Svelte script syntax and CSS parsing. Full Svelte/WebView2 build, Windows visual shaping, Narrator and installer/security gates remain authoritative on Windows.

## Required Windows language qualification

Before GA, execute both `en` and `ar` on supported Windows 11/WebView2 builds at 100%, 125%, 150%, 175% and 200% DPI. Verify keyboard/Narrator navigation, high contrast, reduced motion/transparency, long Arabic strings, mixed Arabic plus paths/GUIDs/bugchecks/versions, dialogs, command palette, progress/recovery surfaces and all eight feature pages. Technical tokens must preserve exact character order and copyability.

## Final authoring qualification

- EN/AR message catalog parity: **908 / 908 keys**.
- Dedicated deep localization audit: **32 / 32 PASS**.
- Legacy/consent/service-message localization audit: **PASS**.
- Arabic Intl/pluralization runtime tests: **PASS**.
- Phase 0–12 platform-neutral invariants: **213 / 213 PASS**.
- Strict TypeScript source audit: **32 / 32 files PASS** using dependency API stubs.
- Svelte control-block balance: **23 / 23 components PASS**.
- Embedded Svelte TypeScript syntax: **23 / 23 script blocks PASS**.
- CSS parser: **9 / 9 stylesheets PASS**.

The authoring environment does not contain the frozen pnpm dependency graph, Cargo/Windows SDK, PowerShell, WebView2 or the Windows presentation stack. `verify-phase12.ps1` therefore remains the authoritative native compile, localized UAC, accessibility, installer and release gate.
