#!/usr/bin/env python3
"""Exercise the real AetherCore Svelte component tree with a deterministic transport boundary.

Unlike omega-ui-interaction-harness.py, this suite is intentionally full-application. It builds and
loads apps/ui itself, installs a typed-equivalent transport before the module graph executes, and
then drives the same App -> AppShell -> feature component tree used by production.

If the pinned pnpm dependency graph is unavailable, the script reports BLOCKED rather than falling
back to a synthetic page or claiming E2E coverage from source inspection.
"""
from __future__ import annotations

import argparse
from contextlib import contextmanager
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import threading
import time
from typing import Any, Callable, Iterator

ROOT = Path(__file__).resolve().parents[1]
UI = ROOT / "apps/ui"


def run(command: list[str], cwd: Path = ROOT, timeout: int = 300, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    merged_env = os.environ.copy()
    if env:
        merged_env.update(env)
    return subprocess.run(command, cwd=cwd, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout, env=merged_env)


def source_contract() -> dict[str, Any]:
    app = (UI / "src/App.svelte").read_text(encoding="utf-8")
    shell = (UI / "src/app/AppShell.svelte").read_text(encoding="utf-8")
    service = (UI / "src/platform/service-client.ts").read_text(encoding="utf-8")
    kernel = (UI / "src/platform/kernel-session.ts").read_text(encoding="utf-8")
    transport = (UI / "src/platform/transport.ts").read_text(encoding="utf-8")
    pages = {
        "overview": "OverviewPage",
        "drivers": "DriversPage",
        "repair": "RepairPage",
        "cleanup": "CleanupPage",
        "startup": "StartupPage",
        "hardware": "HardwarePage",
        "crash": "CrashPage",
        "activity": "ActivityPage",
    }
    checks = {
        "production_root_uses_app_shell": "<AppShell />" in app and "./app/AppShell.svelte" in app,
        "all_navigation_surfaces_in_production_shell": all(name in shell for name in pages.values()),
        "update_and_support_surface_in_production_tree": "SystemCarePanel" in (UI / "src/features/overview/OverviewPage.svelte").read_text(encoding="utf-8"),
        "service_calls_use_transport_boundary": "uiTransport.invoke" in service and "@tauri-apps/api" not in service,
        "kernel_events_use_transport_boundary": "uiTransport.listen" in kernel and "uiTransport.invoke" in kernel and "@tauri-apps/api" not in kernel,
        "test_transport_is_startup_only_and_build_gated": "export const uiTransport" in transport and "__AETHERCORE_TEST_TRANSPORT__" in transport and "VITE_AETHERCORE_TEST_TRANSPORT" in transport and "setTransport" not in transport,
    }
    return {"ok": all(checks.values()), "checks": checks, "pages": pages}


MOCK_TRANSPORT = r"""
(() => {
  const listeners = new Map();
  const calls = [];
  const session = { connected:true, sessionId:'e2e-session', serviceVersion:'e2e-1.0.0', currentSequence:0, replayFloorSequence:0, replayComplete:true };
  const updateReady = {
    state:5, channel:1, currentVersion:'0.1.0',
    latestRelease:{releaseId:'release-1',version:'0.2.0',channel:1,publishedUnixMs:1700000000000,notesMessageKey:'update.status.available',minimumWindowsBuild:19045,sizeBytes:1048576,sha256:'ab'.repeat(32),packageKind:1},
    stagedRelease:null, progressKnown:false, overallPercent:0, bytesCompleted:0, bytesTotal:1048576,
    statusMessageKey:'update.status.available', checkedUnixMs:1700000000000, updatedUnixMs:1700000000000
  };
  const supportPreview = {
    previewId:'preview-e2e', expiresUnixMs:1700000600000,
    sections:[{fileName:'diagnostics.json',displayKey:'support.section.diagnostics',sizeBytes:512}],
    privacy:{userPathRedactions:2,accountIdentifierRedactions:1,hardwareSerialRedactions:1,emailRedactions:1},
    estimatedSizeBytes:1024
  };
  const handlers = (event) => {
    let set = listeners.get(event);
    if (!set) { set = new Set(); listeners.set(event, set); }
    return set;
  };
  window.__AETHERCORE_TEST_CALLS__ = calls;
  window.__AETHERCORE_TEST_EMIT = (event, payload) => { for (const handler of handlers(event)) handler({payload}); };
  window.__AETHERCORE_TEST_TRANSPORT__ = {
    listen: async (event, handler) => { const set=handlers(event); set.add(handler); return () => set.delete(handler); },
    invoke: async (command, args) => {
      calls.push({command,args:args ?? null});
      if (command === 'start_ipc_session') return session;
      if (command === 'check_for_updates') return {...updateReady, channel: Number(args?.channel ?? 1)};
      if (command === 'create_support_bundle_preview') return supportPreview;
      if (command === 'export_support_bundle') return {path:'C:\\AetherCore\\support-e2e.tar',sha256:'cd'.repeat(32),verificationFingerprintSha256:'ef'.repeat(32)};
      if (command === 'get_snapshot') return {connected:true,serviceVersion:'e2e-1.0.0',health:'PlatformReady',serverTimeUnixMs:1700000000000,journalEventCount:0,activePlan:null};
      throw new Error(`E2E mock has no deterministic response for command: ${command}`);
    }
  };
})();
"""


class QuietHandler(SimpleHTTPRequestHandler):
    def log_message(self, _format: str, *_args: object) -> None:
        return


@contextmanager
def serve(directory: Path) -> Iterator[str]:
    handler = lambda *args, **kwargs: QuietHandler(*args, directory=str(directory), **kwargs)  # noqa: E731
    server = ThreadingHTTPServer(("127.0.0.1", 0), handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_port}/"
    finally:
        server.shutdown(); server.server_close(); thread.join(timeout=2)


def expect(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def browser_checks(dist: Path, chromium: str | None) -> tuple[list[dict[str, Any]], list[str]]:
    from playwright.sync_api import sync_playwright
    checks: list[dict[str, Any]] = []
    console_errors: list[str] = []

    def record(name: str, fn: Callable[[], dict[str, Any] | None]) -> None:
        started = time.perf_counter()
        try:
            detail = fn() or {}
            checks.append({"name": name, "status": "PASS", "duration_ms": round((time.perf_counter()-started)*1000, 3), "details": detail})
        except Exception as exc:
            checks.append({"name": name, "status": "FAIL", "duration_ms": round((time.perf_counter()-started)*1000, 3), "error": str(exc)})

    with serve(dist) as url, sync_playwright() as p:
        launch: dict[str, Any] = {"headless": True, "args": ["--no-sandbox"]}
        if chromium:
            launch["executable_path"] = chromium
        browser = p.chromium.launch(**launch)
        context = browser.new_context(viewport={"width": 1440, "height": 960})
        context.add_init_script(MOCK_TRANSPORT)
        page = context.new_page()
        page.on("console", lambda msg: console_errors.append(msg.text) if msg.type == "error" else None)
        page.on("pageerror", lambda exc: console_errors.append(str(exc)))
        page.goto(url, wait_until="networkidle")
        page.locator(".app-shell").wait_for(state="visible")

        def root_and_session() -> dict[str, Any]:
            nav_count = page.locator("button[data-nav-item]").count()
            active = page.locator(".app-shell").get_attribute("data-page")
            calls = page.evaluate("window.__AETHERCORE_TEST_CALLS__")
            expect(active == "overview", f"unexpected initial page: {active}")
            expect(nav_count == 8, f"expected 8 production navigation surfaces, got {nav_count}")
            expect(any(c["command"] == "start_ipc_session" for c in calls), "production AppShell did not start the IPC session")
            return {"active_page": active, "navigation_items": nav_count, "transport_calls": calls}
        record("production-root-and-session", root_and_session)

        def navigate_all() -> dict[str, Any]:
            visited: list[str] = []
            for page_id in ("drivers","repair","cleanup","startup","hardware","crash","activity","overview"):
                page.locator("button[data-nav-item]").nth({"overview":0,"drivers":1,"repair":2,"cleanup":3,"startup":4,"hardware":5,"crash":6,"activity":7}[page_id]).click()
                page.wait_for_function("id => document.querySelector('.app-shell')?.getAttribute('data-page') === id", page_id)
                expect(page.locator("main h1").count() >= 1, f"{page_id} surface did not render a primary heading")
                visited.append(page_id)
            return {"visited": visited}
        record("all-production-navigation-surfaces", navigate_all)

        def keyboard_shortcut() -> dict[str, Any]:
            page.keyboard.press("Control+Shift+6")
            page.wait_for_function("document.querySelector('.app-shell')?.getAttribute('data-page') === 'hardware'")
            return {"active_page": page.locator(".app-shell").get_attribute("data-page")}
        record("global-keyboard-navigation", keyboard_shortcut)

        def palette_accessibility() -> dict[str, Any]:
            page.keyboard.press("Control+K")
            dialog = page.get_by_role("dialog")
            dialog.wait_for(state="visible")
            page.wait_for_timeout(30)
            focused = page.evaluate("document.activeElement?.id")
            expect(focused == "command-input", f"palette did not focus search input: {focused}")
            page.keyboard.press("Escape")
            dialog.wait_for(state="hidden")
            page.wait_for_timeout(80)
            expanded = page.locator("button.command-trigger").get_attribute("aria-expanded")
            expect(expanded == "false", "command trigger remained expanded after Escape")
            return {"initial_focus": focused, "aria_expanded_after_close": expanded}
        record("command-palette-focus-and-escape", palette_accessibility)

        def rtl_locale() -> dict[str, Any]:
            page.locator("button.locale-button").click()
            page.wait_for_function("document.documentElement.dir === 'rtl' && document.documentElement.lang === 'ar'")
            values = page.evaluate("({dir:document.documentElement.dir,lang:document.documentElement.lang,overflow:document.documentElement.scrollWidth-document.documentElement.clientWidth})")
            expect(values["overflow"] <= 1, f"RTL introduced horizontal overflow: {values}")
            page.locator("button.locale-button").click()
            return values
        record("arabic-rtl-real-app-layout", rtl_locale)

        def system_care() -> dict[str, Any]:
            page.keyboard.press("Control+Shift+1")
            page.wait_for_function("document.querySelector('.app-shell')?.getAttribute('data-page') === 'overview'")
            care = page.locator("section.system-care-grid")
            expect(care.count() == 1, "system-care production surface missing")
            update_card = care.locator("article.system-care-card").nth(0)
            support_card = care.locator("article.system-care-card").nth(1)
            update_card.locator("button.secondary").click()
            page.wait_for_function("window.__AETHERCORE_TEST_CALLS__.some(c => c.command === 'check_for_updates')")
            support_card.locator("button.secondary").click()
            page.wait_for_function("window.__AETHERCORE_TEST_CALLS__.some(c => c.command === 'create_support_bundle_preview')")
            support_card.locator(".support-preview").wait_for(state="visible")
            calls = page.evaluate("window.__AETHERCORE_TEST_CALLS__.map(c => c.command)")
            return {"calls": calls, "preview_visible": True}
        record("update-and-support-production-workflows", system_care)

        def disconnect_and_reset() -> dict[str, Any]:
            page.evaluate("window.__AETHERCORE_TEST_EMIT('aethercore://session-state',{connected:false,sessionId:'',serviceVersion:'',currentSequence:0,replayFloorSequence:0,replayComplete:false})")
            page.wait_for_timeout(30)
            offline = page.locator(".app-service .service-copy strong").inner_text()
            page.evaluate("window.__AETHERCORE_TEST_EMIT('aethercore://stream-reset',{reason:'Lagged',currentSequence:12,replayFloorSequence:8,messageKey:'stream.reset'})")
            page.wait_for_timeout(30)
            live = page.locator(".a11y-live").inner_text()
            expect(bool(live.strip()), "stream reset did not produce a restrained live announcement")
            return {"service_label": offline, "live_announcement": live}
        record("disconnected-and-stream-reset-states", disconnect_and_reset)

        def layout_and_semantics() -> dict[str, Any]:
            values = page.evaluate("({sw:document.documentElement.scrollWidth,cw:document.documentElement.clientWidth,main:document.querySelectorAll('main').length,nav:document.querySelectorAll('nav').length,dialogs:document.querySelectorAll('[role=dialog]').length})")
            expect(values["sw"] <= values["cw"] + 1, f"production app has horizontal overflow: {values}")
            expect(values["main"] == 1, f"expected one main landmark: {values}")
            expect(values["nav"] >= 1, f"navigation landmark missing: {values}")
            return values
        record("production-layout-and-landmarks", layout_and_semantics)

        context.close(); browser.close()
    return checks, console_errors


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", type=Path)
    ap.add_argument("--source-only", action="store_true")
    ap.add_argument("--skip-build", action="store_true")
    ap.add_argument("--dist", type=Path, default=UI / "dist")
    ap.add_argument("--chromium", default=os.environ.get("AETHERCORE_CHROMIUM") or ("/usr/bin/chromium" if Path("/usr/bin/chromium").is_file() else None))
    args = ap.parse_args()

    evidence: dict[str, Any] = {
        "schema": "aethercore.sigma-master-full-app-ui.v1",
        "scope": "real Svelte application component tree",
        "native_equivalence": False,
        "source_contract": source_contract(),
        "build": {},
        "checks": [],
        "limitations": [
            "System Chromium is not Windows WebView2 and does not qualify Narrator/native DPI/Mica integration.",
            "The deterministic transport exercises the production component tree without requiring a Windows service; native IPC remains separately qualified.",
        ],
    }

    if args.source_only:
        evidence["status"] = "SOURCE_SUITE_READY" if evidence["source_contract"]["ok"] else "FAIL"
        evidence["executed_application"] = False
    else:
        pnpm = shutil.which("pnpm")
        if not args.skip_build and not pnpm:
            evidence["status"] = "BLOCKED"
            evidence["executed_application"] = False
            evidence["blocker"] = {"class": "DEPENDENCY_BLOCKER", "condition": "pinned pnpm is unavailable; full-app Svelte build cannot execute"}
        else:
            if not args.skip_build:
                result = run([pnpm, "--dir", "apps/ui", "build"], ROOT, env={"VITE_AETHERCORE_TEST_TRANSPORT": "1"})
                evidence["build"] = {"command": "VITE_AETHERCORE_TEST_TRANSPORT=1 pnpm --dir apps/ui build", "exit_code": result.returncode, "output_tail": result.stdout[-4000:]}
                if result.returncode:
                    evidence["status"] = "FAIL"; evidence["executed_application"] = False
                else:
                    evidence["build"]["ok"] = True
            if evidence.get("status") != "FAIL":
                dist = args.dist.resolve()
                if not (dist / "index.html").is_file():
                    evidence["status"] = "FAIL"; evidence["executed_application"] = False; evidence["harness_error"] = f"missing built app: {dist / 'index.html'}"
                else:
                    try:
                        checks, console_errors = browser_checks(dist, args.chromium)
                        evidence["checks"] = checks
                        evidence["console_errors"] = console_errors
                        failures = [c for c in checks if c.get("status") != "PASS"]
                        evidence["executed_application"] = True
                        evidence["status"] = "PASS" if not failures and not console_errors and evidence["source_contract"]["ok"] else "FAIL"
                    except Exception as exc:
                        evidence["status"] = "FAIL"; evidence["executed_application"] = False; evidence["harness_error"] = str(exc)

    evidence["summary"] = {
        "total": len(evidence["checks"]),
        "passed": sum(c.get("status") == "PASS" for c in evidence["checks"]),
        "failed": sum(c.get("status") != "PASS" for c in evidence["checks"]),
        "status": evidence["status"],
    }
    rendered = json.dumps(evidence, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True); args.json.write_text(rendered, encoding="utf-8")
    print(rendered, end="")
    return 0 if evidence["status"] in {"PASS", "SOURCE_SUITE_READY"} else (2 if evidence["status"] == "BLOCKED" else 1)


if __name__ == "__main__":
    raise SystemExit(main())
