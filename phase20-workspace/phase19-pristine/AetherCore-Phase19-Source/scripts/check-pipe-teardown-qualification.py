#!/usr/bin/env python3
"""Fail closed until native named-pipe teardown evidence proves deterministic resource bounds."""
from __future__ import annotations
import argparse, json, os
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
DEFAULT=ROOT/'out'/'omega-pipe-teardown.json'
BLOCKER=ROOT/'release'/'pipe-teardown-qualification.blocker.json'
REQUIRED={
 'slow_non_reading_peer','cancel_before_syscall','cancel_after_syscall_begin','service_stop',
 'client_termination','server_termination','reconnect_storm','backpressure_saturation',
 'partial_frame','half_open_direction','worker_exhaustion'
}

def inspect(path:Path)->dict:
    result={
      'schema':'aethercore.pipe-teardown-qualification-status.v1','approved':False,
      'evidence_path':path.relative_to(ROOT).as_posix() if path.is_relative_to(ROOT) else str(path),
      'blocker_present':BLOCKER.is_file(),'required_scenarios':sorted(REQUIRED),
    }
    if not path.is_file():
        result['reason']='native teardown evidence is missing'; return result
    try: data=json.loads(path.read_text(encoding='utf-8-sig'))
    except Exception as exc:
        result['reason']=f'evidence is unreadable: {exc}'; return result
    checks={}
    checks['schema']=data.get('schema')=='aethercore.pipe-teardown-evidence.v1'
    checks['platform']=str(data.get('platform','')).lower().startswith('windows')
    checks['architecture']=data.get('architecture') in {'synchronous-cancel-synchronous-io','overlapped-event-driven'}
    scenarios={str(v.get('id')):v for v in data.get('scenarios',[]) if isinstance(v,dict)}
    missing=sorted(REQUIRED-set(scenarios))
    checks['scenario_set_complete']=not missing
    checks['scenario_results']=not missing and all(
        scenarios[s].get('passed') is True
        and int(scenarios[s].get('rounds',0)) >= 100
        and 0 <= float(scenarios[s].get('max_teardown_ms',float('inf'))) <= 2000
        for s in REQUIRED
    )
    resource=data.get('resource_delta',{}) if isinstance(data.get('resource_delta'),dict) else {}
    checks['resource_bounds']=(
        abs(int(resource.get('handles',10**9))) <= 8
        and abs(int(resource.get('threads',10**9))) <= 4
        and abs(int(resource.get('private_bytes',10**18))) <= 32*1024*1024
    )
    checks['service_recovered']=data.get('service_recovered') is True
    checks['blocker_removed']=not BLOCKER.exists()
    result.update({'checks':checks,'missing_scenarios':missing,'architecture':data.get('architecture')})
    result['approved']=all(checks.values())
    if not result['approved']: result['reason']='native teardown evidence does not satisfy every fail-closed bound'
    return result

def main()->int:
    ap=argparse.ArgumentParser(); ap.add_argument('--evidence',type=Path,default=DEFAULT); ap.add_argument('--json',action='store_true'); args=ap.parse_args()
    result=inspect(args.evidence)
    if args.json: print(json.dumps(result,indent=2,sort_keys=True))
    elif result['approved']: print('Named-pipe teardown qualification: APPROVED')
    else: print('Named-pipe teardown qualification: BLOCKED - '+result.get('reason','unknown'),file=os.sys.stderr)
    return 0 if result['approved'] else 2
if __name__=='__main__': raise SystemExit(main())
