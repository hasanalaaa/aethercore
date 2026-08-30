#!/usr/bin/env python3
"""Focused adversarial regression proof for Sigma verification integrity.

This test deliberately avoids rerunning the expensive browser/native-adjacent evidence matrix.
The normal omega-evidence.py qualification supplies the whole-application pass. This regression
isolates the integrity mechanisms that closed the Omega verifier defect.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True


def sha(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def state(root: Path) -> dict[str, str]:
    return {
        p.relative_to(root).as_posix(): sha(p)
        for p in sorted(root.rglob("*"), key=lambda q: q.relative_to(root).as_posix())
        if p.is_file() and not p.is_symlink()
    }


def regenerate_manifest(root: Path) -> None:
    result = subprocess.run(
        [sys.executable, str(root / "scripts/regenerate-source-manifest.py")],
        cwd=root,
        text=True,
        capture_output=True,
    )
    if result.returncode:
        raise RuntimeError(result.stdout + result.stderr)


def run_omega(root: Path, evidence: Path) -> tuple[int, dict]:
    p = subprocess.run(
        [sys.executable, str(root / "scripts/omega-evidence.py"), "--output", str(evidence)],
        cwd=root,
        text=True,
        capture_output=True,
    )
    payload = json.loads(evidence.read_text(encoding="utf-8")) if evidence.is_file() else {}
    return p.returncode, payload


def load_omega(root: Path):
    path = root / "scripts/omega-evidence.py"
    spec = importlib.util.spec_from_file_location("aethercore_sigma_omega_probe", path)
    if spec is None or spec.loader is None:
        raise RuntimeError("could not load omega-evidence.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def check(name: str, condition: bool, **detail: object) -> dict[str, object]:
    return {"name": name, "ok": bool(condition), **detail}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", type=Path)
    args = parser.parse_args()
    results: list[dict[str, object]] = []
    with tempfile.TemporaryDirectory(prefix="aethercore-sigma-regression-") as td:
        temp = Path(td)

        # Case 1: the original offender is read-only by default and deterministic at source level.
        valid = temp / "valid"
        shutil.copytree(ROOT, valid, symlinks=True)
        before = state(valid)
        p = subprocess.run(
            [sys.executable, str(valid / "scripts/static_validate.py")],
            cwd=valid,
            text=True,
            capture_output=True,
        )
        after = state(valid)
        payload = json.loads(p.stdout[p.stdout.find("{"):]) if "{" in p.stdout else {}
        results.append(check(
            "valid_delivery_is_byte_stable",
            p.returncode == 0 and before == after and payload.get("ok") is True and payload.get("checks") == 342,
            exit_code=p.returncode,
            checks=payload.get("checks"),
            changed=[k for k in sorted(set(before) | set(after)) if before.get(k) != after.get(k)][:16],
        ))

        # Case 2: a manifest-covered byte tamper fails before repository audits execute.
        tampered = temp / "tampered"
        shutil.copytree(ROOT, tampered, symlinks=True)
        with (tampered / "README.md").open("a", encoding="utf-8") as f:
            f.write("\nSIGMA-MANIFEST-TAMPER\n")
        ec, evidence = run_omega(tampered, temp / "tampered-evidence.json")
        results.append(check(
            "invalid_manifest_cannot_return_zero",
            ec != 0
            and evidence.get("source_manifest_before", {}).get("ok") is False
            and "OMEGA-RB-003" in evidence.get("release_blockers", [])
            and not evidence.get("audits"),
            exit_code=ec,
        ))

        # Case 3: an audit write attempt occurs only in the disposable clone and is detected.
        mutator = temp / "mutator"
        shutil.copytree(ROOT, mutator, symlinks=True)
        audit = mutator / "scripts/phase13-reliability-audit.py"
        text = audit.read_text(encoding="utf-8")
        injection = "\n# SIGMA adversarial mutation injection\n(ROOT / 'SIGMA-MUTATION-SENTINEL').write_text('attempted\\n', encoding='utf-8')\n"
        marker = "sys.exit(0 if ok else 1)"
        if marker not in text:
            raise RuntimeError("phase13 audit exit marker not found")
        audit.write_text(text.replace(marker, injection + marker, 1), encoding="utf-8")
        regenerate_manifest(mutator)
        before = state(mutator)
        omega = load_omega(mutator)
        detail_out = temp / "mutator-phase13.json"
        result = omega.run_repo_script(
            "phase13_reliability",
            "scripts/phase13-reliability-audit.py",
            ["--output", str(detail_out)],
        )
        after = state(mutator)
        changes = result.get("source_tree_changes", [])
        results.append(check(
            "audit_source_write_is_contained_and_detected",
            result.get("exit_code") == 0
            and result.get("source_tree_unchanged") is False
            and before == after
            and any(item.get("path") == "SIGMA-MUTATION-SENTINEL" for item in changes)
            and not (mutator / "SIGMA-MUTATION-SENTINEL").exists(),
            audit_exit_code=result.get("exit_code"),
            detected_changes=changes,
        ))

        # Case 4: manifest entries are portable canonical source-relative POSIX paths only.
        traversal = temp / "traversal"
        shutil.copytree(ROOT, traversal, symlinks=True)
        omega = load_omega(traversal)
        external = temp / "external-evidence-input.txt"
        external.write_text("not part of the source tree\n", encoding="utf-8")
        manifest = traversal / "MANIFEST.sha256"
        manifest.write_text(
            manifest.read_text(encoding="utf-8") + f"{sha(external)}  ../external-evidence-input.txt\n",
            encoding="utf-8",
        )
        manifest_result = omega.verify_manifest(manifest)
        traversal_failures = manifest_result.get("failed", [])
        results.append(check(
            "manifest_path_escape_is_rejected",
            manifest_result.get("ok") is False
            and any(item.get("path") == "../external-evidence-input.txt" and item.get("reason") == "path_escape_or_noncanonical" for item in traversal_failures),
            failures=traversal_failures[:8],
        ))

        # Case 5: source deliveries are self-contained regular files; symlinks cannot redirect a
        # manifest entry to bytes outside (or elsewhere inside) the qualified tree.
        symlinked = temp / "symlinked"
        shutil.copytree(ROOT, symlinked, symlinks=True)
        link = symlinked / "README-LINK.md"
        try:
            link.symlink_to("README.md")
            omega = load_omega(symlinked)
            manifest_result = omega.verify_manifest(symlinked / "MANIFEST.sha256")
            symlink_failures = manifest_result.get("failed", [])
            symlink_ok = manifest_result.get("ok") is False and any(
                item.get("path") == "README-LINK.md" and item.get("reason") == "symlink_not_allowed"
                for item in symlink_failures
            )
            details = symlink_failures[:8]
        except (OSError, NotImplementedError) as error:
            symlink_ok = False
            details = [{"reason": "symlink_test_unavailable", "error": str(error)}]
        results.append(check(
            "source_symlink_is_rejected",
            symlink_ok,
            failures=details,
        ))

    ok = all(item["ok"] for item in results)
    payload = {
        "schema": "aethercore.sigma-evidence-integrity-test.v1",
        "ok": ok,
        "checks": len(results),
        "results": results,
    }
    rendered = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(rendered, encoding="utf-8")
    print(rendered, end="")
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
