# Accessibility and localization contract — Phase 12

## Accessibility invariants

AetherCore's non-elevated renderer maintains these cross-page requirements:

- skip link to main content;
- `aria-current="page"` on active primary navigation;
- polite live announcements for session/page/state changes;
- alert semantics for action failures;
- modal-dialog semantics for review and command surfaces;
- focus enters the newest dialog, Tab/Shift+Tab stay inside, Escape closes it, and focus returns to the prior control only after the closing presentation settles;
- keyboard-only primary navigation and command palette operation;
- visible tokenized `:focus-visible` indicators;
- all product buttons provide immediate pointer-down feedback while committing on normal click/release semantics;
- unknown progress is exposed as indeterminate rather than an invented percentage.

The Windows gate runs `svelte-check --threshold warning --fail-on-warnings`; Svelte accessibility diagnostics therefore fail verification.

## Independent adaptation signals

Phase 11 treats motion, transparency and contrast separately:

- `prefers-reduced-motion: reduce` removes spring travel/overshoot and leaves concise state feedback;
- `prefers-reduced-transparency: reduce` removes material blur and raises surface opacity;
- `prefers-contrast: more` strengthens boundaries and text contrast;
- `forced-colors: active` uses Windows system colors and explicit borders rather than glass effects.

`window-ux.ts` mirrors these signals into document datasets so component/CSS adaptation can remain consistent. High-DPI scale changes are still observed through Tauri `scaleFactor()` and `onScaleChanged`; CSS is not manually multiplied by device scale because WebView2 already operates in logical CSS pixels.

## Motion accessibility

Reduced motion does not mean no feedback. Pressed state, selection, color and completion cues remain available. Large spatial travel, elastic overshoot and repeating status motion are removed. Indeterminate indicators become a static subdued progress surface when motion is reduced.

Direct drag primitives disable projected momentum under reduced motion and snap directly to their semantic target after manipulation.

## Screen-reader content policy

Visual icons remain decorative unless they communicate information not repeated in text. Technical evidence—hashes, bugchecks, protocol versions, ATA raw evidence and device IDs—remains literal. AetherCore must not turn uncertain diagnostic evidence into a more confident spoken interpretation.

## Full English / Arabic localization and RTL

`lib/i18n/catalog.en.ts` is the canonical product message schema. `MessageKey` is derived from that catalog and `catalog.ar.ts` must satisfy the exact same key set. Literal `t()` calls infer parameter placeholders at compile time; runtime semantic maps select only typed keys. Count-sensitive copy uses `Intl.PluralRules`, with all six Arabic plural categories present.

All eight feature surfaces, recovery/activity records, driver servicing, system repair, cleanup, startup management, hardware telemetry, crash diagnostics and typed service errors use the catalogs for AetherCore-owned prose. Verification rejects missing/extra keys, placeholder drift, manual English pluralization, visible hardcoded English and business logic that inspects translated display strings.

Selecting Arabic updates document `lang`/`dir` and stores only the locale preference. Phase 11 logical layout remains intact; Phase 12 additionally removes Latin uppercase/tracking treatment from Arabic labels and uses Arabic-appropriate leading/weight.

Technical evidence is intentionally not translated. Device/vendor names, paths, GUIDs, hashes, bugcheck/HRESULT/WUA codes, addresses, INF/SYS/DMP names and driver/firmware versions are rendered through `TechnicalText` with `direction:ltr` and `unicode-bidi:isolate`. `LocalizedOwnedText` segments technical placeholders embedded in Arabic prose so character order and copyability remain exact. Unknown native text falls back to isolated technical evidence rather than being misrepresented as localized product prose.

## Required physical accessibility qualification

Before public release, validate on Windows 11 with:

- Narrator and at least one additional screen reader used by the target audience;
- keyboard-only traversal across all eight surfaces and all dialogs;
- 100%, 125%, 150%, 175% and 200% display scaling;
- mixed-DPI monitor movement;
- 60/120/144 Hz displays for motion quality;
- Windows High Contrast / forced colors;
- transparency disabled/reduced;
- reduced motion;
- Arabic RTL with mixed Arabic/Latin technical evidence and long strings;
- touch/pen target review where supported;
- rapid open/close reversal of dialogs to confirm focus and presentation continuity.

## Zenith accessibility/RTL additions

- Update channel and Driver filter toggle buttons expose `aria-pressed` alongside their visual active state.
- Startup Unreviewed/Keep/Disable decisions are an ARIA radio group with roving focus and Arrow/Home/End keyboard navigation; horizontal Arrow direction follows RTL spatial order.
- Update progress uses the shared `ProgressBar` with `role="progressbar"`, localized label and current value when known.
- Guided diagnostic action arrows are spatial decoration, not localized prose; they mirror under `[dir="rtl"]`.
- Progress growth explicitly starts at the left edge in LTR and right edge in RTL.
- Reduced-motion dialog close is exactly-once and retains deterministic focus restoration.
