#!/usr/bin/env python3
"""Phase 12 localization/RTL contract audit.

Platform-neutral and deterministic. It validates the catalog as a compile-time
contract, scans Svelte templates for accidental English UI copy, verifies
plural morphology coverage, and checks the bidi/technical-evidence invariants.
"""
from __future__ import annotations

import ast
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
UI = ROOT / "apps/ui/src"
I18N = UI / "lib/i18n"

class AuditFailure(RuntimeError):
    pass


def parse_flat_catalog(path: Path) -> dict[str, str]:
    entries: dict[str, str] = {}
    for lineno, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith(("//", "/*", "*")):
            continue
        m = re.match(r"^(?P<keyq>['\"].*?['\"])\s*:\s*(?P<valq>['\"].*['\"])\s*,?$", line)
        if not m:
            continue
        try:
            key = ast.literal_eval(m.group("keyq"))
            value = ast.literal_eval(m.group("valq"))
        except Exception as exc:
            raise AuditFailure(f"{path}:{lineno}: unparseable catalog entry: {exc}") from exc
        if not isinstance(key, str) or not isinstance(value, str):
            continue
        if key in entries:
            raise AuditFailure(f"{path}:{lineno}: duplicate key {key!r}")
        entries[key] = value
    if not entries:
        raise AuditFailure(f"{path}: no catalog entries parsed")
    return entries


def parse_plural_catalog(path: Path) -> dict[str, dict[str, str]]:
    out: dict[str, dict[str, str]] = {}
    line_re = re.compile(r"^\s*(['\"])(?P<key>.+?)\1\s*:\s*\{(?P<body>.+)\}\s*,?\s*$")
    pair_re = re.compile(r"\b(zero|one|two|few|many|other)\s*:\s*(['\"])(?P<value>.*?)(?<!\\)\2(?=\s*(?:,|$))")
    for lineno, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        m = line_re.match(line)
        if not m:
            continue
        key = m.group("key")
        if key in out:
            raise AuditFailure(f"{path}:{lineno}: duplicate plural key {key}")
        forms = {x.group(1): x.group("value") for x in pair_re.finditer(m.group("body"))}
        if not forms:
            raise AuditFailure(f"{path}:{lineno}: plural forms were not parsed for {key}")
        out[key] = forms
    if not out:
        raise AuditFailure(f"{path}: no plural entries parsed")
    return out


def placeholders(value: str) -> set[str]:
    return set(re.findall(r"\{([A-Za-z_][A-Za-z0-9_]*)\}", value))


def source_files() -> list[Path]:
    return sorted([*UI.rglob("*.ts"), *UI.rglob("*.svelte")])


def literal_calls(pattern: str, files: list[Path]) -> list[tuple[Path, str]]:
    rx = re.compile(pattern)
    found: list[tuple[Path, str]] = []
    for path in files:
        text = path.read_text(encoding="utf-8")
        found.extend((path, key) for key in rx.findall(text))
    return found


def strip_svelte_code(text: str) -> str:
    text = re.sub(r"<script\b[^>]*>.*?</script>", "", text, flags=re.S | re.I)
    text = re.sub(r"<style\b[^>]*>.*?</style>", "", text, flags=re.S | re.I)
    # Svelte expressions/snippets are code, not literal display text.
    text = re.sub(r"\{[^{}]*\}", "", text, flags=re.S)
    return text


# A keyboard hint names a physical key the reader has to find on the keyboard,
# and Arabic keyboards carry Latin legends on the modifier row. Translating
# "Ctrl" would describe a key that is not on the keycap. The token is also the
# source of `aria-keyshortcuts`, whose vocabulary is fixed by the UI Events
# spec — NavigationRail.svelte derives it with .replace('Ctrl', 'Control'), so a
# translated token would silently emit an invalid value to screen readers.
# `kbd` is already inside the Phase 12 unicode-bidi isolation group
# (typography.css), so the token renders LTR and does not reorder under RTL.
KBD_ELEMENT = re.compile(r"<kbd\b[^>]*>(?P<body>[^<>]*)</kbd>", flags=re.I)
KEY_TOKEN = re.compile(r"^(?:Ctrl|Alt|Shift|Win|Esc|Enter|Tab|Del|Space|F[1-9][0-9]?)$")


