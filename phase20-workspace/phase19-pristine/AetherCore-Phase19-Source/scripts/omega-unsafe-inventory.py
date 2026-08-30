#!/usr/bin/env python3
"""Generate a reviewable inventory of production Rust unsafe boundaries.

This is not a proof that an unsafe block is sound. It makes the FFI surface finite and auditable,
and records whether the source carries a nearby explicit SAFETY invariant.
"""
from __future__ import annotations
import argparse, json, re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
POLICY_PATH = ROOT / "release" / "unsafe-boundary-policy.json"
BASES = [ROOT / x for x in ("apps", "crates", "services", "tools")]
UNSAFE = re.compile(r"\bunsafe\s*(?:\{|fn\b)")
FN = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:unsafe\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)")

def inventory() -> list[dict]:
    rows=[]
    for base in BASES:
        if not base.exists(): continue
        for path in sorted(base.rglob("*.rs")):
            if "tests" in path.parts or "target" in path.parts: continue
            lines=path.read_text(encoding="utf-8", errors="replace").splitlines()
            current_fn=None
            for i,line in enumerate(lines,1):
                m=FN.match(line)
                if m: current_fn=m.group(1)
                for hit in UNSAFE.finditer(line):
                    prior="\n".join(lines[max(0,i-5):i])
                    rows.append({
                        "path": path.relative_to(ROOT).as_posix(),
                        "line": i,
                        "function": current_fn,
                        "kind": "unsafe_fn" if "fn" in hit.group(0) else "unsafe_block",
                        "nearby_safety_comment": "SAFETY:" in prior,
                        "source_excerpt": line.strip()[:240],
                    })
    return rows

def main() -> int:
    ap=argparse.ArgumentParser(); ap.add_argument("--output", type=Path); args=ap.parse_args()
    rows=inventory()
    policy=json.loads(POLICY_PATH.read_text(encoding="utf-8")) if POLICY_PATH.exists() else {"contracts":{}}
    contracts=policy.get("contracts",{})
    files={r["path"] for r in rows}
    unclassified=sorted(files-set(contracts))
    stale_policy=sorted(set(contracts)-files)
    for row in rows:
        row["policy_classified"] = row["path"] in contracts
        if row["policy_classified"]:
            row["policy_domain"] = contracts[row["path"]].get("domain")
    payload={
        "schema":"aethercore.unsafe-inventory.v2",
        "production_boundaries":len(rows),
        "files":len(files),
        "policy_classified_boundaries":sum(r["policy_classified"] for r in rows),
        "unclassified_files":unclassified,
        "stale_policy_entries":stale_policy,
        "with_nearby_safety_comment":sum(r['nearby_safety_comment'] for r in rows),
        "without_nearby_safety_comment":sum(not r['nearby_safety_comment'] for r in rows),
        "policy_path":POLICY_PATH.relative_to(ROOT).as_posix(),
        "boundaries":rows,
        "qualification_note":"Policy classification is executable review governance, not a proof of soundness; compiler, Miri where applicable, and Windows-native FFI qualification remain authoritative.",
    }
    text=json.dumps(payload,indent=2,sort_keys=True)+"\n"
    if args.output:
        args.output.parent.mkdir(parents=True,exist_ok=True); args.output.write_text(text,encoding="utf-8")
    else: print(text,end="")
    return 2 if unclassified or stale_policy else 0
if __name__=="__main__": raise SystemExit(main())
