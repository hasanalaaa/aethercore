#!/usr/bin/env bash
# Apply the Phase 35 text and binary delta to one pristine sealed-Phase-34 extraction.
set -euo pipefail
if [[ "$#" -ne 1 ]]; then echo 'usage: apply_phase35_patch.sh <sealed-phase34-root>' >&2; exit 2; fi
TARGET="$(cd "$1" && pwd)"
HERE="$(cd "$(dirname "$0")" && pwd)"
if [[ "$TARGET" == "/" || -z "$TARGET" ]]; then echo 'refusing unsafe reconstruction target' >&2; exit 2; fi
patch --batch --forward -p1 -d "$TARGET" < "$HERE/changes.patch"
python3 - "$HERE/MANIFEST.json" "$HERE/BINARY_ARTIFACTS" "$TARGET" <<'PY'
import json, pathlib, shutil, sys
manifest = json.loads(pathlib.Path(sys.argv[1]).read_text())
source = pathlib.Path(sys.argv[2]); target = pathlib.Path(sys.argv[3])
for rel in manifest.get("binaryFiles", []):
    src = source / rel; dst = target / rel
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src, dst)
PY
python3 "$HERE/verify_phase35.py" "$TARGET"