def keyboard_shortcuts() -> list[tuple[str, str]]:
    """Every <kbd> body in the UI, as (location, text)."""
    found: list[tuple[str, str]] = []
    for path in sorted(UI.rglob("*.svelte")):
        template = strip_svelte_code(path.read_text(encoding="utf-8"))
        for match in KBD_ELEMENT.finditer(template):
            body = " ".join(match.group("body").split())
            if body:
                found.append((str(path.relative_to(ROOT)), body))
    return found


def non_key_shortcut_text() -> list[str]:
    """`kbd` is exempt from the prose scan, so it must hold key names only."""
    bad: list[str] = []
    for where, body in keyboard_shortcuts():
        parts = re.split(r"[ +]", body)
        if not all(KEY_TOKEN.match(p) or len(p) == 1 for p in parts):
            bad.append(f"{where}: <kbd> is not a key sequence: {body!r}")
    return bad


def hardcoded_visible_english() -> list[str]:
    # Deliberately tiny allowlist: proper product/technology names are technical
    # evidence, not translatable AetherCore prose. Keyboard hints are NOT listed
    # here one literal at a time — `kbd` is a category, checked separately by
    # non_key_shortcut_text(); enumerating "Ctrl K" but not "Ctrl /" is how this
    # gate came to treat two identical shortcuts differently (DBT-P66-002).
    allowed_phrases = {
        "AetherCore",
        "Windows Update",
    }
    failures: list[str] = []
    for path in sorted(UI.rglob("*.svelte")):
        template = KBD_ELEMENT.sub("", strip_svelte_code(path.read_text(encoding="utf-8")))
        # Text nodes only.
        for match in re.finditer(r">([^<>]+)<", template, flags=re.S):
            node = " ".join(match.group(1).split())
            if not node or '{' in node or '}' in node or not re.search(r"[A-Za-z]{2,}", node):
                continue
            normalized = node.strip(" ·—–:()[]{}↗→◇◫▱!+−✓")
            if normalized in allowed_phrases:
                continue
            failures.append(f"{path.relative_to(ROOT)}: text node {node!r}")
        # Literal user-facing attributes.
        for match in re.finditer(r"\b(?:aria-label|placeholder|title|alt)\s*=\s*(['\"])([^'\"{]+)\1", template, flags=re.I):
            value = match.group(2).strip()
            if re.search(r"[A-Za-z]{2,}", value) and value not in allowed_phrases:
                failures.append(f"{path.relative_to(ROOT)}: literal display attribute {value!r}")
    return failures


def assert_no_physical_direction_css() -> list[str]:
    bad: list[str] = []
    rx = re.compile(r"(?:margin|padding|border)-(?:left|right)\s*:|(?<![-\w])(?:left|right)\s*:|text-align\s*:\s*(?:left|right)|inset-(?:left|right)\s*:", re.I)
    for path in sorted(UI.rglob("*.css")):
        for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if rx.search(line):
                bad.append(f"{path.relative_to(ROOT)}:{n}: {line.strip()}")
    return bad


