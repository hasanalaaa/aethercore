#!/usr/bin/env python3
"""Dependency-light Phase 12 localization, parity, bidi and raw-prose audit."""
from __future__ import annotations
from pathlib import Path
import re, sys

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT / "scripts"))
from gate_reader import module_text  # noqa: E402

# `DBT-P63-004`: the maintenance service router is a module tree now -
# `router.rs` plus `router/*.rs`. These checks assert its verbs.
ROUTER = "services/maintenance-service/src/router.rs"
UI = ROOT / 'apps/ui/src'
EN = UI / 'lib/i18n/catalog.en.ts'
AR = UI / 'lib/i18n/catalog.ar.ts'
PEN = UI / 'lib/i18n/plurals.en.ts'
PAR = UI / 'lib/i18n/plurals.ar.ts'

class AuditError(RuntimeError): pass

def parse_catalog(path: Path):
    entries, duplicates = {}, []
    entry_re = re.compile(r"^\s*(['\"])(?P<key>[^'\"]+)\1\s*:\s*(?P<q>['\"])(?P<value>(?:\\.|(?!\3).)*)\3\s*,?\s*$")
    for lineno, line in enumerate(path.read_text(encoding='utf-8').splitlines(), 1):
        m = entry_re.match(line)
        if not m: continue
        key = m.group('key')
        if key in entries: duplicates.append((key, entries[key][1], lineno))
        entries[key] = (m.group('value'), lineno)
    return entries, duplicates

def placeholders(value: str): return frozenset(re.findall(r"\{([A-Za-z_][A-Za-z0-9_]*)\}", value))

def fail(errors, message): errors.append(message)

