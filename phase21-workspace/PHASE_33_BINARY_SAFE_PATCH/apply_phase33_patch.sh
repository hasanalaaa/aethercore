#!/usr/bin/env bash
# Apply the Phase 33 -U0 text delta to one pristine sealed-Phase-32 extraction.
set -euo pipefail
if [[ "$#" -ne 1 ]]; then
  echo "usage: apply_phase33_patch.sh <sealed-phase32-root>" >&2
  exit 2
fi
TARGET="$(cd "$1" && pwd)"
HERE="$(cd "$(dirname "$0")" && pwd)"
if [[ "$TARGET" == "/" || -z "$TARGET" ]]; then
  echo "refusing unsafe reconstruction target" >&2
  exit 2
fi
patch --batch --forward -p1 -d "$TARGET" < "$HERE/changes.patch"
python3 "$HERE/verify_phase33.py" "$TARGET"