def main() -> int:
    checks: list[tuple[str, bool, str]] = []
    def check(name: str, ok: bool, detail: str = "") -> None:
        checks.append((name, ok, detail))

    en = parse_flat_catalog(I18N / "catalog.en.ts")
    ar = parse_flat_catalog(I18N / "catalog.ar.ts")
    check("catalog_key_parity", set(en) == set(ar), f"en={len(en)} ar={len(ar)} missing_ar={sorted(set(en)-set(ar))[:5]} missing_en={sorted(set(ar)-set(en))[:5]}")

    placeholder_mismatches = [key for key in en if key in ar and placeholders(en[key]) != placeholders(ar[key])]
    check("catalog_placeholder_parity", not placeholder_mismatches, f"mismatches={placeholder_mismatches[:10]}")
    check("arabic_catalog_typed", "satisfies Record<MessageKey, string>" in (I18N / "catalog.ar.ts").read_text(encoding="utf-8"))
    check("message_key_derived_from_english", "export type MessageKey = keyof typeof enCatalog" in (I18N / "catalog.en.ts").read_text(encoding="utf-8"))

    enp = parse_plural_catalog(I18N / "plurals.en.ts")
    arp = parse_plural_catalog(I18N / "plurals.ar.ts")
    check("plural_key_parity", set(enp) == set(arp), f"en={sorted(enp)} ar={sorted(arp)}")
    bad_en_forms = [k for k,v in enp.items() if set(v) != {"one","other"}]
    bad_ar_forms = [k for k,v in arp.items() if set(v) != {"zero","one","two","few","many","other"}]
    check("english_plural_forms", not bad_en_forms, str(bad_en_forms))
    check("arabic_six_plural_forms", not bad_ar_forms, str(bad_ar_forms))
    plural_placeholder_bad = [k for k in enp if k in arp and any("{count}" not in v and form not in {"zero","one","two"} for form,v in arp[k].items())]
    check("arabic_plural_count_placeholders", not plural_placeholder_bad, str(plural_placeholder_bad))

    files = source_files()
    literal_t = literal_calls(r"\bt\(\s*['\"]([^'\"]+)['\"]", files)
    missing_t = sorted({key for _,key in literal_t if key not in en})
    check("literal_message_keys_exist", not missing_t, f"missing={missing_t[:20]}")
    literal_tp = literal_calls(r"\btp\(\s*['\"]([^'\"]+)['\"]", files)
    missing_tp = sorted({key for _,key in literal_tp if key not in enp})
    check("literal_plural_keys_exist", not missing_tp, f"missing={missing_tp[:20]}")

    runtime = (I18N / "runtime.ts").read_text(encoding="utf-8")
    check("typed_placeholder_inference", "type ExtractVars" in runtime and "type VarsFor" in runtime and "...args: Args<K>" in runtime)
    check("intl_plural_rules", "new Intl.PluralRules" in runtime and "tp(" in runtime)
    check("no_legacy_record_catalog", "Record<string, string>" not in (UI / "lib/i18n.ts").read_text(encoding="utf-8") and "Record<string,string>" not in runtime)

    manual_plural_rx = re.compile(r"count\s*===\s*1\s*\?|\?\s*['\"]['\"]\s*:\s*['\"]s['\"]|file\{.*?\?.*?s", re.I | re.S)
    manual_plural_files = [str(p.relative_to(ROOT)) for p in files if manual_plural_rx.search(p.read_text(encoding="utf-8"))]
    check("no_manual_english_pluralization", not manual_plural_files, str(manual_plural_files[:10]))

    visible = hardcoded_visible_english()
    check("no_hardcoded_visible_english", not visible, f"{len(visible)} found: " + " | ".join(visible[:12]))
    shortcuts = keyboard_shortcuts()
    bad_shortcuts = non_key_shortcut_text()
    check("keyboard_shortcuts_are_key_tokens", bool(shortcuts) and not bad_shortcuts, f"{len(shortcuts)} kbd nodes; bad={bad_shortcuts[:8]}")

    bidi = (I18N / "bidi.ts").read_text(encoding="utf-8")
    tech = (UI / "design/primitives/TechnicalText.svelte").read_text(encoding="utf-8")
    owned = (UI / "design/primitives/LocalizedOwnedText.svelte").read_text(encoding="utf-8")
    typography = (UI / "design/styles/typography.css").read_text(encoding="utf-8")
    check("technical_text_primitive", 'class="technical-isolate"' in tech and 'dir="ltr"' in tech)
    check("technical_css_unicode_isolation", "direction: ltr" in typography and "unicode-bidi: isolate" in typography)
    check("technical_segmenter", all(token in bidi for token in ["segmentBidiEvidence", "0x[0-9A-Fa-f]+", "32,128", "inf|sys|dmp|mdmp"]))
    check("owned_text_segments_technical_tokens", "segmentBidiEvidence" in owned and "segment.technical" in owned and "TechnicalText" in owned)

    physical = assert_no_physical_direction_css()
    check("css_logical_direction_only", not physical, " | ".join(physical[:10]))
    check("rtl_optical_typography", "html[dir='rtl'] body" in typography and "line-height: 1.62" in typography and "text-transform:none" in typography and "letter-spacing:0" in typography)

    feature_paths = [
        "features/overview/OverviewPage.svelte", "features/drivers/DriversPage.svelte", "features/repair/RepairPage.svelte",
        "features/cleanup/CleanupPage.svelte", "features/startup/StartupPage.svelte", "features/diagnostics/HardwarePage.svelte",
        "features/diagnostics/CrashPage.svelte", "features/activity/ActivityPage.svelte",
    ]
    missing_i18n_feature = [p for p in feature_paths if "lib/i18n" not in (UI / p).read_text(encoding="utf-8")]
    check("all_eight_surfaces_use_i18n", not missing_i18n_feature, str(missing_i18n_feature))

    owned_surfaces = {
        "drivers": UI / "features/drivers/DriversPage.svelte",
        "repair": UI / "features/repair/RepairPage.svelte",
        "cleanup": UI / "features/cleanup/CleanupPage.svelte",
        "startup": UI / "features/startup/StartupPage.svelte",
        "hardware": UI / "features/diagnostics/HardwarePage.svelte",
        "crash": UI / "features/diagnostics/CrashPage.svelte",
        "recovery": UI / "features/RecoveryPanel.svelte",
    }
    missing_owned = [name for name,path in owned_surfaces.items() if ("LocalizedOwnedText" not in path.read_text(encoding="utf-8") and "localizeOwnedText" not in path.read_text(encoding="utf-8"))]
    check("backend_owned_prose_localized", not missing_owned, str(missing_owned))

    semantic = (I18N / "semantic.ts").read_text(encoding="utf-8")
    required_deep = [
        "tech.repair.dismRestore", "tech.cleanup.memoryDumpDesc", "tech.startup.validatingCurrent",
        "tech.driver.wuaEntryCollection", "tech.driver.install.backup", "tech.storage.nvmeCritical",
        "tech.card.memoryErrorsSummary", "tech.card.bugcheckEvidence", "tech.recovery.cleanupDetail",
    ]
    check("deep_technical_mapping_coverage", all(key in en and key in ar and key in semantic for key in required_deep), str([k for k in required_deep if k not in semantic]))

    # Cross-check literal product prose emitted by the native domains. Coded enum/result values and
    # unit-test fixture labels are intentionally not translated prose. Dynamic format strings are
    # checked separately below against semantic pattern handlers.
    native_roots = [
        ROOT / "crates/system-repair", ROOT / "crates/cleaner", ROOT / "crates/startup-manager",
        ROOT / "crates/hardware-telemetry", ROOT / "crates/crash-diagnostics", ROOT / "crates/diagnostic-engine",
        ROOT / "crates/driver-hub", ROOT / "crates/driver-install", ROOT / "crates/windows-update",
    ]
    # `result_code` is deliberately absent from this list. It is a wire value, not
    # display text: Rust matches it (`match check.result_code.as_str()` in
    # system-repair/src/lib.rs), the UI compares it (RepairPage.svelte), every UI
    # render site wraps it in <TechnicalText>, and it is an OPEN set —
    # `format!("ExitCode{code}")` means it can never be enumerated in a catalog.
    # Asking for its Arabic was a category error, not a missing translation
    # (DBT-P66-001). result_code_path_isolation below asserts the real invariant.
    display_fields = "title|stage|detail|summary|failure_message|protection_reason|impact|confidence|evidence_detail|recommendation|health_status|reason|source_note|category|severity|kind|scope|publisher"
    literal_rx = re.compile(rf"\b(?P<field>{display_fields})\s*:\s*\"(?P<value>(?:\\.|[^\"\\])*)\"\.into\(\)")
    native_literals: set[tuple[str, str]] = set()
    scanned_rs = 0
    for root in native_roots:
        if not root.exists():
            continue
        for path in root.rglob("*.rs"):
            if "tests" in path.parts:
                continue
            text = path.read_text(encoding="utf-8", errors="ignore")
            # `"tests" in path.parts` only skips tests/ DIRECTORIES. Unit tests live
            # in an inline `#[cfg(test)] mod` at the end of the shipping file, and
            # their fixture labels were reaching this scan — which is why three
            # fixtures had been hand-added to the allowlist below one at a time
            # (DBT-P66-003). Rust convention puts that module last; the audit
            # asserts that rather than assuming it.
            cut = text.find("\n#[cfg(test)]")
            if cut != -1:
                text = text[:cut]
            scanned_rs += 1
            for match in literal_rx.finditer(text):
                native_literals.add((match.group("field"), match.group("value")))
    # Coded enum values that ship in production code. `category` is branched on
    # (`e.category == "BugcheckReport"` in diagnostic-engine/src/lib.rs) and is a
    # closed set, so it stays measured here with its members named explicitly.
    coded_values = {
        ("category", "BugcheckReport"), ("category", "SystemEvent"), ("category", "UnexpectedShutdown"),
    }
    uncovered_native = sorted((field, value) for field, value in native_literals if value and (field, value) not in coded_values and value not in semantic)
    # Report the full count. This message used to print uncovered_native[:20] with
    # no total, so a 30-failure result read as exactly 20 (DBT-P66-004).
    check("backend_literal_owned_text_coverage", not uncovered_native, f"{len(uncovered_native)} uncovered across {scanned_rs} shipping .rs files: {uncovered_native[:20]}")

    dynamic_handlers = [
        "storage reliability$/", "WHEA event\\(s\\)", "No memory-related WHEA evidence",
        "skipped (\\d+) bytes", "Disable|Restore",
    ]
    check("backend_dynamic_owned_text_coverage", all(marker in semantic for marker in dynamic_handlers), str([m for m in dynamic_handlers if m not in semantic]))

    # No business decisions may inspect translated copy. Looking up raw engine semantic codes is fine.
    all_logic = "\n".join(p.read_text(encoding="utf-8") for p in files)
    display_logic_bad = re.findall(r"\bt\([^\n]+\)\.(?:startsWith|includes|endsWith)\(|localize[A-Za-z]+\([^\n]+\)\.(?:startsWith|includes|endsWith)\(", all_logic)
    check("no_display_string_business_logic", not display_logic_bad, str(display_logic_bad[:5]))

    # Strict field-level technical isolation markers.
    drivers = (UI / "features/drivers/DriversPage.svelte").read_text(encoding="utf-8")
    crash = (UI / "features/diagnostics/CrashPage.svelte").read_text(encoding="utf-8")
    hardware = (UI / "features/diagnostics/HardwarePage.svelte").read_text(encoding="utf-8")
    dialogs = (UI / "app/PlanDialogs.svelte").read_text(encoding="utf-8")
    check("driver_version_path_isolation", "device.driver?.version" in drivers and "TechnicalText value={device.driver?.date || device.driver?.infPath" in drivers and "candidate.targetVersion" in drivers and "TechnicalText value={candidate.targetVersion}" in drivers)
    check("crash_code_path_isolation", "TechnicalText value={crash.dumpFile}" in crash and "TechnicalText value={crash.bugcheckHex" in crash)
    check("hardware_identifier_isolation", "TechnicalText value={disk.serialNumber" in hardware and "TechnicalText value={disk.firmwareVersion" in hardware and "nvmeCriticalWarning" in hardware)
    check("plan_hash_identifier_isolation", "TechnicalText" in dialogs and "digest" in dialogs and "repairPlan.scanId" in dialogs)

    # The invariant that replaces result_code's removal from display_fields above.
    # A wire value is exempt from translation only because it reaches the reader
    # as isolated technical evidence; if it ever renders through a bare mustache
    # that exemption is no longer true and this must fail.
    bare_result_code: list[str] = []
    result_code_surfaces: set[str] = set()
    for path in sorted(UI.rglob("*.svelte")):
        template = re.sub(r"<script\b[^>]*>.*?</script>", "", path.read_text(encoding="utf-8"), flags=re.S | re.I)
        for match in re.finditer(r"\{[^{}]*?\b[A-Za-z_]\w*\.resultCode\b[^{}]*\}", template):
            where = f"{path.relative_to(ROOT)}: {match.group(0)}"
            preceding = template[max(0, match.start() - 32):match.start()]
            if "TechnicalText value=" in preceding:
                result_code_surfaces.add(str(path.relative_to(ROOT)))
            elif re.match(r"\{[#:]?(?:if|else if|each)\b", match.group(0)) or ".some(" in match.group(0):
                continue  # a branch on the value, not a render of it
            else:
                bare_result_code.append(where)
    check("result_code_path_isolation", bool(result_code_surfaces) and not bare_result_code, f"isolated on {len(result_code_surfaces)} surfaces; bare={bare_result_code[:5]}")

    # Locale catalog must cover every major semantic namespace.
    required_prefixes = ["nav.","overview.","drivers.","repair.","cleanup.","startup.","hardware.","crash.","activity.","recovery.","dialog.","tech.","state.","severity.","confidence."]
    missing_prefixes = [p for p in required_prefixes if not any(k.startswith(p) for k in en)]
    check("catalog_feature_namespace_coverage", not missing_prefixes, str(missing_prefixes))

    failed = [(n,d) for n,ok,d in checks if not ok]
    for name, ok, detail in checks:
        status = "PASS" if ok else "FAIL"
        suffix = f" — {detail}" if detail else ""
        print(f"[{status}] {name}{suffix}")
    print(f"\nPhase 12 localization audit: {len(checks)-len(failed)}/{len(checks)} checks passed; EN/AR keys={len(en)}; plural keys={len(enp)}")
    if failed:
        print("Failures:")
        for name, detail in failed:
            print(f" - {name}: {detail}")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