def main() -> int:
    errors=[]; notes=[]
    en,en_dups=parse_catalog(EN); ar,ar_dups=parse_catalog(AR)
    if en_dups: fail(errors, f'English duplicate catalog keys: {en_dups[:8]}')
    if ar_dups: fail(errors, f'Arabic duplicate catalog keys: {ar_dups[:8]}')
    if set(en)!=set(ar):
        fail(errors, f'Catalog parity mismatch: missing_ar={sorted(set(en)-set(ar))[:20]}, missing_en={sorted(set(ar)-set(en))[:20]}')
    for key in sorted(set(en)&set(ar)):
        if placeholders(en[key][0]) != placeholders(ar[key][0]):
            fail(errors, f'Placeholder mismatch {key}: en={sorted(placeholders(en[key][0]))} ar={sorted(placeholders(ar[key][0]))}')
    # Typed architecture invariants.
    ent=EN.read_text(encoding='utf-8'); art=AR.read_text(encoding='utf-8')
    runtime=(UI/'lib/i18n/runtime.ts').read_text(encoding='utf-8')
    semantic=(UI/'lib/i18n/semantic.ts').read_text(encoding='utf-8')
    bidi=(UI/'lib/i18n/bidi.ts').read_text(encoding='utf-8')
    tech=(UI/'design/primitives/TechnicalText.svelte').read_text(encoding='utf-8')
    if 'export type MessageKey = keyof typeof enCatalog' not in ent: fail(errors,'MessageKey is not derived from the English catalog.')
    if 'satisfies Record<MessageKey, string>' not in art: fail(errors,'Arabic catalog is not compile-time constrained to MessageKey parity.')
    for token in ['ExtractVars<','VarsFor<','Intl.PluralRules','formatNumber','formatDateTime']:
        if token not in runtime: fail(errors,f'i18n runtime missing typed/runtime invariant: {token}')
    for token in ['segmentBidiEvidence','technical: boolean','TECHNICAL_TOKEN']:
        if token not in bidi: fail(errors,f'bidi helper missing: {token}')
    for token in ['dir="ltr"','unicode-bidi', 'data-technical']:
        hay=tech + '\n' + (UI/'design/styles/typography.css').read_text(encoding='utf-8')
        if token not in hay: fail(errors,f'technical isolation missing: {token}')

    # Plural architecture and Arabic six-category coverage.
    pen=PEN.read_text(encoding='utf-8'); par=PAR.read_text(encoding='utf-8')
    en_plural_keys=set(re.findall(r"^\s*['\"]([^'\"]+)['\"]\s*:\s*\{",pen,re.M))
    ar_plural_keys=set(re.findall(r"^\s*['\"]([^'\"]+)['\"]\s*:\s*\{",par,re.M))
    if en_plural_keys != ar_plural_keys: fail(errors,f'Plural key parity mismatch: en-only={sorted(en_plural_keys-ar_plural_keys)} ar-only={sorted(ar_plural_keys-en_plural_keys)}')
    for category in ('zero','one','two','few','many','other'):
        if not re.search(rf"\b{category}\s*:",par): fail(errors,f'Arabic plural catalog missing category {category}.')
    if 'satisfies Record<PluralMessageKey, ArabicPlural>' not in par: fail(errors,'Arabic plural catalog lacks typed parity constraint.')

    # Literal key references must exist.
    source='\n'.join(p.read_text(encoding='utf-8') for p in UI.rglob('*') if p.suffix in {'.ts','.svelte'})
    used=set(re.findall(r"\b(?:t|td)\(\s*['\"]([^'\"]+)['\"]",source))
    missing=sorted(used-set(en))
    if missing: fail(errors,f'Unknown message keys referenced: {missing}')
    usedp=set(re.findall(r"\btp\(\s*['\"]([^'\"]+)['\"]",source))
    missingp=sorted(usedp-en_plural_keys)
    if missingp: fail(errors,f'Unknown plural keys referenced: {missingp}')

    # Each of the eight product surfaces must use the localization runtime.
    surfaces=[
      'features/overview/OverviewPage.svelte','features/drivers/DriversPage.svelte','features/repair/RepairPage.svelte',
      'features/cleanup/CleanupPage.svelte','features/startup/StartupPage.svelte','features/diagnostics/HardwarePage.svelte',
      'features/diagnostics/CrashPage.svelte','features/activity/ActivityPage.svelte']
    for rel in surfaces:
        text=(UI/rel).read_text(encoding='utf-8')
        if "locale = $shellState.locale" not in text or "t('" not in text: fail(errors,f'Surface is not locale-driven: {rel}')

    # Stable semantic route IDs: UI labels must never be routing identifiers.
    navigation=(UI/'lib/navigation.ts').read_text(encoding='utf-8')
    expected=['overview','drivers','repair','cleanup','startup','hardware','crash','activity']
    for route in expected:
        if f"id: '{route}'" not in navigation: fail(errors,f'Missing semantic route id: {route}')
    forbidden_routes=['Overview','Drivers','Repair','Cleanup','Startup','Hardware','Crash history','Activity & recovery']
    for route in forbidden_routes:
        if f"id: '{route}'" in navigation: fail(errors,f'Display string used as route id: {route}')
    # Known historic display-string business logic must stay dead.
    for bad in ["title.startsWith(", "active.title", "=== 'Crash history'", "=== 'Activity & recovery'"]:
        if bad in source: fail(errors,f'Display-string business logic reintroduced: {bad}')

    # Visible hard-coded English prose scanner. Technical brand/shortcut tokens are intentionally exempt.
    # `Ctrl /` joined `Ctrl K` and `Esc` as a keyboard shortcut rendered in a `<kbd>`
    # (`NavigationRail.svelte:56`); a chord is not prose and does not translate. P63.
    allowed_exact={'AetherCore','Esc','Ctrl K','Ctrl /'}
    for path in UI.rglob('*.svelte'):
        text=path.read_text(encoding='utf-8')
        text_no_script=re.sub(r'<script\b[^>]*>.*?</script>','',text,flags=re.S)
        text_no_style=re.sub(r'<style\b[^>]*>.*?</style>','',text_no_script,flags=re.S)
        for m in re.finditer(r'>([^<>]+)<',text_no_style):
            raw=m.group(1).strip()
            if not raw: continue
            # Ignore any chunk containing Svelte expression/block syntax; literals inside those are contract/technical values, not markup prose.
            if '{' in raw or '}' in raw: continue
            normalized=' '.join(raw.split())
            if normalized in allowed_exact: continue
            if re.search(r'[A-Za-z]{2,}',normalized):
                fail(errors,f'Raw visible English prose {path.relative_to(ROOT)}:{text[:m.start()].count(chr(10))+1}: {normalized[:120]}')
        for m in re.finditer(r'\b(?:aria-label|title|placeholder|alt)="([^"{}]*[A-Za-z][^"{}]*)"',text_no_script):
            value=m.group(1).strip()
            if value not in allowed_exact: fail(errors,f'Raw English accessibility/tooltip attribute {path.relative_to(ROOT)}: {value}')

    # Live announcements/errors emitted by TypeScript must be localized rather than hardcoded English prose.
    for path in UI.rglob('*.ts'):
        text = path.read_text(encoding='utf-8')
        for pattern in [r'\bannounce\(\s*[\"\']([A-Za-z][^\"\']*)[\"\']', r'\bsetError\(\s*[\"\']([A-Za-z][^\"\']*)[\"\']']:
            for m in re.finditer(pattern, text): fail(errors, f'Raw English live/error prose {path.relative_to(ROOT)}: {m.group(1)}')

    # Arabic should not accidentally mirror English prose. Only stable technical/autonym forms are allowed identical.
    same_allow={
      'common.windows','crash.bugcheck','drivers.step.inventoryHint','locale.switchToArabic','locale.switchToEnglish',
      'overview.moduleHardwareCopy','tech.card.bugcheckEvidence','common.countWithBytes','crash.historyCounts','tech.startup.actionProgress','tech.startup.publisherWindows',
      # Protocol acronyms, and the six fleet PLACEHOLDER samples: an example hostname, an
      # example identity-file path, an example base64 host key and example comma lists are
      # technical exemplars, identical in every locale by construction. They are catalog
      # entries rather than markup literals so they CAN diverge if a deployment ever wants
      # different examples; the seventh, `fleet.placeholderFingerprint`, is prose and is
      # translated. P63.
      'fleet.ssh','repair.domain.dns',
      'fleet.placeholderId','fleet.placeholderHost','fleet.placeholderAuthPath',
      'fleet.placeholderTags','fleet.placeholderPublicKey','fleet.placeholderScope'
    }
    identical=[k for k in set(en)&set(ar) if en[k][0]==ar[k][0] and k not in same_allow]
    if identical: fail(errors,f'Arabic entries unexpectedly identical to English: {sorted(identical)[:30]}')

    # Arabic catalog must not preserve the unlocalized English UI phrase "Windows Update".
    if 'Windows Update' in '\n'.join(v for v,_ in ar.values()): fail(errors,'Arabic catalog still contains the UI phrase "Windows Update"; use تحديث Windows.')

    # Every typed service error message key must have renderer catalog coverage; technical_detail stays diagnostic-only.
    service_sources = '\n'.join(module_text(ROOT, rel) for rel in [
        'services/maintenance-service/src/errors.rs', ROUTER, 'services/maintenance-service/src/server.rs'
    ])
    service_keys = set(re.findall(r'\"((?:ipc|kernel|plan|authorization|drivers|repair|cleanup|startup|diagnostics|service)\.[A-Za-z0-9_.]+)\"', service_sources))
    uncovered_service = sorted(service_keys - set(en))
    if uncovered_service: fail(errors, f'Typed service error keys missing from catalogs: {uncovered_service}')
    desktop = (ROOT/'apps/desktop/src/main.rs').read_text(encoding='utf-8')
    if 'anyhow::bail!(error.message_key.clone())' not in desktop: fail(errors,'Desktop bridge does not preserve typed service message_key to the renderer.')
    if 'error.technical_detail.is_empty()' in desktop: fail(errors,'Desktop bridge still prefers technical_detail over localized message_key.')

    # UAC consent stays secret-free while allowing only an en/ar presentation hint; locale never participates in authorization.
    broker = (ROOT/'apps/consent-broker/src/main.rs').read_text(encoding='utf-8')
    for token in ['--intent-id','--locale','locale != "en" && locale != "ar"','MB_RTLREADING','localized_operation_ar','GetUserDefaultUILanguage']:
        if token not in broker: fail(errors,f'Localized consent broker invariant missing: {token}')
    if '--challenge' in broker or '--challenge' in desktop: fail(errors,'Retired secret/challenge reintroduced to consent broker CLI.')

    # Deep domain adapters must exist for backend-owned semantic/prose payloads.
    for token in ['localizeOwnedText','localizeState','localizeRisk','localizeSeverity','localizeConfidence','localizeImpact','localizeKind','localizeDirection','localizeDomain','localizePlanKind','driverStateLabel','driverTargetEvidence']:
        if token not in semantic: fail(errors,f'Semantic localizer missing: {token}')
    owned_component=(UI/'design/primitives/LocalizedOwnedText.svelte').read_text(encoding='utf-8')
    if 'segmentBidiEvidence' not in owned_component or 'TechnicalText' not in owned_component: fail(errors,'LocalizedOwnedText does not isolate technical segments/fallback evidence.')

    # Logical-property / RTL typography checks.
    styles='\n'.join(p.read_text(encoding='utf-8') for p in (UI/'design/styles').glob('*.css'))
    if re.search(r'\b(?:margin|padding|border|inset)-(?:left|right)\b',styles): fail(errors,'Physical left/right box-model property found in design styles.')
    for token in ['[dir="rtl"]','unicode-bidi: isolate','font-optical-sizing','data-technical']:
        if token not in styles + tech: fail(errors,f'RTL/typography invariant missing: {token}')

    print(f'Catalog entries: en={len(en)} ar={len(ar)} parity={len(set(en)&set(ar))}')
    print(f'Literal message references: {len(used)}; plural references: {len(usedp)}; plural keys: {len(en_plural_keys)}')
    if errors:
        print(f'Phase 12 localization audit: FAIL ({len(errors)} issue(s))', file=sys.stderr)
        for e in errors: print(f' - {e}', file=sys.stderr)
        return 1
    print('Phase 12 localization audit: PASS')
    return 0

if __name__=='__main__': raise SystemExit(main())
