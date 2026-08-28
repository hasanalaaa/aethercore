#!/usr/bin/env python3
"""Verify the Phase 35 patch or its scoped full-tree ledger."""
from __future__ import annotations
import hashlib, json, pathlib, sys

EXCLUDED={"target","node_modules",".git","dist","state","support-staging","__pycache__"}
SELF_PREFIX="PHASE_35_BINARY_SAFE_PATCH/"
def sha256(path):
    h=hashlib.sha256()
    with path.open("rb") as f:
        for b in iter(lambda:f.read(1024*1024),b""): h.update(b)
    return h.hexdigest()
def excluded(root,path):
    rel=path.relative_to(root)
    return path.name==".DS_Store" or any(part in EXCLUDED for part in rel.parts) or str(rel).startswith(SELF_PREFIX)
def file_map(root):
    return {str(p.relative_to(root)):sha256(p) for p in sorted(root.rglob("*")) if p.is_file() and not excluded(root,p)}
def main():
    root=pathlib.Path(sys.argv[1] if len(sys.argv)>1 else ".").resolve(); here=pathlib.Path(__file__).resolve()
    manifest=json.loads((here.parent/"MANIFEST.json").read_text()); problems=[]; checked=0
    for entry in manifest.get("files",[]):
        removed=entry.startswith("-"); rel=entry[1:] if entry[:1] in "+-" else entry; path=root/rel
        if removed:
            if path.exists(): problems.append(f"removed file still present: {rel}")
            continue
        if not path.is_file(): problems.append(f"missing: {rel}"); continue
        checked+=1
        if sha256(path)!=manifest.get("sha256",{}).get(rel): problems.append(f"hash mismatch: {rel}")
    listed_binary={r[1:] for r in manifest.get("files",[]) if r.startswith("+") and r[1:] in set(manifest.get("binaryFiles",[]))}
    if listed_binary != set(manifest.get("binaryFiles",[])): problems.append("binary file manifest mismatch")
    if "--full-tree" in sys.argv:
        ledger=json.loads((here.parent/"PHASE_35_EXPECTED_FULL_SHA256.json").read_text())["files"]; actual=file_map(root)
        problems += [f"ledger mismatch: {r}" for r in sorted(set(ledger)^set(actual))]
        problems += [f"hash mismatch: {r}" for r in sorted(set(ledger)&set(actual)) if ledger[r]!=actual[r]]
        result={"schema":"aethercore.phase35.binary-safe-verify.v1","mode":"full-tree","checked":len(actual),"expected_total":len(ledger),"problems":problems[:50],"problem_count":len(problems),"status":"PASS" if not problems and len(actual)==len(ledger) else "FAIL"}
    else:
        result={"schema":"aethercore.phase35.binary-safe-verify.v1","mode":"patch","checked":checked,"expected_total":len(manifest.get("files",[])),"problems":problems,"problem_count":len(problems),"status":"PASS" if not problems else "FAIL"}
    print(json.dumps(result,indent=2)); return 0 if result["status"]=="PASS" else 1
if __name__=="__main__": raise SystemExit(main())
