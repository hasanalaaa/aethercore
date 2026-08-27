#!/usr/bin/env python3
"""Builds PHASE_30_BINARY_SAFE_PATCH (dual-mode verify + full-tree ledger) relative to
the sealed P29 archive."""
import hashlib
import json
import os
import re
import pathlib
import shutil
import subprocess
import sys

ROOT = pathlib.Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path(__file__).resolve().parents[1]
PATCH = ROOT / "PHASE_30_BINARY_SAFE_PATCH"
SEAL = pathlib.Path("/tmp/p30_seal_extract/x")
EXCLUDED_DIRS = {"target", "node_modules", ".git", "dist", "state", "support-staging"}


def sha256(p: pathlib.Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


if not SEAL.exists():
    # P32 hygiene: superseded master archives moved to ROOT.parent/_archive/.
    # Resolve the seal archive from its canonical location, falling back to
    # the archive directory (path recorded in docs/phase32/HYGIENE.md).
    candidates = [
        ROOT.parent / "AetherCore-Phase29-Master-Delivery.zip",
        ROOT.parent / "_archive" / "AetherCore-Phase29-Master-Delivery.zip",
    ]
    z = next((c for c in candidates if c.exists()), candidates[0])
    if SEAL.exists():
        shutil.rmtree(SEAL)
    SEAL.mkdir(parents=True)
    subprocess.run(["tar", "-xzf", str(z), "-C", str(SEAL), "--strip-components", "1"],
                   check=True, capture_output=True)

changed, added = [], []
for path in sorted(ROOT.rglob("*")):
    rel = str(path.relative_to(ROOT))
    if any(part in EXCLUDED_DIRS for part in path.parts):
        continue
    if rel.startswith("PHASE_30_BINARY_SAFE_PATCH"):
        continue
    if not path.is_file():
        continue
    sealed = SEAL / rel
    if not sealed.exists():
        added.append(rel)
    elif sealed.read_bytes() != path.read_bytes():
        changed.append(rel)

print(f"modified={len(changed)} added={len(added)}")

sha_map = {f: sha256(ROOT / f) for f in changed + added}
manifest = {
    "schema": "aethercore.phase30.binary-safe-patch.v1",
    "base": "sealed-P29-archive",
    "files": changed + [f"+{a}" for a in sorted(added)],
    "modifiedCount": len(changed),
    "addedCount": len(added),
    "removedCount": 0,
    "sha256": sha_map,
}

full = {}
for path in sorted(ROOT.rglob("*")):
    rel = str(path.relative_to(ROOT))
    if any(part in EXCLUDED_DIRS for part in path.parts):
        continue
    if rel == "PHASE_30_BINARY_SAFE_PATCH/PHASE_30_EXPECTED_FULL_SHA256.json":
        continue
    if not path.is_file():
        continue
    full[rel] = sha256(path)

diffs = []
for f in changed:
    r = subprocess.run(["diff", "-u0", str(SEAL / f), str(ROOT / f)],
                       capture_output=True, text=True)
    diffs.append(re.sub(r"\t[^\t\n]*$", "", r.stdout.replace(str(ROOT), ""), count=2, flags=re.M))
for a in sorted(added):
    lines = (ROOT / a).read_text(errors="replace").splitlines(keepends=True)
    body = "".join(f"+{l}" for l in lines)
    diffs.append(f"--- /dev/null\n+++ {a}\n@@ -0,0 +1,{len(lines)} @@\n{body}")

verify_py = '''#!/usr/bin/env python3
"""Phase 30 binary-safe verification — dual mode (patch hashes / full tree)."""
from __future__ import annotations
import hashlib, json, pathlib, sys

EXCLUDED_DIRS = {"target", "node_modules", ".git", "dist", "state", "support-staging"}
SELF = "PHASE_30_BINARY_SAFE_PATCH/PHASE_30_EXPECTED_FULL_SHA256.json"

def sha256(p): return hashlib.sha256(p.read_bytes()).hexdigest()

def patch_mode(root, here):
    manifest = json.loads((here / "MANIFEST.json").read_text())
    problems, checked = [], 0
    for entry in manifest.get("files", []):
        rel = entry[1:] if entry.startswith("+") else entry
        p = root / rel
        if not p.is_file():
            problems.append(f"missing: {rel}"); continue
        want = manifest.get("sha256", {}).get(rel)
        if want is None:
            problems.append(f"no recorded sha256 for: {rel}"); continue
        checked += 1
        if sha256(p) != want:
            problems.append(f"hash mismatch: {rel}")
    return {"schema": "aethercore.phase30.binary-safe-verify.v1", "mode": "patch",
            "root": str(root), "checked": checked, "problems": problems,
            "status": "PASS" if not problems else "FAIL"}

def full_tree_mode(root, here):
    entries = json.loads((here / "PHASE_30_EXPECTED_FULL_SHA256.json").read_text()).get("files", {})
    problems, checked, seen = [], 0, set()
    for path in sorted(root.rglob("*")):
        rel = str(path.relative_to(root))
        if rel == SELF:
            seen.add(rel); continue
        if any(part in EXCLUDED_DIRS for part in path.parts) or not path.is_file():
            continue
        seen.add(rel)
        want = entries.get(rel)
        if want is None:
            problems.append(f"unrecorded file: {rel}"); continue
        checked += 1
        if sha256(path) != want:
            problems.append(f"hash mismatch: {rel}")
    for rel in sorted(set(entries) - seen):
        problems.append(f"missing: {rel}")
    return {"schema": "aethercore.phase30.binary-safe-verify.v1", "mode": "full-tree",
            "root": str(root), "checked": checked, "expected_total": len(entries),
            "self_excluded": SELF, "problems": problems[:50],
            "problem_count": len(problems),
            "status": "PASS" if not problems else "FAIL"}

def main():
    root = pathlib.Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else pathlib.Path.cwd().resolve()
    here = pathlib.Path(__file__).resolve().parent
    result = full_tree_mode(root, here) if "--full-tree" in sys.argv else patch_mode(root, here)
    print(json.dumps(result, indent=2))
    return 0 if result["status"] == "PASS" else 1

if __name__ == "__main__":
    sys.exit(main())
'''

apply_sh = '''#!/usr/bin/env bash
# Applies PHASE_30_BINARY_SAFE_PATCH onto the sealed P29 tree. Usage: apply <root>
set -euo pipefail
TARGET="$1"; HERE="$(cd "$(dirname "$0")" && pwd)"
python3 - "$TARGET" "$HERE" <<'PYEOF'
import pathlib, re, sys, json
target = pathlib.Path(sys.argv[1]); here = pathlib.Path(sys.argv[2])
m = json.loads((here/'MANIFEST.json').read_text())
patch = (here/'changes.patch').read_text()
chunks={}; current=None; buf=[]
for line in patch.splitlines(keepends=True):
    if line.startswith('--- '):
        if current is not None: chunks[current]=''.join(buf)
        raw=line[4:].split('\\t')[0].strip()
        if raw=='/dev/null': current=None; buf=[]; continue
        current=raw.lstrip('/'); buf=[]; continue
    if line.startswith('+++ '): continue
    if current is not None: buf.append(line)
if current is not None: chunks[current]=''.join(buf)
applied=0
for entry in m['files']:
    added=entry.startswith('+')
    f=entry[1:] if added else entry
    dest=target/f; dest.parent.mkdir(parents=True, exist_ok=True)
    cands=[k for k in chunks if k==f or k.endswith('/'+f)]
    assert cands, f'no chunk for {f}'
    chunk=chunks[cands[0]]
    if added:
        out=[l[1:] for l in chunk.splitlines(keepends=True)
             if not l.startswith(('--- ','+++ ','\\\\')) and l.startswith('+')]
        dest.write_text(''.join(out))
    else:
        src=(pathlib.Path('/tmp/p30_seal_extract/x')/f).read_text(errors='replace').splitlines(keepends=True)
        out=[]; idx=0; plus=[]; active=False
        for line in chunk.splitlines(keepends=True):
            if line.startswith(('--- ','+++ ')) or line.startswith(chr(92)): continue
            mm=re.match(r'@@ -(\\d+)(?:,(\\d+))? \\+(\\d+)(?:,(\\d+))? @@', line)
            if mm:
                out.extend(plus); plus=[]
                old_start=int(mm.group(1)); old_n=int(mm.group(2) or 1)
                carry = old_start if old_n==0 else old_start-1
                while idx<carry: out.append(src[idx]); idx+=1
                assert idx==carry, f'drift {f}'
                active=True; continue
            assert active, f'no hunk header {f}'
            if line.startswith('-'):
                assert idx<len(src) and src[idx]==line[1:], f'mismatch {f}@{idx+1}'
                idx+=1
            elif line.startswith('+'): plus.append(line[1:])
            else: raise AssertionError(f'context line for {f}')
        out.extend(plus)
        while idx<len(src): out.append(src[idx]); idx+=1
        dest.write_text(''.join(out))
    applied+=1
print(f'applied {applied} files')
PYEOF
'''

if PATCH.exists():
    keep = PATCH / "verify_phase30.py"
    kept = keep.read_text() if keep.exists() else None
    shutil.rmtree(PATCH)
    PATCH.mkdir(parents=True)
    if kept:
        (PATCH / "verify_phase30.py").write_text(kept)
else:
    PATCH.mkdir(parents=True)

VERIFY_PHASE30_SRC = '#!/usr/bin/env python3\n"""Phase 30 binary-safe verification — dual mode, full standard continued.\n\nModes:\n  default (patch)  : hash-verifies every modified/added file in MANIFEST.json against\n                     the sha256 recorded there.\n  --full-tree      : hash-verifies the ENTIRE delivered tree against\n                     PHASE_30_EXPECTED_FULL_SHA256.json. The ledger cannot contain its\n                     own digest; that file is self-excluded and bound by the master\n                     archive SHA-256.\n"""\nfrom __future__ import annotations\n\nimport hashlib\nimport json\nimport pathlib\nimport sys\n\nEXCLUDED_DIRS = {"target", "node_modules", ".git", "dist", "state", "support-staging"}\nSELF = "PHASE_30_BINARY_SAFE_PATCH/PHASE_30_EXPECTED_FULL_SHA256.json"\n\n\ndef sha256(path: pathlib.Path) -> str:\n    return hashlib.sha256(path.read_bytes()).hexdigest()\n\n\ndef patch_mode(root: pathlib.Path, here: pathlib.Path) -> dict:\n    manifest = json.loads((here / "MANIFEST.json").read_text())\n    problems = []\n    checked = 0\n    for entry in manifest.get("files", []):\n        rel = entry[1:] if entry.startswith("+") else entry\n        p = root / rel\n        if not p.is_file():\n            problems.append(f"missing: {rel}")\n            continue\n        want = manifest.get("sha256", {}).get(rel)\n        if want is None:\n            problems.append(f"no recorded sha256 for: {rel}")\n            continue\n        checked += 1\n        if sha256(p) != want:\n            problems.append(f"hash mismatch: {rel}")\n    return {"schema": "aethercore.phase30.binary-safe-verify.v1", "mode": "patch",\n            "root": str(root), "checked": checked, "problems": problems,\n            "status": "PASS" if not problems else "FAIL"}\n\n\ndef full_tree_mode(root: pathlib.Path, here: pathlib.Path) -> dict:\n    ledger = json.loads((here / "PHASE_30_EXPECTED_FULL_SHA256.json").read_text())\n    entries = ledger.get("files", {})\n    problems = []\n    checked = 0\n    seen = set()\n    for path in sorted(root.rglob("*")):\n        rel = str(path.relative_to(root))\n        if rel == SELF:\n            seen.add(rel)\n            continue\n        if any(part in EXCLUDED_DIRS for part in path.parts):\n            continue\n        if not path.is_file():\n            continue\n        seen.add(rel)\n        want = entries.get(rel)\n        if want is None:\n            problems.append(f"unrecorded file: {rel}")\n            continue\n        checked += 1\n        if sha256(path) != want:\n            problems.append(f"hash mismatch: {rel}")\n    for rel in sorted(set(entries) - seen):\n        problems.append(f"missing: {rel}")\n    return {"schema": "aethercore.phase30.binary-safe-verify.v1", "mode": "full-tree",\n            "root": str(root), "checked": checked,\n            "expected_total": len(entries), "self_excluded": SELF,\n            "problems": problems[:50], "problem_count": len(problems),\n            "status": "PASS" if not problems else "FAIL"}\n\n\ndef main() -> int:\n    root = pathlib.Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else pathlib.Path.cwd().resolve()\n    here = pathlib.Path(__file__).resolve().parent\n    full = "--full-tree" in sys.argv\n    result = full_tree_mode(root, here) if full else patch_mode(root, here)\n    print(json.dumps(result, indent=2))\n    return 0 if result["status"] == "PASS" else 1\n\n\nif __name__ == "__main__":\n    sys.exit(main())\n'

# Always (re)write the dual-mode verifier.
verify_py_path = PATCH / "verify_phase30.py"
if not verify_py_path.exists():
    verify_py_path.write_text(VERIFY_PHASE30_SRC)
    os.chmod(verify_py_path, 0o755)

(PATCH / "MANIFEST.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
(PATCH / "changes.patch").write_text("".join(diffs))
(PATCH / "PHASE_30_EXPECTED_FULL_SHA256.json").write_text(
    json.dumps({"schema": "aethercore.phase30.full-tree-sha256.v1",
                "base": manifest["base"], "files": full},
               indent=2, sort_keys=True) + "\n")
(PATCH / "apply_phase30_patch.sh").write_text(apply_sh)
os.chmod(PATCH / "apply_phase30_patch.sh", 0o755)
print("PHASE_30_BINARY_SAFE_PATCH written")
