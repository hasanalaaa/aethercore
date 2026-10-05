#!/usr/bin/env python3
"""DBT-P55-007: `signatureValidationMode=require` was not enforced by `dotnet tool restore` on a developer
machine, so the control the project believed it had did nothing there.

Measured on the Windows PC (SDK 8.0.425) with the cached wix 6.0.2:
  `dotnet nuget verify <nupkg>`                                  exit 0
  `... --certificate-fingerprint <the declared author, FireGiant>`  exit 0
  `... --certificate-fingerprint <the declared repository, nuget.org>` exit 1, NU3034 (it matches the
      author signature only)
  `... --certificate-fingerprint 000...0`                         exit 1, NU3034
  `... --certificate-fingerprint 000...0 --certificate-fingerprint <author>`  exit 0 (any of them)

So the check that does not depend on how the restoring SDK behaves is `dotnet nuget verify` on the
restored package with the *author* fingerprints that nuget.config already declares. This test asserts:

  1. Every script that runs `dotnet tool restore` calls scripts/verify-tool-signatures.ps1 before it runs
     a tool, and no other script or workflow restores without it.
  2. The script passes for the real config, and fails closed for a corrupted fingerprint, for a config
     with no author signer, and for a tool that was never restored.

Part 2 needs `dotnet` and `pwsh` (the Windows job has both). Without them it says so and exits 2.

Run: `python3 scripts/test_tool_signatures.py`
"""
from __future__ import annotations

import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPO = ROOT.parent
VERIFY = ROOT / "scripts" / "verify-tool-signatures.ps1"
RESTORING = ("build-installer.ps1", "sign-burn-bundle.ps1", "bootstrap.ps1")
failures: list[str] = []

# 1. every restore site verifies before it runs a tool, and no site is missing from the list above
for name in RESTORING:
    text = (ROOT / "scripts" / name).read_text(encoding="utf-8")
    restore = text.find("dotnet tool restore")
    if restore < 0:
        failures.append(f"{name} no longer restores the pinned tools; update RESTORING")
        continue
    verify = text.find("verify-tool-signatures.ps1", restore)
    run = text.find("dotnet tool run", restore)
    if verify < 0:
        failures.append(f"{name} restores the pinned tools and never verifies their signatures")
    elif 0 <= run < verify:
        failures.append(f"{name} runs a tool before verifying its signature")

candidates = [*(ROOT / "scripts").glob("*.ps1"), *(REPO / ".github" / "workflows").glob("*.yml")]
for path in sorted(candidates):
    if path.name in RESTORING or path == VERIFY:
        continue
    if "dotnet tool restore" in path.read_text(encoding="utf-8", errors="ignore"):
        failures.append(f"{path.relative_to(REPO)} restores tools without verify-tool-signatures.ps1")

# 2. the script itself, for real
pwsh, dotnet = shutil.which("pwsh"), shutil.which("dotnet")
behavioural = bool(pwsh and dotnet)
if behavioural:
    def run(*argv: str) -> tuple[int, str]:
        done = subprocess.run(list(argv), cwd=ROOT, capture_output=True, text=True, timeout=300)
        return done.returncode, done.stdout + done.stderr

    def verify(*extra: str) -> tuple[int, str]:
        return run(pwsh, "-NoProfile", "-File", str(VERIFY), *extra)

    code, out = run(dotnet, "tool", "restore")
    if code != 0:
        failures.append(f"precondition: dotnet tool restore failed ({code}): {out[-300:]}")
    elif not VERIFY.exists():
        failures.append("scripts/verify-tool-signatures.ps1 does not exist")
    else:
        code, out = verify()
        if code != 0:
            failures.append(f"the real config must verify the restored tools, got exit {code}: {out[-400:]}")

        with tempfile.TemporaryDirectory() as tmp:
            real = (ROOT / "nuget.config").read_text(encoding="utf-8")

            corrupted = Path(tmp) / "corrupted.config"
            corrupted.write_text(
                re.sub(r'fingerprint="[0-9A-Fa-f]{64}"', 'fingerprint="' + "0" * 64 + '"', real), encoding="utf-8")
            code, out = verify("-NuGetConfigPath", str(corrupted))
            if code == 0:
                failures.append("corrupted trusted-signer fingerprints still verified: the control is not enforced")
            elif "NU3034" not in out:
                failures.append(f"corrupted fingerprints failed for the wrong reason: {out[-400:]}")

            unsigned = Path(tmp) / "no-author.config"
            unsigned.write_text(re.sub(r"<author .*?</author>", "", real, flags=re.S), encoding="utf-8")
            code, out = verify("-NuGetConfigPath", str(unsigned))
            if code == 0:
                failures.append("a config naming no author signer verified: it must fail closed")

            manifest = json.loads((ROOT / ".config" / "dotnet-tools.json").read_text(encoding="utf-8"))
            for tool in manifest["tools"].values():
                tool["version"] = "0.0.0-never-restored"
            missing = Path(tmp) / "missing-tool.json"
            missing.write_text(json.dumps(manifest), encoding="utf-8")
            code, out = verify("-ManifestPath", str(missing))
            if code == 0:
                failures.append("a tool that was never restored verified: it must fail closed")

for failure in failures:
    print("FAIL  " + failure)
if failures:
    print(f"{len(failures)} failed")
    sys.exit(1)
if not behavioural:
    print("static checks passed; the behavioural controls were NOT RUN here (need dotnet and pwsh)")
    sys.exit(2)
print("every tool restore is followed by a signature verification, and the check fails closed")
