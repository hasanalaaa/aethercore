#!/usr/bin/env python3
"""P76 DBT-P76-009: the build scripts need PowerShell 7 and nothing made sure of it.

Found on the owner's Windows 11 install at be965a2: `package.json` ran every script with
Windows PowerShell 5.1 (`powershell`), bootstrap.ps1 never checked for `pwsh`, and the build
failed inside [IO.Path]::GetRelativePath, which 5.1's .NET Framework does not have.

Asserts: bootstrap.ps1 (the one entry a fresh machine can start under 5.1) installs pwsh via
winget and re-runs itself there before anything else; every other package.json script runs
under pwsh; every script that calls a PowerShell-7-only API declares `#Requires -Version 7`.

Run: `python3 scripts/test_bootstrap_pwsh.py`
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
failures: list[str] = []

scripts = json.loads((ROOT / "package.json").read_text(encoding="utf-8"))["scripts"]
for name, command in scripts.items():
    if ".ps1" not in command:
        continue
    shell = command.split()[0]
    expected = "powershell" if name == "bootstrap" else "pwsh"
    if shell != expected:
        failures.append(f"package.json script {name!r} runs under {shell!r}, expected {expected!r}")

bootstrap = (ROOT / "scripts/bootstrap.ps1").read_text(encoding="utf-8")
gate = bootstrap.find("$PSVersionTable.PSVersion.Major -lt 7")
first_work = bootstrap.find("freeze-dependencies.ps1")
if gate < 0 or not (0 <= gate < first_work):
    failures.append("bootstrap.ps1 does not check for PowerShell 7 before it starts work")
for token in ("winget install --id Microsoft.PowerShell", "& pwsh @forward", "exit $LASTEXITCODE"):
    if token not in bootstrap[gate:first_work if first_work > gate else None]:
        failures.append(f"bootstrap.ps1's PowerShell 7 block lacks {token!r}")

for path in sorted((ROOT / "scripts").glob("*.ps1")):
    text = path.read_text(encoding="utf-8", errors="ignore")
    if "GetRelativePath" in text and not re.search(r"^#Requires -Version 7", text, re.M):
        failures.append(f"{path.name} calls [IO.Path]::GetRelativePath without #Requires -Version 7")

for failure in failures:
    print("FAIL  " + failure)
print(f"{len(failures)} failed" if failures else "bootstrap and package.json run the build under PowerShell 7")
sys.exit(1 if failures else 0)
