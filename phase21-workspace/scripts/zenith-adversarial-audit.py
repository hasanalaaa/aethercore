#!/usr/bin/env python3
"""Platform-neutral Zenith regression gate.

This gate intentionally augments, rather than replaces, the historical Phase 0-16 and
Enterprise gates. It proves source-level interaction/accessibility/integration invariants
that were found during the Zenith adversarial pass. Windows runtime qualification remains
owned by verify-enterprise/verify-phase16/verify-production.
"""
from __future__ import annotations

import argparse, json, re, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PARSER = argparse.ArgumentParser()
PARSER.add_argument("--output", type=Path)
ARGS = PARSER.parse_args()
checks: dict[str, dict[str, object]] = {}


# P59 / DBT-P58-005: one shared reader that raises instead of substituting "".
# P58 fixed where this reader looked - `.github/` resolves against the
# repository root - but not what it did when the look failed, so a check
# asserting something is ABSENT still passed against a file never opened.
# No bytecode: `omega-evidence.py` runs each gate against a disposable clone
# and treats ANY new file in it as a source mutation, so a `__pycache__`
# entry for this import would be reported as the gate rewriting the tree.
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from gate_reader import SourceReader  # noqa: E402

read = SourceReader(ROOT).read


def check(name: str, ok: bool, **details: object) -> None:
    checks[name] = {"ok": bool(ok), **details}


def has(text: str, *tokens: str) -> bool:
    return all(token in text for token in tokens)


def ordered(text: str, *tokens: str) -> bool:
    """True only when every token exists and appears in the requested order."""
    cursor = 0
    for token in tokens:
        position = text.find(token, cursor)
        if position < 0:
            return False
        cursor = position + len(token)
    return True


def section(text: str, start: str, end: str) -> str:
    """Return a bounded source section; missing delimiters fail closed to an empty string."""
    begin = text.find(start)
    if begin < 0:
        return ""
    finish = text.find(end, begin + len(start))
    if finish < 0:
        return ""
    return text[begin:finish]


press = read("apps/ui/src/design/motion/fluid-press.ts")
drag = read("apps/ui/src/design/motion/fluid-drag.ts")
dialog = read("apps/ui/src/design/primitives/FluidDialog.svelte")
progress = read("apps/ui/src/design/primitives/ProgressBar.svelte")
system_care = read("apps/ui/src/features/system-care/SystemCarePanel.svelte")
drivers = read("apps/ui/src/features/drivers/DriversPage.svelte")
startup = read("apps/ui/src/features/startup/StartupPage.svelte")
crash = read("apps/ui/src/features/diagnostics/CrashPage.svelte")
hardware = read("apps/ui/src/features/diagnostics/HardwarePage.svelte")
motion_css = read("apps/ui/src/design/styles/motion.css")
feature_css = read("apps/ui/src/design/styles/feature-layout.css")
verify_zenith = read("scripts/verify-zenith.ps1")
verify_production = read("scripts/verify-production.ps1")
ci = read(".github/workflows/ci.yml")
release = read(".github/workflows/release.yml")
package = read("package.json")

required = [
    "scripts/zenith-adversarial-audit.py",
    "scripts/zenith-adversarial-audit.ps1",
    "scripts/verify-zenith.ps1",
    "ZENITH_ISSUE_LEDGER.md",
    "ZENITH_TRANSFORMATION_MATRIX.md",
    "ZENITH_ADVERSARIAL_AUDIT.md",
    "ZENITH_UI_UX_RECONSTRUCTION.md",
    "ZENITH_FEATURE_EVOLUTION.md",
    "ZENITH_VERIFICATION_SUMMARY.md",
    "ZENITH_REMAINING_RISKS.md",
    "ZENITH_ARCHITECTURE_MAP.md",
    "ZENITH_FINAL_INTEGRATOR_REVIEW.md",
    "ZENITH_DELIVERABLES.md",
    "docs/adr/0020-zenith-interaction-resilience.md",
]
check("zenith_required_artifacts", all((ROOT / rel).is_file() for rel in required), missing=[rel for rel in required if not (ROOT / rel).is_file()])

# ZEN-001: reduced-motion dialog close is exactly-once and closed-state preference updates are inert.
check("dialog_close_has_idempotent_guard", has(dialog, "function completeClose(): void", "if (!rendered) return;", "returnFocus = null;", "onClosed();"))
check("dialog_close_callback_has_single_callsite", dialog.count("onClosed();") == 1, callsites=dialog.count("onClosed();"))
reduced_branch = section(dialog, "if (reducedMotion) {", "} else {")
check("dialog_reduced_motion_uses_spring_single_source", "spring.set(open ? 1 : 0);" in reduced_branch and "onClosed" not in reduced_branch)
check("dialog_spring_settled_delegates_guarded_close", has(dialog, "settled && !latestOpen", "completeClose();"))

# ZEN-002: pointer capture must not turn drag-away cancellation into a native click.
check("press_pointer_capture_and_hysteresis_preserved", has(press, "setPointerCapture", "hysteresis()", "data-pressed"))
check("press_dragaway_click_fence", has(press, "suppressNextClick = !armed;", "event.preventDefault();", "event.stopImmediatePropagation();", "addEventListener('click', clickCapture, true)"))
check("press_click_fence_removed_on_destroy", "removeEventListener('click', clickCapture, true)" in press)
check("press_keyboard_activation_not_suppressed", re.search(r"const keydown[\s\S]*?suppressNextClick = false;[\s\S]*?setArmed\(true\)", press) is not None)
check("press_action_options_are_reactive", has(press, "update(next: FluidPressOptions)", "config = { ...next };", "scale.retarget(pressedScale())"))

