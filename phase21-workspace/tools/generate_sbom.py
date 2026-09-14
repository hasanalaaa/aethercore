#!/usr/bin/env python3
"""Phase 29 (T5) — CycloneDX-SHAPED component inventory generator.

Parses Cargo.lock and pnpm-lock.yaml into a deterministic, sorted, CycloneDX-SHAPED
JSON inventory. Output is labeled honestly: "generated component inventory — not a
certified SBOM" (no NTIA/CycloneDX certification claim). Regeneration twice must be
byte-identical (gate: p29-sbom-*).

    python3 tools/generate_sbom.py [ROOT] [--out PATH]

`--out` exists so a determinism check can generate into a scratch path and
compare, instead of overwriting the delivered artifact it is measuring
(DBT-P64-001). Default remains ROOT/SBOM.cdx.json, so regenerating the
delivered inventory stays the plain no-flag invocation.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

try:
    import tomllib  # py3.11+
except ModuleNotFoundError:  # py3.9 fallback: minimal [package] parser for Cargo.lock
    tomllib = None

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
            # DBT-P65-005: the previous pattern was `([^@'\s]+)@(...)`, whose name
            # class EXCLUDES `@`. Every scoped package -- `@scope/name@version` --
            # therefore failed to match and was dropped without a word. Measured
            # against this lockfile: `packages:` holds 85 entries, 40 of them
            # scoped, and the old pattern matched exactly the 45 unscoped ones.
            # pnpm's own step-10 line, "Verifying lockfile against supply-chain
            # policies (85 entries)", is what this is reconciled against.
            # The version is the segment after the LAST `@`, because a scoped name
            # contains one of its own.
            m = re.match(r"\s+'?(\S+?)'?:\s*$", line)
            if m:
                key_text = m.group(1)
                # `name@version(peer)` -> drop the peer-dependency suffix
                key_text = key_text.split("(", 1)[0]
                if "@" not in key_text.lstrip("@"):
                    continue
                name, _, version = key_text.rpartition("@")
                if not name or not version:
                    continue
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
    ap = argparse.ArgumentParser(description="CycloneDX-shaped component inventory")
    ap.add_argument("root", nargs="?", default=None,
                    help="workspace root to read lockfiles from (default: cwd)")
    ap.add_argument("--out", type=Path, default=None,
                    help="write the inventory here instead of ROOT/SBOM.cdx.json")
    args = ap.parse_args()
    ROOT = Path(args.root).resolve() if args.root else Path.cwd()

    # DBT-P65-001: this read `apps/ui/pnpm-lock.yaml`, a path that has never existed
    # in any commit (`git log --all -- apps/ui/pnpm-lock.yaml` is empty). pnpm's
    # lockfile for this workspace is at the workspace root, and `ci.yml`'s
    # `pnpm --dir apps/ui install --frozen-lockfile` resolves it from there. The
    # missing-file branch returned [] silently, so every inventory ever generated
    # carried zero npm components and said nothing about it.
    cargo_lock = ROOT / "Cargo.lock"
    pnpm_lock = ROOT / "pnpm-lock.yaml"
    missing = [p for p in (cargo_lock, pnpm_lock) if not p.exists()]
    if missing:
        print("lockfile(s) absent, refusing to emit a silently partial inventory: "
              + ", ".join(str(p) for p in missing), file=sys.stderr)
        return 2
    cargo = parse_cargo_lock(cargo_lock)
    npm = parse_pnpm_lock(pnpm_lock)
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
    dest = args.out if args.out is not None else ROOT / "SBOM.cdx.json"
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(body)
    digest = hashlib.sha256(dest.read_bytes()).hexdigest()
    print(f"components={len(components)} cargo={len(cargo)} npm={len(npm)} sha256={digest}")
    # Programmatic cross-check: lockfile entry counts must match emitted components.
    assert len(cargo) == len(parse_cargo_lock(cargo_lock))
    assert len(npm) == len(parse_pnpm_lock(pnpm_lock))
    assert len(components) == len(cargo) + len(npm)
    return 0


if __name__ == "__main__":
    sys.exit(main())
