# ADR 0014 — Typed Localization and Bidirectional Technical Integrity

## Status

Accepted — Phase 12.

## Context

Phase 11 established a modular event-driven frontend and fluid design system, but product localization was still incomplete. AetherCore receives a mixture of product-owned prose and externally sourced technical evidence from Windows, WUA, PnP, Event Log, WHEA, storage telemetry and maintenance journals. Treating both classes as ordinary strings creates three risks: missing translations, business logic accidentally tied to English display text, and corruption/reordering of technical values inside Arabic RTL layouts.

## Decision

1. English is the canonical typed message schema; `MessageKey` is derived from `catalog.en.ts`.
2. Arabic must statically satisfy the exact `MessageKey` set and parameter placeholders.
3. Plural copy is selected by `Intl.PluralRules`; Arabic implements all six CLDR categories.
4. All eight product surfaces consume localization through the selected shell locale.
5. Stable semantic values are localized by typed adapters; localized/display strings are never identifiers or business-state discriminators.
6. Service errors preserve `ErrorInfo.message_key` through the Tauri bridge instead of preferring English technical detail.
7. Known backend-owned prose is translated only at the presentation boundary through explicit exact/pattern mappings.
8. Device/vendor strings, paths, GUIDs, hashes, native codes, addresses and versions remain untranslated technical evidence.
9. Technical evidence is rendered with explicit LTR direction and Unicode bidi isolation, including technical placeholders embedded inside Arabic prose.
10. CSS layout uses logical properties and Arabic receives dedicated shaping/leading/tracking rules.
11. The elevated UAC broker accepts only a non-secret service intent plus a strictly allowlisted display locale. Locale never participates in authorization.
12. CI/static verification rejects catalog drift, hardcoded visible English, missing backend prose coverage, display-string business logic, plural drift and bidi/layout regressions.

## Consequences

A new user-visible message is incomplete until both English and Arabic entries exist. A new parameter requires parity in both catalogs. New native prose either needs a semantic mapping or must be explicitly classified as technical evidence. UI logic cannot use wording as a state machine. Technical values remain copyable and exact even in Arabic. Localization changes therefore become reviewable contract changes rather than best-effort visual edits.

## Alternatives rejected

- A free-form `Record<string,string>` catalog: permits missing keys and silent drift.
- Translating raw backend strings directly in Rust: couples engine safety logic to presentation language and makes technical evidence harder to preserve.
- Treating all native text as Arabic-translatable prose: risks corrupting identifiers and falsely translating third-party/Windows evidence.
- CSS `direction:ltr` on whole cards: preserves tokens but breaks Arabic reading order.
- Locale-dependent route IDs or plan-title parsing: makes product behavior dependent on copywriting.