# ZEN-003: cancellation/system loss is not a user flick and must not inherit momentum.
check("drag_release_preserves_user_momentum", "const release = (event: PointerEvent) => settle(event, true);" in drag)
check("drag_cancel_drops_momentum", has(drag, "const cancel = (event: PointerEvent) => settle(event, false);", "pointercancel', cancel", "lostpointercapture', cancel"))
check("drag_cancel_velocity_is_zero", "const velocity = preserveMomentum ? estimateVelocity(samples) : 0;" in drag)

# ZEN-004/005: semantic state/progress is exposed to assistive technology and progress stays compositor-friendly.
check("update_channel_is_aria_pressed", system_care.count("aria-pressed={$systemCareUi.channel===") >= 2)
check("driver_filters_are_aria_pressed", re.search(r"aria-pressed=\{filter\s*===\s*name\}", drivers) is not None)
check("startup_decisions_are_radio_group", has(startup, 'role="radiogroup"', 'role="radio"', 'aria-checked={decisionFor(item)', 'tabindex={decisionFor(item)'))
check("startup_radio_keyboard_contract", has(startup, "function decisionKeydown", "'ArrowDown'", "'ArrowUp'", "'Home'", "'End'", "locale === 'ar' ? 'ArrowLeft' : 'ArrowRight'"))
check("startup_radio_skips_forbidden_disable", "decision !== 'Disable' || (item.manageable && !item.protected)" in startup)
check("system_care_reuses_semantic_progress", has(system_care, "import ProgressBar", "<ProgressBar value={update.overallPercent}"))
check("semantic_progressbar_contract", has(progress, 'role="progressbar"', 'aria-valuemin="0"', 'aria-valuemax="100"', "aria-valuenow={Math.round(target)}"))
check("progress_motion_uses_transform_not_width", "transform:scaleX" in progress and "style={`width:" not in system_care)

# ZEN-006/007: spatial indicators and transform origins honor RTL without reordering technical identifiers.
check("guided_actions_no_literal_spatial_arrow", "→ <LocalizedOwnedText" not in crash and "→ <LocalizedOwnedText" not in hardware)
check("guided_actions_use_semantic_class", 'class="guided-action"' in crash and 'class="guided-action"' in hardware)
check("guided_actions_mirror_in_rtl", re.search(r'\.guided-action::before\s*\{[^}]*content\s*:\s*"→"', feature_css) is not None and re.search(r'\[dir="rtl"\]\s+\.guided-action::before\s*\{[^}]*content\s*:\s*"←"', feature_css) is not None)
check("progress_origin_mirrors_in_rtl", has(motion_css, "transform-origin:left center", '[dir="rtl"] .ac-progress>span { transform-origin:right center; }'))

# Cross-cutting UI discipline from the supplied fluid-interface specification.
ui_files = [p for p in (ROOT / "apps/ui/src").rglob("*") if p.suffix in {".ts", ".svelte", ".css"}]
ui_text = "\n".join(p.read_text(encoding="utf-8", errors="ignore") for p in ui_files)
check("ui_zero_interval_polling", re.search(r"\bsetInterval\s*\(|\bclearInterval\s*\(", ui_text) is None)
check("ui_zero_interactive_any_escape", " as any" not in ui_text and "as unknown as" not in ui_text and "@ts-ignore" not in ui_text and "@ts-expect-error" not in ui_text)
check("ui_no_transform_or_all_fixed_transitions", re.search(r"transition\s*:[^;]*(?:transform|all)", ui_text) is None)
check("ui_no_important_override", "!important" not in ui_text)

# The Zenith gate is additive: it must inherit Enterprise, and CI/release/GA paths must enforce Zenith.
check("verify_zenith_inherits_enterprise", has(verify_zenith, "verify-enterprise.ps1", "zenith-adversarial-audit.ps1"))
check("verify_zenith_packages_only_after_zenith_audit", ordered(verify_zenith, "zenith-adversarial-audit.ps1", "if($ReleasePackaging)"))
check("ci_runs_zenith_source_gate", "zenith-adversarial-audit.ps1" in ci)
check("release_runs_zenith_before_packaging", ordered(release, "zenith-adversarial-audit.ps1", "verify-enterprise.ps1 -ReleasePackaging"))
check("ga_seal_requires_zenith_audit", "zenith-adversarial-audit.ps1" in verify_production)
check("package_exposes_zenith_gate", "verify:zenith" in package)

failed = [name for name, value in checks.items() if not value.get("ok")]
report = {
    "schema": "aethercore.zenith-adversarial-audit.v1",
    "ok": not failed,
    "check_count": len(checks),
    "failed": failed,
    "checks": checks,
    "qualification_boundary": [
        "This gate is platform-neutral and proves source-level Zenith invariants only.",
        "It does not replace cargo fmt/clippy/tests, Svelte checking/building, Windows FFI/runtime, WebView2, Authenticode, installer lifecycle, accessibility host, stress, or soak qualification.",
        "The authoritative Windows path remains verify-zenith.ps1 plus verify-production.ps1 for a release seal.",
    ],
}
if ARGS.output:
    ARGS.output.parent.mkdir(parents=True, exist_ok=True)
    ARGS.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
print(json.dumps({"ok": report["ok"], "checks": len(checks), "failed": failed}, indent=2))
sys.exit(0 if report["ok"] else 1)
