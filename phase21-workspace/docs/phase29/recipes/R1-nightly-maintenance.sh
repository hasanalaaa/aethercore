#!/usr/bin/env bash
# =============================================================================
# R1 — nightly-maintenance.sh (Phase 29 recipe, copy-paste grade)
# detect → doctor → telemetry-once → export journal → verify
#
# Exit codes follow the P28 registry:
#   0 ok · 1 usage · 2 daemon unreachable · 3 refused by policy/consent
#   4 timeout · 5 command-specific failure
#
# Required env: AETHERCTL (path to the aetherctl binary), optional AETHER_DATA_DIR.
# =============================================================================
set -euo pipefail

AETHERCTL="${AETHERCTL:-./target/debug/aetherctl}"
DATA_DIR="${AETHER_DATA_DIR:-$HOME/.local/share/aethercore-nightly}"
SOCKET_ARGS=()

if [ -n "${AETHERCTL_SOCKET_DIR:-}" ]; then
  SOCKET_ARGS=(--socket-dir "$AETHERCTL_SOCKET_DIR")
fi
STAMP="$(date +%Y%m%d-%H%M%S)"
OUT_DIR="$DATA_DIR/exports"
mkdir -p "$OUT_DIR"

echo "[R1] 1/5 detect"
"$AETHERCTL" "${SOCKET_ARGS[@]}" service detect >/dev/null

echo "[R1] 2/5 doctor (informational — tolerated when diagnostic state is empty)"
if ! "$AETHERCTL" "${SOCKET_ARGS[@]}" --output json doctor >/dev/null; then
  echo "[R1]    note: doctor reported no diagnostic state yet (fresh daemon) — continuing"
fi

echo "[R1] 3/5 telemetry-once"
"$AETHERCTL" "${SOCKET_ARGS[@]}" telemetry-once --interval-ms 250 >/dev/null

echo "[R1] 4/5 export journal"
"$AETHERCTL" "${SOCKET_ARGS[@]}" export journal --out "$OUT_DIR/journal-$STAMP.json"

echo "[R1] 5/5 verify exported chain"
"$AETHERCTL" "${SOCKET_ARGS[@]}" --output json export verify "$OUT_DIR/journal-$STAMP.json" | tee "$OUT_DIR/verify-$STAMP.log" >/dev/null

echo "[R1] OK — verified export at $OUT_DIR/journal-$STAMP.json"
