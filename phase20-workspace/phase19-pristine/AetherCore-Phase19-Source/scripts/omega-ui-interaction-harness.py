#!/usr/bin/env python3
"""Browser-level source interaction harness for AetherCore motion/accessibility primitives.

The harness compiles the platform-neutral motion TypeScript that ships in the repository, creates a
small temporary browser bundle, and drives that code in system Chromium via Playwright. It is source
interaction evidence only: it does not replace the pinned pnpm build or Windows WebView2/Narrator/DPI
qualification.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import time
from typing import Any, Callable

ROOT = Path(__file__).resolve().parents[1]
MOTION = ROOT / "apps/ui/src/design/motion"
STYLES = ROOT / "apps/ui/src/design/styles"
TOKENS = ROOT / "apps/ui/src/design-tokens.css"


def run(command: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(command, cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)


def compile_motion(out_dir: Path) -> dict[str, Any]:
    tsc = shutil.which("tsc")
    if not tsc:
        raise RuntimeError("global TypeScript compiler is unavailable")
    command = [
        tsc,
        *(str(MOTION / name) for name in ("physics.ts", "spring.ts", "preferences.ts", "fluid-press.ts", "fluid-drag.ts")),
        "--target", "ES2022", "--module", "ES2022", "--moduleResolution", "bundler",
        "--lib", "ES2022,DOM", "--strict", "--skipLibCheck", "--outDir", str(out_dir),
    ]
    result = run(command)
    if result.returncode:
        raise RuntimeError(f"TypeScript source compilation failed:\n{result.stdout}")
    version = run([tsc, "--version"]).stdout.strip()
    return {"compiler": tsc, "compiler_version": version, "pinned_project_compiler": "6.0.3"}


def browser_bundle(out_dir: Path) -> str:
    """Bundle the five compiler outputs for an in-memory page without adding a bundler dependency."""
    chunks: list[str] = []
    for name in ("physics.js", "spring.js", "preferences.js", "fluid-press.js", "fluid-drag.js"):
        source = (out_dir / name).read_text(encoding="utf-8")
        source = re.sub(r"^import\s+.*?;\s*$", "", source, flags=re.MULTILINE)
        source = re.sub(r"^export\s+", "", source, flags=re.MULTILINE)
        chunks.append(f"// {name}\n{source}")
    chunks.append("window.AetherMotion = { fluidPress, fluidDrag, SpringValue, projectMomentum, projectedEndpoint, rubberband, estimateVelocity, nearestSnapPoint };\n")
    return "\n".join(chunks)


def harness_html() -> str:
    css_parts: list[str] = []
    for source in (TOKENS, STYLES / "base.css", STYLES / "materials.css", STYLES / "typography.css", STYLES / "responsive.css"):
        if source.exists():
            css_parts.append(f"/* {source.relative_to(ROOT)} */\n{source.read_text(encoding='utf-8')}")
    css = "\n".join(css_parts)
    return f'''<!doctype html><html lang="en" dir="ltr"><head><meta charset="utf-8"><style>{css}
html,body{{margin:0;padding:0;min-width:800px;min-height:600px;background:#0b1016;color:#fff}}
#stage{{padding:80px;display:grid;gap:48px;align-items:start}}
#press{{width:180px;height:56px}}
#drag{{width:120px;height:64px;touch-action:none;background:rgba(255,255,255,.12)}}
.ac-material{{width:240px;height:60px;background:rgba(20,30,40,.55);backdrop-filter:blur(20px)}}
</style></head><body><div id="stage">
<button id="press" type="button">Press target</button>
<div id="drag" tabindex="0">Drag target</div>
<div id="material" class="ac-material">Material</div>
<h1 id="arabic">أثير كور</h1>
<code id="technical" class="technical-isolate">S-1-5-80-1234 C:\\Users\\example</code>
</div></body></html>'''


INIT = """
const press = document.querySelector('#press');
const drag = document.querySelector('#drag');
window.__pressClicks = 0;
press.addEventListener('click', () => { window.__pressClicks += 1; });
window.__dragPositions = [];
window.__pressAction = AetherMotion.fluidPress(press, { pressedScale: .97, hysteresis: 10 });
window.__dragOptions = {
  axis: 'x', initial: 0, min: 0, max: 100, snapPoints: [0, 100], rubberbandDimension: 120,
  onPosition: (value, settled) => window.__dragPositions.push({ value, settled, t: performance.now() })
};
window.__dragAction = AetherMotion.fluidDrag(drag, window.__dragOptions);
window.__ready = true;
"""


def expect(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def execute_context(browser: Any, scale: float, html: str, bundle: str) -> list[dict[str, Any]]:
    context = browser.new_context(viewport={"width": 1200, "height": 800}, device_scale_factor=scale)
    page = context.new_page()
    page.set_content(html, wait_until="load")
    page.add_script_tag(content=bundle)
    page.evaluate(INIT)
    results: list[dict[str, Any]] = []

    def record(name: str, fn: Callable[[], dict[str, Any] | None]) -> None:
        start = time.perf_counter()
        try:
            details = fn() or {}
            results.append({"name": name, "status": "PASS", "duration_ms": round((time.perf_counter()-start)*1000, 3), "details": details})
        except Exception as exc:
            results.append({"name": name, "status": "FAIL", "duration_ms": round((time.perf_counter()-start)*1000, 3), "error": str(exc)})

    press = page.locator("#press")
    drag = page.locator("#drag")

    def pointerdown_feedback() -> dict[str, Any]:
        box = press.bounding_box(); assert box
        page.mouse.move(box["x"] + box["width"]/2, box["y"] + box["height"]/2)
        page.mouse.down()
        armed = press.get_attribute("data-pressed") is not None
        transform = press.evaluate("el => el.style.transform")
        expect(armed, "data-pressed was not set synchronously on pointerdown")
        page.mouse.up()
        return {"armed_before_pointerup": armed, "presentation_transform": transform}
    record("pointerdown-immediate-feedback", pointerdown_feedback)

    def cancel_rearm() -> dict[str, Any]:
        box = press.bounding_box(); assert box
        cx, cy = box["x"] + box["width"]/2, box["y"] + box["height"]/2
        page.mouse.move(cx, cy); page.mouse.down()
        page.mouse.move(box["x"] + box["width"] + 80, cy)
        canceled = press.get_attribute("data-pressed") is None
        page.mouse.move(cx, cy)
        rearmed = press.get_attribute("data-pressed") is not None
        page.mouse.up()
        expect(canceled, "drag-away did not cancel press under pointer capture")
        expect(rearmed, "drag-back did not re-arm press")
        return {"canceled_outside": canceled, "rearmed_inside": rearmed}
    record("pointer-capture-cancel-rearm", cancel_rearm)

    def cancel_click() -> dict[str, Any]:
        page.evaluate("window.__pressClicks = 0")
        box = press.bounding_box(); assert box
        cx, cy = box["x"] + box["width"]/2, box["y"] + box["height"]/2
        page.mouse.move(cx, cy); page.mouse.down(); page.mouse.move(box["x"] + box["width"] + 100, cy); page.mouse.up()
        clicks = page.evaluate("window.__pressClicks")
        expect(clicks == 0, f"canceled pointer sequence leaked {clicks} click(s)")
        return {"committed_clicks": clicks}
    record("cancelled-press-does-not-commit", cancel_click)

    def keyboard_feedback() -> dict[str, Any]:
        press.focus(); page.keyboard.down("Space")
        armed = press.get_attribute("data-pressed") is not None
        page.keyboard.up("Space")
        disarmed = press.get_attribute("data-pressed") is None
        expect(armed and disarmed, "Space feedback did not arm/disarm")
        return {"armed_on_keydown": armed, "disarmed_on_keyup": disarmed}
    record("keyboard-press-feedback", keyboard_feedback)

    def direct_drag() -> dict[str, Any]:
        page.evaluate("window.__dragAction.update(window.__dragOptions); window.__dragPositions=[]")
        page.wait_for_timeout(500)
        box = drag.bounding_box(); assert box
        x, y = box["x"] + 20, box["y"] + box["height"]/2
        page.mouse.move(x, y); page.mouse.down(); page.mouse.move(x + 40, y, steps=1)
        translate = drag.evaluate("el => el.style.translate")
        dragging = drag.get_attribute("data-dragging") is not None
        page.mouse.up(); page.wait_for_timeout(900)
        final_translate = drag.evaluate("el => el.style.translate")
        samples = page.evaluate("window.__dragPositions.slice(-8)")
        expect(dragging, "dragging state missing during direct manipulation")
        during_value = float(translate.split("px", 1)[0])
        final_value = float(final_translate.split("px", 1)[0])
        expect(abs(during_value - 40.0) <= 0.01, f"drag did not track 1:1: {translate}")
        expect(min(abs(final_value), abs(final_value - 100.0)) <= 0.01, f"drag did not settle to snap point: {final_translate}")
        return {"during_drag_translate": translate, "settled_translate": final_translate, "tail_samples": samples}
    record("drag-1to1-and-snap", direct_drag)

    def reduced_motion() -> dict[str, Any]:
        page.evaluate("document.documentElement.dataset.motion='reduced'"); page.wait_for_timeout(50)
        box = press.bounding_box(); assert box
        page.mouse.move(box["x"]+20, box["y"]+20); page.mouse.down()
        transform = press.evaluate("el => el.style.transform")
        armed = press.get_attribute("data-pressed") is not None
        page.mouse.up()
        expect(armed, "reduced motion removed semantic press feedback")
        expect(transform == "", f"reduced motion still applies transform: {transform}")
        page.evaluate("document.documentElement.dataset.motion='normal'"); page.wait_for_timeout(30)
        return {"semantic_feedback_preserved": armed, "transform": transform}
    record("reduced-motion-nonvestibular-feedback", reduced_motion)

    def reduced_transparency() -> dict[str, Any]:
        page.evaluate("document.documentElement.dataset.transparency='reduced'"); page.wait_for_timeout(30)
        values = page.locator("#material").evaluate("el => ({backdrop:getComputedStyle(el).backdropFilter, background:getComputedStyle(el).backgroundColor})")
        expect(values["backdrop"] == "none", f"reduced transparency retained blur: {values}")
        page.evaluate("document.documentElement.dataset.transparency='normal'")
        return values
    record("reduced-transparency-material", reduced_transparency)

    def rtl_typography() -> dict[str, Any]:
        page.evaluate("document.documentElement.lang='ar'; document.documentElement.dir='rtl'"); page.wait_for_timeout(30)
        values = page.evaluate("({bodyDirection:getComputedStyle(document.body).direction, technicalDirection:getComputedStyle(document.querySelector('#technical')).direction, technicalBidi:getComputedStyle(document.querySelector('#technical')).unicodeBidi, headingTracking:getComputedStyle(document.querySelector('#arabic')).letterSpacing})")
        expect(values["bodyDirection"] == "rtl", f"document did not compute RTL: {values}")
        expect(values["technicalDirection"] == "ltr", f"technical identifier lost LTR isolation: {values}")
        expect(values["technicalBidi"] in {"isolate", "isolate-override"}, f"technical identifier lacks bidi isolation: {values}")
        return values
    record("arabic-rtl-technical-isolation", rtl_typography)

    def layout_sanity() -> dict[str, Any]:
        metrics = page.evaluate("({sw:document.documentElement.scrollWidth,cw:document.documentElement.clientWidth,sh:document.documentElement.scrollHeight,ch:document.documentElement.clientHeight})")
        expect(metrics["sw"] <= metrics["cw"] + 1, f"unexpected horizontal overflow at scale {scale}: {metrics}")
        return {"device_scale_factor": scale, **metrics}
    record(f"layout-scale-{scale:g}", layout_sanity)

    context.close()
    return results


def forced_colors_check(browser: Any, html: str, bundle: str) -> dict[str, Any]:
    start = time.perf_counter()
    try:
        context = browser.new_context(viewport={"width": 1200, "height": 800}, forced_colors="active")
        page = context.new_page(); page.set_content(html); page.add_script_tag(content=bundle); page.evaluate(INIT)
        active = page.evaluate("matchMedia('(forced-colors: active)').matches")
        outline = page.locator("#press").evaluate("el => getComputedStyle(el).outlineStyle")
        context.close()
        expect(active, "forced-colors media emulation did not activate")
        return {"name": "forced-colors-browser-emulation", "status": "PASS", "duration_ms": round((time.perf_counter()-start)*1000,3), "details": {"media_active": active, "outline_style": outline}}
    except Exception as exc:
        return {"name": "forced-colors-browser-emulation", "status": "FAIL", "duration_ms": round((time.perf_counter()-start)*1000,3), "error": str(exc)}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", type=Path)
    parser.add_argument("--chromium", default=os.environ.get("AETHERCORE_CHROMIUM") or ("/usr/bin/chromium" if Path("/usr/bin/chromium").is_file() else None))
    args = parser.parse_args()
    evidence: dict[str, Any] = {
        "schema": "aethercore.ui-browser-evidence.v1",
        "scope": "platform-neutral browser interaction source harness",
        "native_equivalence": False,
        "limitations": [
            "Uses the locally available global TypeScript compiler, not the dependency-frozen pnpm toolchain.",
            "System Chromium is not WebView2; this does not qualify Windows WebView2, Narrator, or native DPI behavior.",
            "deviceScaleFactor is browser emulation and is not a substitute for Windows 125/150/200 percent DPI qualification.",
        ],
        "checks": [],
    }
    fatal: str | None = None
    try:
        with tempfile.TemporaryDirectory(prefix="aethercore-omega-ui-") as temp:
            out = Path(temp) / "motion"; out.mkdir()
            evidence["typescript"] = compile_motion(out)
            bundle = browser_bundle(out); html = harness_html()
            from playwright.sync_api import sync_playwright
            with sync_playwright() as p:
                launch = {"headless": True, "args": ["--no-sandbox"]}
                if args.chromium:
                    launch["executable_path"] = args.chromium
                browser = p.chromium.launch(**launch)
                for scale in (1.0, 1.25, 1.5, 2.0):
                    evidence["checks"].extend(execute_context(browser, scale, html, bundle))
                evidence["checks"].append(forced_colors_check(browser, html, bundle))
                browser.close()
    except Exception as exc:
        fatal = str(exc); evidence["harness_error"] = fatal
    failures = [c for c in evidence["checks"] if c.get("status") != "PASS"]
    evidence["summary"] = {
        "total": len(evidence["checks"]), "passed": len(evidence["checks"])-len(failures), "failed": len(failures),
        "status": "PASS" if not failures and fatal is None else "FAIL",
    }
    rendered = json.dumps(evidence, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True); args.json.write_text(rendered, encoding="utf-8")
    print(rendered, end="")
    return 0 if evidence["summary"]["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
