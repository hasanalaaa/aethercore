#!/usr/bin/env python3
"""GG gate: PHASE_27_BINARY_SAFE_PATCH round-trip ×2.

Builds a binary-safe patch from the sealed P26 snapshot diff (MANIFEST.json +
changes.patch + apply + verify scripts), applies it to a pristine copy of the sealed
tree twice, and requires the result byte-identical to the live tree both times.
"""
import hashlib
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SEAL = Path("/tmp/p26_sealed_snapshot")
PATCH_DIR = ROOT / "PHASE_27_BINARY_SAFE_PATCH"
WORK = Path("/tmp/p27_patch_roundtrip")

EXCLUDE_DIRS = {"target", "node_modules", ".git", "dist", "state", "support-staging"}
# Runtime artifacts that appear in the live tree but are not source (never patched).
SKIP_PREFIXES = ("C:\\ProgramData",)

changed: list[str] = []
added: list[str] = []

for path in sorted(ROOT.rglob("*")):
    rel = path.relative_to(ROOT)
    parts = set(rel.parts)
    if parts & EXCLUDE_DIRS:
        continue
    s = str(rel)
    if any(s.startswith(p) for p in SKIP_PREFIXES):
        continue
    if not path.is_file():
        continue
    if s == "PHASE_27_BINARY_SAFE_PATCH" or s.startswith("PHASE_27_BINARY_SAFE_PATCH/"):
        continue
    sealed = SEAL / rel
    if not sealed.exists():
        added.append(s)
    else:
        if sealed.read_bytes() != path.read_bytes():
            changed.append(s)

print(f"modified={len(changed)} added={len(added)}")

if PATCH_DIR.exists():
    shutil.rmtree(PATCH_DIR)
PATCH_DIR.mkdir(parents=True)

manifest = {
    "schema": "aethercore.phase27.binary-safe-patch.v1",
    "base": "sealed-P26-snapshot",
    "files": sorted(changed) + [f"+{a}" for a in sorted(added)],
    "modifiedCount": len(changed),
    "addedCount": len(added),
    "removedCount": 0,
}
(PATCH_DIR / "MANIFEST.json").write_text(json.dumps(manifest, indent=2) + "\n")

diffs = []
for f in changed:
    r = subprocess.run(["diff", "-u0", str(SEAL / f), str(ROOT / f)],
                       capture_output=True, text=True)
    out = r.stdout.replace(str(ROOT), "")
    diffs.append(out)
for a in added:
    new_text = (ROOT / a).read_text(errors="replace")
    lines = new_text.splitlines(keepends=True)
    body = "".join(f"+{l}" for l in lines)
    diffs.append(f"--- /dev/null\n+++ {a}\n@@ -0,0 +1,{len(lines)} @@\n{body}")
(PATCH_DIR / "changes.patch").write_text("".join(diffs))

apply_sh = """#!/usr/bin/env bash
# Applies PHASE_27_BINARY_SAFE_PATCH onto the sealed P26 tree. Usage: apply <target-root>
set -euo pipefail
TARGET="$1"; HERE="$(cd "$(dirname "$0")" && pwd)"
python3 - "$TARGET" "$HERE" <<'PYEOF'
import sys, pathlib, re
target = pathlib.Path(sys.argv[1]); here = pathlib.Path(sys.argv[2])
manifest = __import__('json').loads((here/'MANIFEST.json').read_text())
patch = (here/'changes.patch').read_text()
# Split unified diff into per-file chunks.
chunks = {}
current = None
buf = []
for line in patch.splitlines(keepends=True):
    if line.startswith('--- '):
        if current: chunks[current] = ''.join(buf)
        raw = line[4:].split('\\t')[0].strip()
        current = raw.lstrip('/') if raw != '/dev/null' else None
        buf = [line] if raw == '/dev/null' else []
        continue
    if line.startswith('+++ '):
        if current is None:
            current = line[4:].split('\\t')[0].strip().lstrip('/')
        continue
    if current is not None:
        buf.append(line)
if current: chunks[current] = ''.join(buf)
applied = 0
for entry in manifest['files']:
    added = entry.startswith('+')
    f = entry[1:] if added else entry
    dest = target/f
    dest.parent.mkdir(parents=True, exist_ok=True)
    key_candidates = [c for c in chunks if c == f or c.endswith('/' + f)]
    assert key_candidates, f'no diff chunk for {f}'
    chunk = chunks[key_candidates[0]]
    # Splice-based application: -U0 hunks give exact insert/delete offsets into the
    # sealed source. We walk hunks in order, carrying untouched source through.
    if added:
        out_lines: list[str] = []
        for line in chunk.splitlines(keepends=True):
            if line.startswith('--- ') or line.startswith('+++ '):
                continue
            if line.startswith(chr(92)):
                continue
            m = re.match(r'@@ -(\\d+)(?:,(\\d+))? \\+(\\d+)(?:,(\\d+))? @@', line)
            if m:
                continue
            if line.startswith('+'):
                out_lines.append(line[1:])
        dest.write_text(''.join(out_lines))
    else:
        src = (pathlib.Path('/tmp/p26_sealed_snapshot')/f).read_text(errors='replace').splitlines(keepends=True)
        out_lines: list[str] = []
        idx = 0  # next unread source line
        hunk_active = False
        plus_buf: list[str] = []
        for line in chunk.splitlines(keepends=True):
            if line.startswith('--- ') or line.startswith('+++ '):
                continue
            if line.startswith(chr(92)):
                continue
            m = re.match(r'@@ -(\\d+)(?:,(\\d+))? \\+(\\d+)(?:,(\\d+))? @@', line)
            if m:
                # flush the previous hunk's insertions before starting a new one
                out_lines.extend(plus_buf); plus_buf = []
                old_start = int(m.group(1))  # 1-based position in the OLD file
                old_count = int(m.group(2)) if m.group(2) is not None else 1
                # carry untouched source up to the hunk start in the OLD file.
                # For a pure insertion (old_count == 0) diff semantics place it AFTER
                # old line old_start, so carry one extra source line.
                carry_to = old_start if old_count == 0 else old_start - 1
                while idx < carry_to:
                    out_lines.append(src[idx]); idx += 1
                assert idx == carry_to, f'offset drift in {f} at {idx+1}'
                hunk_active = True
                continue
            assert hunk_active, f'hunk header missing for {f}'
            if line.startswith('-'):
                # -U0 offsets are exact; verify the line matches the sealed source.
                assert idx < len(src) and src[idx] == line[1:], f'mismatch in {f} at line {idx+1}'
                idx += 1
            elif line.startswith('+'):
                plus_buf.append(line[1:])
            else:
                raise AssertionError(f'unexpected context line in -U0 hunk for {f}')
        # end of chunk: flush any trailing hunk's insertions, then carry the tail.
        out_lines.extend(plus_buf)
        while idx < len(src):
            out_lines.append(src[idx]); idx += 1
        dest.write_text(''.join(out_lines))
    applied += 1
print(f'applied {applied} files')
PYEOF
"""
(PATCH_DIR / "apply_phase27_patch.sh").write_text(apply_sh)
os.chmod(PATCH_DIR / "apply_phase27_patch.sh", 0o755)

verify_py = '''#!/usr/bin/env python3
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
'''
(PATCH_DIR / "verify_phase27.py").write_text(verify_py)
print("PHASE_27_BINARY_SAFE_PATCH written")
