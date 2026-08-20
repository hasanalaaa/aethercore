# AetherCore Localization and Bidirectional Rendering Contract — Phase 12

## Purpose

Phase 12 makes localization a typed product contract rather than a shell-level lookup table. AetherCore supports two complete presentation locales: English (`en`) and Arabic (`ar`). The renderer translates AetherCore-owned prose while preserving technical evidence byte-for-display faithful.

## Catalog architecture

The canonical schema is `apps/ui/src/lib/i18n/catalog.en.ts`. Its keys define `MessageKey`; `catalog.ar.ts` must satisfy `Record<MessageKey, string>`. The catalogs therefore cannot drift without a TypeScript/static verification failure.

Parameterized messages use `{name}` placeholders. The runtime derives the required argument names from the English template so literal `t()` calls are statically checked for missing arguments. Dynamic semantic adapters select a typed `MessageKey` first and then call `td()`.

Count-sensitive messages are separate plural families. `Intl.PluralRules` selects the category. English supplies `one` and `other`; Arabic supplies `zero`, `one`, `two`, `few`, `many`, and `other`. Number/date formatting follows the selected locale, with Arabic using `ar-IQ` presentation.

## Ownership boundary: prose versus evidence

AetherCore-owned prose includes navigation, labels, statuses, warnings, findings, recovery guidance, diagnostic explanations, maintenance stages and service errors. It must resolve to a catalog message or an explicitly tested semantic pattern.

Technical evidence is intentionally not translated or reordered. This includes:

- Windows file/UNC paths;
- GUIDs, hashes and correlation identifiers;
- bugcheck values, HRESULT/WUA/native result codes and memory addresses;
- INF/SYS/DMP/MDMP names;
- device/vendor/product strings received from Windows or third parties;
- driver/firmware versions and hardware identifiers.

`TechnicalText.svelte` renders this evidence with `dir="ltr"`, `translate="no"`, `unicode-bidi:isolate`, and the `data-technical` marker. `LocalizedOwnedText.svelte` first localizes known AetherCore prose, then segments embedded technical tokens so an Arabic sentence can remain RTL while each technical token retains exact LTR order.

Unknown native/backend text is never claimed to be translated prose. It falls back to isolated technical evidence until a deliberate semantic mapping is added.

## Semantic localization

`apps/ui/src/lib/i18n/semantic.ts` owns translation of stable backend semantic values and legacy/native prose that cannot yet be emitted as message keys directly. It covers:

- operation states and risk;
- severity and confidence;
- startup kind/scope/direction/impact/recommendation/publisher classifications;
- repair, cleanup, startup and driver execution stages/details;
- storage, memory, WHEA and crash diagnostic findings;
- recovery records;
- typed service `message_key` errors.

The long-term preferred contract is typed keys from the service. Semantic text mapping exists only at the presentation boundary and must never be used as a business discriminator.

## Routing and business logic

Navigation identifiers are stable semantic IDs: `overview`, `drivers`, `repair`, `cleanup`, `startup`, `hardware`, `crash`, and `activity`. Localized labels are presentation only. Product logic must not branch on `t(...)`, localized strings, plan titles, prefixes or translated labels.

## Arabic RTL rules

Selecting Arabic sets document `lang="ar"` and `dir="rtl"`. Layout uses CSS logical properties. Physical left/right box-model properties are rejected by the localization audit.

Arabic typography uses Arabic-appropriate leading and optical weight, disables Latin uppercase/tracking treatments and avoids forced letter spacing. Mixed technical evidence remains individually isolated LTR rather than forcing an entire Arabic card or sentence to LTR.

## UAC consent presentation

The consent broker receives the service-minted non-secret `intent-id` plus an allowlisted presentation locale (`en` or `ar`). Locale has no authorization authority. The elevated broker renders Arabic with native RTL MessageBox flags when requested. No challenge, secret or translated display string participates in consent authorization.

## Verification contract

Phase 12 must fail verification for any of the following:

- English/Arabic key-set mismatch;
- placeholder mismatch;
- missing Arabic plural category;
- unknown literal message key;
- manual English singular/plural branching in UI copy;
- visible hardcoded English product prose in Svelte surfaces;
- product behavior tied to translated/display strings;
- backend-owned display prose without semantic coverage;
- service error key missing from either catalog;
- physical left/right layout regression;
- technical-data path without bidi isolation;
- reintroduction of secret/challenge parameters into the UAC broker.

The platform-neutral audits are `scripts/phase12-localization-audit.py`, `scripts/test-phase12-localization.py`, `scripts/phase12-i18n-tests.cjs`, and `scripts/static_validate.py`. `scripts/verify-phase12.ps1` remains the authoritative Windows build/runtime/localization gate.
