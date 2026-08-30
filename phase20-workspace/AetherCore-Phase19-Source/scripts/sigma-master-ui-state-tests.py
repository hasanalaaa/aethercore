#!/usr/bin/env python3
"""Execute platform-neutral state tests that defend full-app UI concurrency invariants.

This is intentionally small and executable without the Svelte dependency graph. It compiles the
same TypeScript ActivityCounter imported by production shell-state.ts and checks overlapping,
out-of-order and duplicate completion behavior. It is evidence for the source-level state helper,
not a substitute for the full-application browser suite.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "apps/ui/src/app/activity-counter.ts"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", type=Path)
    args = parser.parse_args()
    evidence: dict[str, Any] = {
        "schema": "aethercore.sigma-master-ui-state.v1",
        "scope": "production ActivityCounter overlap semantics",
        "source": SOURCE.relative_to(ROOT).as_posix(),
        "checks": [],
    }
    tsc = shutil.which("tsc")
    node = shutil.which("node")
    if not tsc or not node:
        evidence.update({
            "status": "BLOCKED",
            "blocker": {"class": "TEST_BLOCKER", "condition": "global tsc/node unavailable for platform-neutral TypeScript state test"},
        })
        code = 2
    else:
        with tempfile.TemporaryDirectory(prefix="aethercore-ui-state-") as tmp_raw:
            tmp = Path(tmp_raw)
            compile_cmd = [tsc, str(SOURCE), "--target", "ES2022", "--module", "commonjs", "--strict", "--outDir", str(tmp)]
            built = subprocess.run(compile_cmd, cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
            evidence["toolchain"] = {"tsc": tsc, "node": node}
            evidence["compile"] = {"command": compile_cmd, "exit_code": built.returncode, "output": built.stdout}
            if built.returncode:
                evidence["status"] = "FAIL"
                code = 1
            else:
                module = tmp / "activity-counter.js"
                script = r"""
const { ActivityCounter } = require(process.argv[1]);
const checks = [];
function test(name, fn) {
  try { fn(); checks.push({name, status:'PASS'}); }
  catch (error) { checks.push({name, status:'FAIL', error:String(error)}); }
}
function expect(value, message) { if (!value) throw new Error(message); }
test('overlap-first-completion-does-not-clear-busy', () => {
  const c = new ActivityCounter();
  const a = c.enter(); const b = c.enter();
  expect(c.active && c.size === 2, `expected 2 active, got ${c.size}`);
  a();
  expect(c.active && c.size === 1, `first completion cleared aggregate busy: ${c.size}`);
  b();
  expect(!c.active && c.size === 0, `counter did not settle: ${c.size}`);
});
test('out-of-order-completion-is-safe', () => {
  const c = new ActivityCounter();
  const a = c.enter(); const b = c.enter(); const d = c.enter();
  b(); d();
  expect(c.active && c.size === 1, `out-of-order completion lost activity: ${c.size}`);
  a(); expect(!c.active && c.size === 0, `counter did not settle: ${c.size}`);
});
test('release-is-idempotent', () => {
  const c = new ActivityCounter();
  const release = c.enter();
  release(); release(); release();
  expect(!c.active && c.size === 0, `duplicate release underflowed/corrupted count: ${c.size}`);
});
test('new-activity-after-settle-remains-valid', () => {
  const c = new ActivityCounter();
  c.enter()();
  const release = c.enter();
  expect(c.active && c.size === 1, 'counter did not reactivate');
  release(); expect(!c.active && c.size === 0, 'counter did not settle again');
});
process.stdout.write(JSON.stringify(checks));
process.exit(checks.some(c => c.status !== 'PASS') ? 1 : 0);
"""
                proc = subprocess.run([node, "-e", script, str(module)], cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
                try:
                    checks = json.loads(proc.stdout)
                except json.JSONDecodeError:
                    checks = [{"name": "node-harness", "status": "FAIL", "error": proc.stdout}]
                evidence["checks"] = checks
                evidence["status"] = "PASS" if proc.returncode == 0 and all(c.get("status") == "PASS" for c in checks) else "FAIL"
                code = 0 if evidence["status"] == "PASS" else 1
    evidence["summary"] = {
        "total": len(evidence["checks"]),
        "passed": sum(c.get("status") == "PASS" for c in evidence["checks"]),
        "failed": sum(c.get("status") == "FAIL" for c in evidence["checks"]),
        "status": evidence["status"],
    }
    rendered = json.dumps(evidence, indent=2, sort_keys=True) + "\n"
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(rendered, encoding="utf-8")
    print(rendered, end="")
    return code


if __name__ == "__main__":
    raise SystemExit(main())
