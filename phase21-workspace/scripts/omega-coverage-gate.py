#!/usr/bin/env python3
"""Evaluate llvm-cov JSON against AetherCore's critical state/security coverage policy."""
from __future__ import annotations
import argparse, json, os
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
POLICY=ROOT/'release'/'critical-test-coverage-policy.json'

def source_markers(policy:dict)->dict:
    corpus='\n'.join(p.read_text(encoding='utf-8',errors='replace') for p in ROOT.rglob('*.rs') if 'target' not in p.parts)
    missing=[m for m in policy.get('required_test_markers',[]) if m not in corpus]
    return {'ok':not missing,'missing':missing,'required':len(policy.get('required_test_markers',[]))}

def normalize(name:str)->str:
    name=name.replace('\\','/')
    root=str(ROOT).replace('\\','/')+'/'
    return name[len(root):] if name.startswith(root) else name

def inspect(path:Path)->dict:
    policy=json.loads(POLICY.read_text(encoding='utf-8'))
    result={'schema':'aethercore.critical-coverage-status.v1','approved':False,'coverage_path':str(path),'source_markers':source_markers(policy),'files':{}}
    if not path.is_file(): result['reason']='llvm-cov JSON is missing'; return result
    try: payload=json.loads(path.read_text(encoding='utf-8-sig'))
    except Exception as exc: result['reason']=f'coverage JSON unreadable: {exc}'; return result
    records={}
    for data in payload.get('data',[]):
      for file in data.get('files',[]):
        records[normalize(str(file.get('filename','')))]=file
    ok=result['source_markers']['ok']
    for rel,minimum in policy.get('files',{}).items():
        rec=records.get(rel)
        percent=None if rec is None else rec.get('summary',{}).get('lines',{}).get('percent')
        passed=isinstance(percent,(int,float)) and float(percent)>=float(minimum)
        result['files'][rel]={'minimum_line_percent':minimum,'actual_line_percent':percent,'passed':passed}
        ok=ok and passed
    result['approved']=bool(ok)
    if not ok: result['reason']='one or more critical modules/required test markers fail coverage policy'
    return result

def main()->int:
    ap=argparse.ArgumentParser(); ap.add_argument('--coverage',type=Path,default=ROOT/'out'/'omega-rust-coverage.json'); ap.add_argument('--json',action='store_true'); args=ap.parse_args()
    result=inspect(args.coverage)
    if args.json: print(json.dumps(result,indent=2,sort_keys=True))
    elif result['approved']: print('Critical Rust coverage: APPROVED')
    else: print('Critical Rust coverage: BLOCKED - '+result.get('reason','unknown'),file=os.sys.stderr)
    return 0 if result['approved'] else 2
if __name__=='__main__': raise SystemExit(main())
