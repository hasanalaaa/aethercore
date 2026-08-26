#!/usr/bin/env python3
"""Builds PHASE_29_BINARY_SAFE_PATCH: MANIFEST (hash per file) + changes.patch (-U0)
+ apply script + PHASE_29_EXPECTED_FULL_SHA256.json, relative to the sealed P28 tree
(snapshot extracted from AetherCore-Phase28-Master-Delivery.zip)."""
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys

ROOT = pathlib.Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path(__file__).resolve().parents[1]
PATCH = ROOT / "PHASE_29_BINARY_SAFE_PATCH"
SEAL = pathlib.Path("/tmp/p29_seal_extract/x")
EXCLUDED_DIRS = {"target", "node_modules", ".git", "dist", "state", "support-staging"}

def sha256(p: pathlib.Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()
SKIP_PREFIXES = ("C:\\ProgramData", "PHASE_29_BINARY_SAFE_PATCH")

if not SEAL.exists():
    print("extracting sealed P28 archive…")
    z = ROOT.parent / "AetherCore-Phase28-Master-Delivery.zip"
    if SEAL.exists():
        shutil.rmtree(SEAL)
    SEAL.mkdir(parents=True)
    subprocess.run(["tar", "-xzf", str(z), "-C", str(SEAL)],
                   check=True, capture_output=True)

changed: list[str] = []
added: list[str] = []
for path in sorted(ROOT.rglob("*")):
    rel = str(path.relative_to(ROOT))
    if any(part in EXCLUDED_DIRS for part in path.parts):
        continue
    if any(rel.startswith(p) for p in SKIP_PREFIXES):
        continue
    if not path.is_file():
        continue
    sealed = SEAL / rel
    if not sealed.exists():
        added.append(rel)
    elif sealed.read_bytes() != path.read_bytes():
        changed.append(rel)

print(f"modified={len(changed)} added={len(added)}")

sha_map: dict[str, str] = {}
for f in changed + added:
    sha_map[f] = hashlib.sha256((ROOT / f).read_bytes()).hexdigest()

manifest = {
    "schema": "aethercore.phase29.binary-safe-patch.v1",
    "base": "sealed-P27→P28-snapshot",
    "files": changed + [f"+{a}" for a in sorted(added)],
    "modifiedCount": len(changed),
    "addedCount": len(added),
    "removedCount": 0,
    "sha256": sha_map,
}

# full-tree ledger (self-excluded file handled by the verifier)
full: dict[str, str] = {}
for path in sorted(ROOT.rglob("*")):
    rel = str(path.relative_to(ROOT))
    if any(part in EXCLUDED_DIRS for part in path.parts):
        continue
    if rel == "PHASE_29_BINARY_SAFE_PATCH/PHASE_29_EXPECTED_FULL_SHA256.json":
        continue
    if not path.is_file():
        continue
    full[rel] = sha256(path)

diffs = []
for f in changed:
    r = subprocess.run(["diff", "-u0", str(SEAL / f), str(ROOT / f)],
                       capture_output=True, text=True)
    diffs.append(r.stdout.replace(str(ROOT), ""))
for a in sorted(added):
    lines = (ROOT / a).read_text(errors="replace").splitlines(keepends=True)
    body = "".join(f"+{l}" for l in lines)
    diffs.append(f"--- /dev/null\n+++ {a}\n@@ -0,0 +1,{len(lines)} @@\n{body}")

if PATCH.exists():
    # keep verify_phase29.py (hand-written above) — regenerate the rest
    keep = PATCH / "verify_phase29.py"
    kept = keep.read_text() if keep.exists() else None
    shutil.rmtree(PATCH)
    PATCH.mkdir(parents=True)
    if kept:
        (PATCH / "verify_phase29.py").write_text(kept)

(PATCH / "MANIFEST.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
(PATCH_DIR := PATCH).joinpath("changes.patch").write_text("".join(diffs))
(PATCH / "PHASE_29_EXPECTED_FULL_SHA256.json").write_text(
    json.dumps({"schema": "aethercore.phase29.full-tree-sha256.v1",
                "base": manifest["base"], "files": full},
               indent=2, sort_keys=True) + "\n")

apply_sh = '''#!/usr/bin/env bash
# Applies PHASE_29_BINARY_SAFE_PATCH onto the sealed P28 tree. Usage: apply <target-root>
set -euo pipefail
TARGET="$1"; HERE="$(cd "$(dirname "$0")" && pwd)"
python3 - "$TARGET" "$HERE" <<'PYEOF'
import pathlib, re, sys
target = pathlib.Path(sys.argv[1]); here = pathlib.Path(sys.argv[2])
import json
m = json.loads((here/'MANIFEST.json').read_text())
patch = (here/'changes.patch').read_text()
chunks = {}
current = None; buf=[]
for line in patch.splitlines(keepends=True):
    if line.startswith('--- '):
        if current is not None: chunks[current]=''.join(buf)
        raw = line[4:].split('\\t')[0].strip()
        if raw == '/dev/null':
            current=None; buf=[]; continue
        current = raw.lstrip('/'); buf=[]
        continue
    if line.startswith('+++ '):
        continue
    if current is not None:
        buf.append(line)
if current is not None: chunks[current]=''.join(buf)

applied=0
for entry in m['files']:
    added = entry.startswith('+')
    f = entry[1:] if added else entry
    dest = target/f
    dest.parent.mkdir(parents=True, exist_ok=True)
    cands=[k for k in chunks if k==f or k.endswith('/'+f)]
    assert cands, f'no diff chunk for {f}'
    chunk=chunks[cands[0]]
    if added:
        out=[]
        for line in chunk.splitlines(keepends=True):
            if line.startswith(('--- ','+++ ')) or line.startswith(chr(92)): continue
            if line.startswith('+'): out.append(line[1:])
        dest.write_text(''.join(out))
    else:
        src=(pathlib.Path('/tmp/p29_seal_extract/x')/f).read_text(errors='replace').splitlines(keepends=True)
        out=[]; idx=0; plus=[]; active=False
        for line in chunk.splitlines(keepends=True):
            if line.startswith(('--- ','+++ ')) or line.startswith(chr(92)): continue
            mm=re.match(r'@@ -(\\d+)(?:,(\\d+))? \\+(\\d+)(?:,(\\d+))? @@', line)
            if mm:
                out.extend(plus); plus=[]
                old_start=int(mm.group(1)); old_n=int(mm.group(2) or 1)
                carry = old_start if old_n==0 else old_start-1
                while idx<carry: out.append(src[idx]); idx+=1
                assert idx==carry, f'drift in {f}'
                active=True; continue
            assert active, f'hunk header missing for {f}'
            if line.startswith('-'):
                assert idx<len(src) and src[idx]==line[1:], f'mismatch {f}@{idx+1}'
                idx+=1
            elif line.startswith('+'):
                plus.append(line[1:])
            else:
                raise AssertionError(f'unexpected context line for {f}')
        out.extend(plus)
        while idx<len(src): out.append(src[idx]); idx+=1
        dest.write_text(''.join(out))
    applied+=1
print(f'applied {applied} files')
PYEOF
'''
(PATCH / "apply_phase29_patch.sh").write_text(apply_sh)
os.chmod(PATCH / "apply_phase29_patch.sh", 0o755)
print("PHASE_29_BINARY_SAFE_PATCH written")
