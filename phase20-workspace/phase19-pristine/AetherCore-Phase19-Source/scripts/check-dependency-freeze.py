#!/usr/bin/env python3
"""Fail-closed, cross-platform inspection of the approved dependency freeze."""
from __future__ import annotations
import argparse, hashlib, json, re, shutil, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REQUIRED_LOCKS = [ROOT / "Cargo.lock", ROOT / "pnpm-lock.yaml"]
LOCK_BASELINE = ROOT / "release/dependency-locks.sha256"
MANIFEST_BASELINE = ROOT / "release/dependency-manifests.sha256"
METADATA = ROOT / "release/dependency-freeze.json"
BLOCKER = ROOT / "release/dependency-freeze.blocker.json"


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def expected_pnpm() -> str | None:
    try:
        value = json.loads((ROOT / "package.json").read_text(encoding="utf-8"))["packageManager"]
    except Exception:
        return None
    m = re.fullmatch(r"pnpm@([^\s]+)", str(value))
    return m.group(1) if m else None


def rust_toolchain() -> str | None:
    text = (ROOT / "rust-toolchain.toml").read_text(encoding="utf-8")
    m = re.search(r'^\s*channel\s*=\s*"([^"]+)"', text, re.M)
    return m.group(1) if m else None


def inspect() -> dict:
    required = REQUIRED_LOCKS + [LOCK_BASELINE, MANIFEST_BASELINE, METADATA]
    missing = [p.relative_to(ROOT).as_posix() for p in required if not p.is_file()]
    result = {
        "schema": "aethercore.dependency-freeze-status.v2",
        "approved": False,
        "missing": missing,
        "blocker_present": BLOCKER.is_file(),
        "expected_rust_toolchain": rust_toolchain(),
        "expected_pnpm": expected_pnpm(),
        "checks": {},
    }
    if missing:
        result["reason"] = "approved dependency freeze is incomplete"
        return result
    try:
        meta = json.loads(METADATA.read_text(encoding="utf-8-sig"))
    except Exception as exc:
        result["reason"] = f"dependency freeze metadata unreadable: {exc}"
        return result
    checks = result["checks"]
    checks["schema"] = meta.get("schema") == "aethercore.dependency-freeze.v1"
    checks["rust_toolchain"] = meta.get("rust_toolchain") == result["expected_rust_toolchain"]
    checks["pnpm_pin"] = meta.get("pnpm") == result["expected_pnpm"]
    checks["cargo_lock_hash"] = meta.get("cargo_lock_sha256") == sha256(ROOT / "Cargo.lock")
    checks["pnpm_lock_hash"] = meta.get("pnpm_lock_sha256") == sha256(ROOT / "pnpm-lock.yaml")
    checks["manifest_baseline_hash"] = meta.get("manifest_baseline_sha256") == sha256(MANIFEST_BASELINE)
    checks["lock_baseline_hash"] = meta.get("lock_baseline_sha256") == sha256(LOCK_BASELINE)
    checks["blocker_absent"] = not BLOCKER.exists()
    result["approved"] = all(checks.values())
    if not result["approved"]:
        result["reason"] = "freeze bytes/tool pins do not match the approved metadata"
    return result


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()
    result = inspect()
    if args.json:
        print(json.dumps(result, indent=2, sort_keys=True))
    elif result["approved"]:
        print("Dependency freeze: APPROVED")
    else:
        print("Dependency freeze: BLOCKED - " + result.get("reason", "unknown reason"), file=sys.stderr)
        if result["missing"]:
            print("Missing: " + ", ".join(result["missing"]), file=sys.stderr)
    return 0 if result["approved"] else 2

if __name__ == "__main__":
    raise SystemExit(main())
