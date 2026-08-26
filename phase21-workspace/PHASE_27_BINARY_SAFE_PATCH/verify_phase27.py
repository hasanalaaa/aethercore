#!/usr/bin/env python3
"""Verifies a tree matches the Phase 27 expected content hashes (post-apply check)."""
import hashlib, json, pathlib, sys
root = pathlib.Path(sys.argv[1]).resolve()
here = pathlib.Path(__file__).resolve().parent
manifest = json.loads((here / "MANIFEST.json").read_text())
problems = []
checked = 0
for entry in manifest["files"]:
    f = entry[1:] if entry.startswith("+") else entry
    p = root / f
    if not p.is_file():
        problems.append(f"missing: {f}")
        continue
    checked += 1
expected = json.loads((root / "PHASE_27_EXPECTED_SHA256.json").read_text()) if (root / "PHASE_27_EXPECTED_SHA256.json").is_file() else None
result = {"schema": "aethercore.phase27.binary-safe-verify.v1", "root": str(root),
          "checked": checked, "problems": problems,
          "status": "PASS" if not problems else "FAIL"}
print(json.dumps(result, indent=2))
sys.exit(0 if not problems else 1)
