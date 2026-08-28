#!/usr/bin/env python3
"""Run two independent P34→P35 reconstructions and prove count equations."""
from __future__ import annotations
import hashlib, json, pathlib, shutil, subprocess, sys, tempfile

ROOT=pathlib.Path(sys.argv[1] if len(sys.argv)>1 else ".").resolve()
ARCHIVE=ROOT.parent/"AetherCore-Phase34-Master-Delivery.zip"
HASH=ROOT.parent/"PHASE34_FINAL_SHA256.txt"
PATCH=ROOT/"PHASE_35_BINARY_SAFE_PATCH"
EXCLUDED={"target","node_modules",".git","dist","state","support-staging","__pycache__","PHASE_35_BINARY_SAFE_PATCH"}

def sha(path):
    h=hashlib.sha256()
    with path.open("rb") as f:
        for b in iter(lambda:f.read(1024*1024),b""): h.update(b)
    return h.hexdigest()

def fmap(root, omit_ds=True):
    out={}
    for path in sorted(root.rglob("*")):
        rel=path.relative_to(root)
        if path.is_file() and not any(x in EXCLUDED for x in rel.parts) and (not omit_ds or path.name!=".DS_Store"):
            out[str(rel)]=sha(path)
    return out

expected=HASH.read_text().split()[0]
if sha(ARCHIVE)!=expected: raise SystemExit("P34 archive hash mismatch")
live=fmap(ROOT)
raw=fmap(ROOT,False)
self_excluded=sorted(set(raw)-set(live))
cycles=[]
for number in (1,2):
    temp=pathlib.Path(tempfile.mkdtemp(prefix=f"aethercore-p35-cycle-{number}-")); reconstructed=temp/"root"; reconstructed.mkdir()
    try:
        subprocess.run(["tar","-xzf",str(ARCHIVE),"-C",str(reconstructed),"--strip-components","1"],check=True,stdout=subprocess.DEVNULL)
        shutil.copytree(PATCH,reconstructed/PATCH.name)
        apply=subprocess.run([str(reconstructed/PATCH.name/"apply_phase35_patch.sh"),str(reconstructed)],capture_output=True,text=True)
        verify=subprocess.run([sys.executable,str(reconstructed/PATCH.name/"verify_phase35.py"),str(reconstructed)],capture_output=True,text=True)
        full=subprocess.run([sys.executable,str(reconstructed/PATCH.name/"verify_phase35.py"),str(reconstructed),"--full-tree"],capture_output=True,text=True)
        rebuilt=fmap(reconstructed)
        mismatches=sorted(set(live)^set(rebuilt)|{r for r in set(live)&set(rebuilt) if live[r]!=rebuilt[r]})
        cycles.append({"cycle":number,"apply_status":"PASS" if apply.returncode==0 else "FAIL","patch_status":json.loads(verify.stdout or "{}").get("status","FAIL"),"full_tree_status":json.loads(full.stdout or "{}").get("status","FAIL"),"comparison_total":len(rebuilt),"mismatch_count":len(mismatches),"mismatches":mismatches[:20]})
    finally:
        shutil.rmtree(temp,ignore_errors=True)
ledger=json.loads((PATCH/"PHASE_35_EXPECTED_FULL_SHA256.json").read_text())["files"]
scoped=len(live); raw_total=len(raw); ledger_total=len(ledger); comparison_total=cycles[0]["comparison_total"] if cycles else 0; mismatch=max((c["mismatch_count"] for c in cycles),default=1)
status="PASS" if raw_total==scoped+len(self_excluded) and ledger_total==comparison_total==scoped and mismatch==0 and all(c["apply_status"]==c["patch_status"]==c["full_tree_status"]=="PASS" for c in cycles) else "FAIL"
report={"raw_tree_total":raw_total,"scoped_tree_total":scoped,"ledger_total":ledger_total,"comparison_total":comparison_total,"self_excluded_count":len(self_excluded),"self_excluded_paths":self_excluded,"ledger_equals_comparison":ledger_total==comparison_total,"raw_equals_scoped_plus_self_excluded":raw_total==scoped+len(self_excluded),"mismatch_count":mismatch,"status":status,"cycles":cycles}
print(json.dumps(report,indent=2)); raise SystemExit(0 if status=="PASS" else 1)
