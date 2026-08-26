#!/usr/bin/env python3
"""Probe: how does router.rs compile on unix despite cfg(windows)-only require_broker?"""
import re

text = open("services/maintenance-service/src/router.rs", encoding="utf-8").read()
lines = text.splitlines()

# Show exact bytes around every 'require_broker' occurrence with line numbers.
for m in re.finditer(r"require_broker", text):
    start_line = text.count("\n", 0, m.start()) + 1
    lo = max(0, start_line - 3)
    hi = min(len(lines), start_line + 2)
    print(f"--- context for line {start_line} ---")
    for n in range(lo, hi):
        print(f"{n+1:5d}| {lines[n]}")

# Any cfg attributes immediately preceding match-arm patterns mentioning broker-guarded payloads?
for name in ("GetConsentIntent", "ApproveConsentIntent", "GetUpdateInstallIntent"):
    idx = text.find(f"Payload::{name}")
    seg = text[max(0, idx - 200):idx]
    print(f"\n{name}: preceding 200 chars:\n{seg[-200:]!r}")
