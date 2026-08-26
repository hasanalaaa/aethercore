#!/usr/bin/env bash
# Applies PHASE_28_BINARY_SAFE_PATCH onto the sealed P27 tree. Usage: apply <target-root>
set -euo pipefail
TARGET="$1"; HERE="$(cd "$(dirname "$0")" && pwd)"
python3 - "$TARGET" "$HERE" <<'PYEOF'
import sys, pathlib, re
target = pathlib.Path(sys.argv[1]); here = pathlib.Path(sys.argv[2])
manifest = __import__('json').loads((here/'MANIFEST.json').read_text())
patch = (here/'changes.patch').read_text()
chunks = {}
current = None
buf = []
for line in patch.splitlines(keepends=True):
    if line.startswith('--- '):
        if current: chunks[current] = ''.join(buf)
        raw = line[4:].split('\t')[0].strip()
        current = raw.lstrip('/') if raw != '/dev/null' else None
        buf = [line] if raw == '/dev/null' else []
        continue
    if line.startswith('+++ '):
        if current is None:
            current = line[4:].split('\t')[0].strip().lstrip('/')
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
    if added:
        out_lines = []
        for line in chunk.splitlines(keepends=True):
            if line.startswith('--- ') or line.startswith('+++ '):
                continue
            if line.startswith(chr(92)):
                continue
            m = re.match(r'@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@', line)
            if m:
                continue
            if line.startswith('+'):
                out_lines.append(line[1:])
        dest.write_text(''.join(out_lines))
    else:
        src = (pathlib.Path('/tmp/p28_sealed_snapshot/AetherCore-Phase27-Master-Delivery')/f).read_text(errors='replace').splitlines(keepends=True)
        out_lines = []
        idx = 0
        hunk_active = False
        plus_buf = []
        for line in chunk.splitlines(keepends=True):
            if line.startswith('--- ') or line.startswith('+++ '):
                continue
            if line.startswith(chr(92)):
                continue
            m = re.match(r'@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@', line)
            if m:
                out_lines.extend(plus_buf); plus_buf = []
                old_start = int(m.group(1))
                old_count = int(m.group(2)) if m.group(2) is not None else 1
                carry_to = old_start if old_count == 0 else old_start - 1
                while idx < carry_to:
                    out_lines.append(src[idx]); idx += 1
                assert idx == carry_to, f'offset drift in {f} at {idx+1}'
                hunk_active = True
                continue
            assert hunk_active, f'hunk header missing for {f}'
            if line.startswith('-'):
                assert idx < len(src) and src[idx] == line[1:], f'mismatch in {f} at line {idx+1}'
                idx += 1
            elif line.startswith('+'):
                plus_buf.append(line[1:])
            else:
                raise AssertionError(f'unexpected context line in -U0 hunk for {f}')
        out_lines.extend(plus_buf)
        while idx < len(src):
            out_lines.append(src[idx]); idx += 1
        dest.write_text(''.join(out_lines))
    applied += 1
print(f'applied {applied} files')
PYEOF
