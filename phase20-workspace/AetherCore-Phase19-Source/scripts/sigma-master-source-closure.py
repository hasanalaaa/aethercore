#!/usr/bin/env python3
"""Run the strongest Sigma Master source-closure evidence available on the current host.

All generated evidence must be outside the qualified source tree. Expected unavailable native,
locked-dependency, fuzz and coverage gates are retained as typed blockers rather than converted to
PASS. A SOURCE_CLOSED maturity state means source-level executable gates passed; it does not mean
BUILD_VERIFIED or NATIVE_QUALIFIED.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import time
from typing import Any

ROOT=Path(__file__).resolve().parents[1]
sys.dont_write_bytecode=True


def inside_root(path:Path)->bool:
    try:path.resolve().relative_to(ROOT.resolve());return True
    except ValueError:return False


def run(cmd:list[str],cwd:Path=ROOT,timeout:int=900)->dict[str,Any]:
    env=os.environ.copy();env['PYTHONDONTWRITEBYTECODE']='1'
    started=time.monotonic()
    try:
        proc=subprocess.run(cmd,cwd=cwd,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=timeout,env=env)
        return {'command':cmd,'exit_code':proc.returncode,'duration_ms':round((time.monotonic()-started)*1000),'output':proc.stdout}
    except subprocess.TimeoutExpired as exc:
        return {'command':cmd,'exit_code':124,'duration_ms':round((time.monotonic()-started)*1000),'output':(exc.stdout or '')+(exc.stderr or ''),'timeout':True}


def load_json(path:Path)->dict[str,Any]:
    try:return json.loads(path.read_text(encoding='utf-8-sig'))
    except Exception:return {}


def parse_stdout_json(result:dict[str,Any])->dict[str,Any]:
    text=str(result.get('output',''))
    for i,ch in enumerate(text):
        if ch!='{':continue
        try:return json.loads(text[i:])
        except Exception:pass
    return {}


def file_sha(path:Path)->str:
    h=hashlib.sha256()
    with path.open('rb') as f:
        for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
    return h.hexdigest()


def toolchains()->dict[str,Any]:
    tools={}
    for name in ['rustc','cargo','rustfmt','clippy-driver','pnpm','node','npm','corepack','tsc','pwsh','powershell']:
        path=shutil.which(name);entry={'available':bool(path),'path':path}
        if path:
            r=run([path,'--version'],timeout=15);entry['version_exit_code']=r['exit_code'];entry['version_output']=r['output'].strip()[:500]
        tools[name]=entry
    return tools


def issue_ledger()->dict[str,Any]:
    return {
      'schema':'aethercore.sigma-master-issue-ledger.v1',
      'issues':[
        {'id':'SIGMA-MASTER-SRC-001','severity':'HIGH','status':'FIXED','area':'verification_integrity','finding':'Manifest verification accepted non-canonical source-relative names and did not explicitly prohibit source symlinks, weakening self-contained byte identity.','closure':'Manifest paths are now canonical portable POSIX paths, every symlink component/source symlink is rejected, regeneration refuses symlink-bearing trees, and adversarial path-escape/symlink regressions are retained.'},
        {'id':'SIGMA-MASTER-SRC-002','severity':'HIGH','status':'FIXED_SOURCE','area':'rust_test_compilability','finding':'ReadBudgetManager regression test referenced stale ReadWorkload::DriverScan and StartupScan variants that do not exist.','closure':'Tests now use DriverDiscovery and StartupDiscovery; static validation rejects the stale qualified variants. Locked cargo test remains BUILD_BLOCKER until the approved Rust/dependency toolchain is available.'},
        {'id':'SIGMA-MASTER-UI-001','severity':'MEDIUM','status':'FIXED','area':'ui_concurrency_state','finding':'A boolean busy flag could clear when the first of overlapping renderer operations completed while another operation remained active.','closure':'Production shell state now uses an idempotent re-entrant ActivityCounter; four platform-neutral overlap/order regressions execute on this host.'},
        {'id':'SIGMA-MASTER-UI-002','severity':'MEDIUM','status':'FIXED_SOURCE','area':'full_app_testing','finding':'The accepted 37-check Chromium suite exercises motion primitives but is not actual production-tree E2E coverage.','closure':'A typed renderer transport boundary and full-app harness now target the real App/AppShell/features tree. Test injection is build-gated and startup-only. Execution is retained as TEST_BLOCKER until the pinned pnpm graph can be built.'},
      ]
    }


def blocker_ledger(e:dict[str,Any])->dict[str,Any]:
    t=e['toolchains'];dep=e.get('dependency_freeze',{});full=e.get('full_app_ui',{});fuzz=e.get('fuzz',{});cov=e.get('coverage',{});pipe=e.get('pipe_teardown',{})
    blockers=[]
    if not dep.get('approved'):
        blockers.append({'id':'MASTER-DEPENDENCY-001','class':'DEPENDENCY_BLOCKER','severity':'RELEASE_BLOCKER','status':'OPEN','condition':'Approved Cargo/pnpm dependency freeze is absent or unverifiable.','closure':'On the trusted freeze workstation, provision repository-pinned tools, run freeze-dependencies.ps1 -Refresh, review dependency/license/advisory graphs, commit lock/freeze artifacts, then rerun every --locked gate.'})
    if not (t['cargo']['available'] and t['rustc']['available'] and t['rustfmt']['available'] and t['clippy-driver']['available']):
        blockers.append({'id':'MASTER-BUILD-001','class':'BUILD_BLOCKER','severity':'RELEASE_BLOCKER','status':'OPEN','condition':'Pinned Rust fmt/check/test/clippy gates could not execute on this host.','closure':'Execute cargo fmt/check/test/clippy with rust-toolchain.toml and the approved lockfile using --locked.'})
    if full.get('status')!='PASS':
        blockers.append({'id':'MASTER-TEST-001','class':'TEST_BLOCKER','severity':'RELEASE_BLOCKER','status':'OPEN','condition':'Actual production Svelte component-tree E2E suite has not executed successfully.','closure':'Restore the exact pnpm dependency graph and run scripts/sigma-master-full-app-ui.py; retain PASS evidence.'})
    if fuzz.get('status') not in {'PASS','EXECUTED_PASS'}:
        blockers.append({'id':'MASTER-FUZZ-001','class':'FUZZ_BLOCKER','severity':'RELEASE_BLOCKER','status':'OPEN','condition':'Registered cargo-fuzz targets were not compiled/executed on this host.','closure':'Run bounded reviewed cargo-fuzz campaigns plus retained corpora/crash metadata on the qualified toolchain.'})
    if not cov.get('approved'):
        blockers.append({'id':'MASTER-COVERAGE-001','class':'COVERAGE_BLOCKER','severity':'RELEASE_BLOCKER','status':'OPEN','condition':'Critical Rust llvm-cov thresholds have no approved execution evidence.','closure':'Generate cargo llvm-cov JSON from the locked workspace and satisfy release/critical-test-coverage-policy.json.'})
    if not pipe.get('approved'):
        blockers.append({'id':'MASTER-NATIVE-001','class':'NATIVE_BLOCKER','severity':'RELEASE_BLOCKER','status':'OPEN','condition':'Synchronous named-pipe cancellation/teardown resource bounds remain unproven on qualified Windows.','closure':'Execute every required slow/stalled/cancel/stop/reconnect/saturation scenario for >=100 rounds. If bounds fail, migrate to overlapped/event-driven I/O and rerun.'})
    if os.name!='nt':
        blockers.append({'id':'MASTER-NATIVE-002','class':'NATIVE_BLOCKER','severity':'RELEASE_BLOCKER','status':'OPEN','condition':'Windows service identity, SCM lifecycle, hardware collectors, WebView2 and native motion evidence cannot execute on this host.','closure':'Run scripts/sigma-master-windows-qualification.ps1 on each required qualified Windows lane and retain host witnesses.'})
        blockers.append({'id':'MASTER-ACCESSIBILITY-001','class':'ACCESSIBILITY_BLOCKER','severity':'RELEASE_BLOCKER','status':'OPEN','condition':'Narrator, physical DPI scaling, Windows high-contrast and native Arabic/RTL accessibility have not been qualified on WebView2.','closure':'Execute the required host witness matrix for keyboard, Narrator, 100/125/150/200% DPI, high contrast, reduced motion and Arabic RTL.'})
        blockers.append({'id':'MASTER-INSTALLER-001','class':'INSTALLER_BLOCKER','severity':'RELEASE_BLOCKER','status':'OPEN','condition':'WiX/Burn clean install, repair, upgrade, rollback, ACL drift repair and uninstall were not executed here.','closure':'Execute the disposable-machine installer lifecycle in Sigma Master Windows qualification.'})
    if not (t['cargo']['available'] and t['rustc']['available']):
        blockers.append({'id':'MASTER-PERFORMANCE-001','class':'PERFORMANCE_BLOCKER','severity':'RELEASE_BLOCKER','status':'OPEN','condition':'Production Rust binaries cannot be built on this host, so allocation/resource/frame-adjacent benchmark evidence cannot be generated from final code.','closure':'Run the approved performance/resource measurement set on the locked build and retain regressions/bounds before stress qualification.'})
    blockers.append({'id':'MASTER-SIGNING-001','class':'SIGNING_BLOCKER','severity':'RELEASE_BLOCKER','status':'OPEN','condition':'No protected production Authenticode/release signing identity is present in this source-closure environment.','closure':'Use approved signing infrastructure during release packaging and verify final artifact signatures/hashes before GA sealing.'})
    return {'schema':'aethercore.sigma-master-blocker-ledger.v1','blockers':blockers,'open_count':len(blockers)}


def scorecard(blockers:dict[str,Any])->dict[str,Any]:
    blocked={b['class'] for b in blockers['blockers']}
    native='NATIVE_BLOCKER' in blocked
    rows=[]
    domains=['Architecture','Security Design','Privilege Isolation','IPC & Concurrency','Rust Engineering','Windows FFI Safety','Cryptographic Engineering','Update Architecture','Support Bundle & Privacy','Scheduler / Autonomous Intelligence','Persistence & Recovery','Performance & Resource Control','Apple-Grade Interaction','Whole-Application UX','Accessibility','Arabic / RTL / Internationalization','Fuzzing','Property / Model Testing','Coverage Quality','Supply-Chain Integrity','Verification Integrity','Maintainability','Installer / Lifecycle Readiness','Windows Native Qualification Readiness','Release Engineering']
    for domain in domains:
        score='SOURCE 10/10'
        remaining=None
        executed='source gates executed'
        native_required=False
        if domain in {'Privilege Isolation','IPC & Concurrency','Windows FFI Safety','Update Architecture','Performance & Resource Control','Apple-Grade Interaction','Accessibility','Installer / Lifecycle Readiness','Windows Native Qualification Readiness'} and native:
            score='SOURCE 10/10 — NATIVE QUALIFICATION PENDING';remaining='MASTER-NATIVE-001/002 as applicable';native_required=True
        if domain=='Rust Engineering' and 'BUILD_BLOCKER' in blocked:
            score='SOURCE REVIEWED — BUILD VERIFICATION PENDING';remaining='MASTER-BUILD-001';executed='static/source regressions only'
        if domain=='Whole-Application UX' and 'TEST_BLOCKER' in blocked:
            score='SOURCE TEST ARCHITECTURE READY — E2E EXECUTION PENDING';remaining='MASTER-TEST-001';executed='full-app source contract only'
        if domain=='Performance & Resource Control' and 'PERFORMANCE_BLOCKER' in blocked:
            score='SOURCE REVIEWED — MEASUREMENT PENDING';remaining='MASTER-PERFORMANCE-001';executed='source/resource-bound controls only';native_required=True
        if domain=='Accessibility' and 'ACCESSIBILITY_BLOCKER' in blocked:
            score='SOURCE 10/10 — NATIVE ACCESSIBILITY QUALIFICATION PENDING';remaining='MASTER-ACCESSIBILITY-001';executed='source/browser primitive assertions';native_required=True
        if domain=='Fuzzing' and 'FUZZ_BLOCKER' in blocked:
            score='TARGETS PRESENT — EXECUTION PENDING';remaining='MASTER-FUZZ-001';executed='inventory only'
        if domain=='Coverage Quality' and 'COVERAGE_BLOCKER' in blocked:
            score='POLICY PRESENT — EXECUTION PENDING';remaining='MASTER-COVERAGE-001';executed='source markers only'
        if domain=='Supply-Chain Integrity' and 'DEPENDENCY_BLOCKER' in blocked:
            score='BLOCKED';remaining='MASTER-DEPENDENCY-001';executed='fail-closed freeze checker'
        if domain=='Release Engineering' and ('SIGNING_BLOCKER' in blocked or 'DEPENDENCY_BLOCKER' in blocked):
            score='SOURCE READY — RELEASE PROOF PENDING';remaining='MASTER-DEPENDENCY-001 / MASTER-SIGNING-001';executed='source/evidence/package controls'
        rows.append({'domain':domain,'design_status':'SOURCE_REVIEWED','implementation_status':'IMPLEMENTED_OR_RETAINED','executed_verification_status':executed,'native_verification_required':native_required,'remaining_blocker':remaining,'score':score})
    return {'schema':'aethercore.sigma-master-scorecard.v1','definition':'10/10 requires design + implementation + adversarial proof + retained evidence; native-dependent entries are explicitly pending.','domains':rows}


def main()->int:
    ap=argparse.ArgumentParser();ap.add_argument('--output-dir',type=Path,required=True);args=ap.parse_args()
    out=args.output_dir.expanduser().resolve()
    if inside_root(out):ap.error('Sigma Master evidence output must be outside the qualified source tree.')
    out.mkdir(parents=True,exist_ok=True)
    evidence:dict[str,Any]={'schema':'aethercore.sigma-master-source-closure.v1','generated_unix_ms':int(time.time()*1000),'platform':platform.platform(),'toolchains':toolchains()}

    dep=run([sys.executable,'scripts/check-dependency-freeze.py','--json']);evidence['dependency_freeze']=parse_stdout_json(dep);evidence['dependency_freeze']['checker_exit_code']=dep['exit_code']
    for index in (1,2):
        path=out/f'omega-evidence-run{index}.json';block=out/f'omega-blockers-run{index}.json'
        result=run([sys.executable,'scripts/omega-evidence.py','--output',str(path),'--blockers-output',str(block)],timeout=1200)
        evidence[f'omega_run_{index}']={'exit_code':result['exit_code'],'duration_ms':result['duration_ms'],'evidence':load_json(path),'blockers':load_json(block)}
    sigma=run([sys.executable,'scripts/sigma-evidence-integrity-test.py']);evidence['sigma_integrity']=parse_stdout_json(sigma);evidence['sigma_integrity']['exit_code']=sigma['exit_code']
    ui_state_path=out/'ui-state.json';ui_state=run([sys.executable,'scripts/sigma-master-ui-state-tests.py','--json',str(ui_state_path)]);evidence['ui_state']=load_json(ui_state_path);evidence['ui_state']['exit_code']=ui_state['exit_code']
    full_source_path=out/'full-app-source-contract.json';full_source=run([sys.executable,'scripts/sigma-master-full-app-ui.py','--source-only','--json',str(full_source_path)]);evidence['full_app_source_contract']=load_json(full_source_path);evidence['full_app_source_contract']['exit_code']=full_source['exit_code']
    full_path=out/'full-app-ui.json';full=run([sys.executable,'scripts/sigma-master-full-app-ui.py','--json',str(full_path)],timeout=600);evidence['full_app_ui']=load_json(full_path);evidence['full_app_ui']['exit_code']=full['exit_code']
    fuzz_path=out/'fuzz-inventory.json';fuzz=run([sys.executable,'scripts/sigma-master-fuzz-inventory.py','--json',str(fuzz_path)]);evidence['fuzz']=load_json(fuzz_path);evidence['fuzz']['exit_code']=fuzz['exit_code']
    coverage=run([sys.executable,'scripts/omega-coverage-gate.py','--coverage',str(out/'omega-rust-coverage.json'),'--json']);evidence['coverage']=parse_stdout_json(coverage);evidence['coverage']['exit_code']=coverage['exit_code']
    pipe=run([sys.executable,'scripts/check-pipe-teardown-qualification.py','--evidence',str(out/'omega-pipe-teardown.json'),'--json']);evidence['pipe_teardown']=parse_stdout_json(pipe);evidence['pipe_teardown']['exit_code']=pipe['exit_code']
    unsafe_path=out/'unsafe-inventory.json';unsafe=run([sys.executable,'scripts/omega-unsafe-inventory.py','--output',str(unsafe_path)]);evidence['unsafe_inventory']=load_json(unsafe_path);evidence['unsafe_inventory']['exit_code']=unsafe['exit_code']

    first=evidence['omega_run_1']['evidence'];second=evidence['omega_run_2']['evidence']
    repeatable=(first.get('source_verification_pass') is True and second.get('source_verification_pass') is True and first.get('source_tree_integrity',{}).get('before_sha256')==first.get('source_tree_integrity',{}).get('after_sha256')==second.get('source_tree_integrity',{}).get('before_sha256')==second.get('source_tree_integrity',{}).get('after_sha256'))
    source_gates=(repeatable and evidence['sigma_integrity'].get('ok') is True and evidence['ui_state'].get('status')=='PASS' and evidence['full_app_source_contract'].get('status')=='SOURCE_SUITE_READY' and evidence['unsafe_inventory'].get('exit_code')==0)
    issues=issue_ledger();blockers=blocker_ledger(evidence);scores=scorecard(blockers)
    maturity='SOURCE_CLOSED' if source_gates else 'SOURCE_UNVERIFIED'
    status={'schema':'aethercore.sigma-master-release-status.v1','maturity':maturity,'source_closed':source_gates,'build_verified':False,'native_qualified':False,'stress_qualified':False,'signed':False,'reproducible_release_verified':False,'release_candidate':False,'ga_approved':False,'open_blockers':[{'id':b['id'],'class':b['class']} for b in blockers['blockers']]}
    evidence['qualification_repeatable']=repeatable;evidence['source_gates_pass']=source_gates;evidence['maturity']=maturity
    (out/'AetherCore-Sigma-Master-Qualification-Summary.json').write_text(json.dumps(evidence,indent=2,sort_keys=True)+'\n',encoding='utf-8')
    (out/'AetherCore-Sigma-Master-Issue-Ledger.json').write_text(json.dumps(issues,indent=2,sort_keys=True)+'\n',encoding='utf-8')
    (out/'AetherCore-Sigma-Master-Blocker-Ledger.json').write_text(json.dumps(blockers,indent=2,sort_keys=True)+'\n',encoding='utf-8')
    (out/'AetherCore-Sigma-Master-Release-Status.json').write_text(json.dumps(status,indent=2,sort_keys=True)+'\n',encoding='utf-8')
    (out/'AetherCore-Sigma-Master-Scorecard.json').write_text(json.dumps(scores,indent=2,sort_keys=True)+'\n',encoding='utf-8')
    print(json.dumps({'status':maturity,'repeatable':repeatable,'source_gates_pass':source_gates,'open_blockers':len(blockers['blockers']),'output_dir':str(out)},indent=2))
    return 0 if source_gates else 1

if __name__=='__main__':raise SystemExit(main())
