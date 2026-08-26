#!/usr/bin/env bash
# Applies PHASE_31_BINARY_SAFE_PATCH onto the sealed P30 tree. Usage: apply <root>
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
        raw=line[4:].split('\t')[0].strip()
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
             if not l.startswith(('--- ','+++ ','\\')) and l.startswith('+')]
        dest.write_text(''.join(out))
    else:
        src=(pathlib.Path('/tmp/p30_seal_extract/x')/f).read_text(errors='replace').splitlines(keepends=True)
        out=[]; idx=0; plus=[]; active=False
        for line in chunk.splitlines(keepends=True):
            if line.startswith(('--- ','+++ ')) or line.startswith(chr(92)): continue
            mm=re.match(r'@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@', line)
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
