#!/usr/bin/env python3
"""Phase 29 (T5) — CycloneDX-SHAPED component inventory generator.

Parses Cargo.lock and pnpm-lock.yaml into a deterministic, sorted, CycloneDX-SHAPED
JSON inventory. Output is labeled honestly: "generated component inventory — not a
certified SBOM" (no NTIA/CycloneDX certification claim). Regeneration twice must be
byte-identical (gate: p29-sbom-*).
"""
from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path

try:
    import tomllib  # py3.11+
except ModuleNotFoundError:  # py3.9 fallback: minimal [package] parser for Cargo.lock
    tomllib = None

ROOT = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path.cwd()

NOTICE = "generated component inventory — not a certified SBOM"


def parse_cargo_lock(path: Path) -> list[dict]:
    if not path.exists():
        return []
    text = path.read_text(encoding="utf-8")
    out = []
    if tomllib is not None:
        data = tomllib.loads(text)
        for pkg in data.get("package", []):
            name = pkg.get("name", "")
            version = pkg.get("version", "")
            source = pkg.get("source", "local")
            component = {
                "type": "library",
                "name": name,
                "version": version,
                "purl": f"pkg:cargo/{name}@{version}",
            }
            if (chk := pkg.get("checksum")):
                component["hashes"] = [{"alg": "SHA-256", "content": chk}]
            out.append(component)
        return out
    # py3.9 fallback: regex over [[package]] blocks (name/version/source/checksum).
    for block in re.split(r"\[\[package\]\]", text)[1:]:
        def field(key: str) -> str:
            m = re.search(rf'^{key}\s*=\s*"([^"]+)"', block, flags=re.M)
            return m.group(1) if m else ""
        name, version = field("name"), field("version")
        if not name:
            continue
        component = {
            "type": "library",
            "name": name,
            "version": version,
            "purl": f"pkg:cargo/{name}@{version}",
        }
        chk = field("checksum")
        if chk:
            component["hashes"] = [{"alg": "SHA-256", "content": chk}]
        out.append(component)
    return out


def parse_pnpm_lock(path: Path) -> list[dict]:
    if not path.exists():
        return []
    text = path.read_text(encoding="utf-8")
    out = []
    # pnpm v9 lockfile: 'packages:' section with keys like 'name@version(peerhash):'
    in_packages = False
    seen: set[str] = set()
    for line in text.splitlines():
        if line.startswith("packages:"):
            in_packages = True
            continue
        if in_packages:
            if line and not line.startswith((" ", "\t")):
                in_packages = False
                continue
            m = re.match(r"\s+('?)([^@'\s]+)@([^:'\s(]+)", line)
            if m:
                name, version = m.group(2), m.group(3)
                key = f"{name}@{version}"
                if key not in seen and not name.startswith(("link:", "file:")):
                    seen.add(key)
                    out.append({
                        "type": "library",
                        "name": name,
                        "version": version,
                        "purl": f"pkg:npm/{name}@{version}",
                    })
    return out


def main() -> int:
    cargo = parse_cargo_lock(ROOT / "Cargo.lock")
    npm = parse_pnpm_lock(ROOT / "apps" / "ui" / "pnpm-lock.yaml")
    components = sorted(cargo + npm, key=lambda c: (c["purl"], c["version"]))
    inventory = {
        "$schema": "http://cyclonedx.org/schema/bom-1.5.schema.json",
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "_notice": NOTICE,
        "metadata": {
            "component": {
                "type": "application",
                "name": "AetherCore",
            }
        },
        "components": components,
    }
    body = json.dumps(inventory, indent=2, sort_keys=True) + "\n"
    dest = ROOT / "SBOM.cdx.json"
    dest.write_text(body)
    digest = hashlib.sha256(dest.read_bytes()).hexdigest()
    print(f"components={len(components)} cargo={len(cargo)} npm={len(npm)} sha256={digest}")
    # Programmatic cross-check: lockfile entry counts must match emitted components.
    assert len(cargo) == len(parse_cargo_lock(ROOT / "Cargo.lock"))
    assert len(npm) == len(parse_pnpm_lock(ROOT / "apps" / "ui" / "pnpm-lock.yaml"))
    assert len(components) == len(cargo) + len(npm)
    return 0


if __name__ == "__main__":
    sys.exit(main())
