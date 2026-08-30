#!/usr/bin/env python3
from __future__ import annotations
import argparse, json, re, sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
PARSER=argparse.ArgumentParser();PARSER.add_argument("--output",type=Path);ARGS=PARSER.parse_args()
checks:dict[str,dict[str,object]]={}
def text(path:str)->str:
    p=ROOT/path
    return p.read_text(encoding='utf-8') if p.exists() else ''
def ok(name:str,value:bool,**extra:object)->None:
    checks[name]={'ok':bool(value),**extra}
def has(name:str,body:str,*tokens:str)->None:
    missing=[t for t in tokens if t not in body]
    checks[name]={'ok':not missing,'missing':missing}
required=[
 'release/ga-matrix.json','release/ga-witness.template.json','tools/ga-probe/Cargo.toml','tools/ga-probe/src/main.rs',
 'scripts/phase16-stress-soak.ps1','scripts/phase16-resilience-matrix.ps1','scripts/phase16-installer-lifecycle.ps1',
 'scripts/phase16-host-qualification.ps1','scripts/phase16-seal-release.ps1','scripts/verify-ga-seal.ps1',
 'scripts/verify-phase16.ps1','scripts/verify-production.ps1','docs/FINAL_PRODUCTION_QUALIFICATION.md',
 'docs/adr/0018-final-production-qualification-and-ga-seal.md','PHASE_16_DELIVERABLES.md'
]
ok('phase16_required_files',all((ROOT/p).exists() for p in required),missing=[p for p in required if not (ROOT/p).exists()])
try: matrix=json.loads(text('release/ga-matrix.json'))
except Exception: matrix={}
ok('phase16_matrix_schema',matrix.get('schema')=='aethercore.ga-matrix.v1')
ok('phase16_minimum_build_preserved',matrix.get('minimum_windows_build')==22621)
ok('phase16_three_os_lanes',len(matrix.get('required_os_lanes',[]))>=3)
coverage=matrix.get('required_coverage',{})
ok('phase16_bidi_coverage',{'en-US','ar-IQ'}<=set(coverage.get('locales',[])) and {'ltr','rtl'}<=set(coverage.get('directions',[])))
ok('phase16_dpi_refresh_coverage',{100,125,150,200}<=set(coverage.get('dpi_percent',[])) and {60,120,144}<=set(coverage.get('refresh_hz',[])))
ok('phase16_accessibility_coverage',{'keyboard-only','narrator','reduced-motion','reduced-transparency','high-contrast'}<=set(coverage.get('accessibility',[])))
ok('phase16_input_coverage',{'mouse','keyboard','touch-or-pen'}<=set(coverage.get('input',[])))
ok('phase16_update_channel_coverage',{'stable','beta'}<=set(coverage.get('update_channels',[])))
stress=matrix.get('stress',{})
ok('phase16_extended_soak_policy',stress.get('required_profile')=='extended' and int(stress.get('minimum_duration_minutes',0))>=1440)
ok('phase16_leak_thresholds_bounded',0<int(stress.get('max_private_bytes_growth_mib',0))<=256 and 0<int(stress.get('max_handle_growth',0))<=512 and 0<int(stress.get('max_thread_growth',0))<=64)
probe=text('tools/ga-probe/src/main.rs');cargo=text('Cargo.toml')
has('phase16_probe_real_ipc',probe,'SessionClient::connect','HydrateSessionRequest','PingRequest','replay_after','event.sequence <= prior')
has('phase16_probe_concurrency_bounds',probe,'sessions: parse_usize("--sessions", 4, 1, 4)?','requests_per_session','reconnect_every')
ok('phase16_probe_workspace_member','"tools/ga-probe"' in cargo)
soak=text('scripts/phase16-stress-soak.ps1')
has('phase16_soak_process_metrics',soak,'PrivateMemorySize64','HandleCount','Threads.Count','baseline-after-warmup')
has('phase16_soak_threshold_enforcement',soak,'max_private_bytes_growth_mib','max_private_bytes_growth_percent','max_handle_growth','max_thread_growth')
has('phase16_soak_live_probe',soak,'aethercore-ga-probe.exe','--sessions','--requests-per-session','--reconnect-every')
ok('phase16_soak_default_extended_24h','extended=1440' in soak)
helper=text('scripts/invoke-cargo-test-case.ps1')
has('phase16_rust_test_selector_nonzero_guard',helper,'--list','matched $($matches.Count)','exactly one test','& cargo @runArgs')
critical='\n'.join(text(p) for p in ['scripts/phase13-fault-injection.ps1','scripts/phase14-scheduler-fault-injection.ps1','scripts/run-ipc-fuzz.ps1','scripts/phase16-resilience-matrix.ps1','scripts/verify-phase16.ps1'])
ok('phase16_no_short_name_exact_false_pass','-- --exact' not in critical and 'invoke-cargo-test-case.ps1' in critical)
res=text('scripts/phase16-resilience-matrix.ps1')
has('phase16_resilience_inherits_fault_gates',res,'phase13-fault-injection.ps1','phase14-scheduler-fault-injection.ps1','phase15-crypto-tests.ps1','run-ipc-fuzz.ps1','concurrent_contenders_never_overlap_machine_mutation_leases')
has('phase16_resilience_restart_matrix',res,'Restart-Service AetherCoreMaintenance','Fresh reconnect probe failed after service restart cycle')
life=text('scripts/phase16-installer-lifecycle.ps1')
has('phase16_burn_lifecycle',life,"@('/install','/quiet','/norestart')","@('/uninstall','/quiet','/norestart')",'Burn install','Burn uninstall')
has('phase16_repair_acl_rehardening',life,"icacls.exe",'/fa','verify-installer-security.ps1','ProgramData preservation sentinel')
host=text('scripts/phase16-host-qualification.ps1')
has('phase16_host_lane_enforcement',host,'required_os_lanes','build_min','build_max','Unknown GA lane')
has('phase16_witness_all_surfaces',host,'required_manual_surfaces','Witness surface is not PASS','Witness check is not PASS')
has('phase16_host_installed_security',host,'verify-installer-security.ps1','-VerifyInstalledStateOnly','-RequireSignedArtifacts')
installer=text('scripts/verify-installer-security.ps1')
has('phase16_update_broker_elevation_checked',installer,"aethercore-update-broker.exe","Update broker PE manifest is not requireAdministrator")
seal=text('scripts/phase16-seal-release.ps1')
has('phase16_seal_matrix_aggregation',seal,'Missing PASS evidence for OS lane','Require-Set','required_coverage')
has('phase16_seal_requires_extended_soak',seal,'required_profile','minimum_duration_minutes','extended soak')
has('phase16_seal_requires_resilience_lifecycle',seal,'aethercore.ga-resilience.v1','service_restart_cycles','aethercore.ga-installer-lifecycle.v1')
has('phase16_seal_supply_chain',seal,'SHA256SUMS.txt','evidence\\sbom','validate-update-trust.ps1','Get-AuthenticodeSignature')
has('phase16_seal_cryptographic_attestation',seal,'System.Security.Cryptography.Pkcs.SignedCms','GA-SEAL.p7s','GA-EVIDENCE-SHA256SUMS.txt','signer_thumbprint')
verifyseal=text('scripts/verify-ga-seal.ps1')
has('phase16_seal_self_verification',verifyseal,'CheckSignature','release_sha256s_sha256','evidence_manifest_sha256','signer thumbprint mismatch')
verify16=text('scripts/verify-phase16.ps1')
has('phase16_master_inherits_phase15',verify16,'verify-phase15.ps1','phase16-ga-audit.ps1','cargo check --workspace --locked','static_validate.py')
has('phase16_master_no_false_ga_claim',verify16,'GA status is NOT conferred by this gate alone','verify-production.ps1')
prod=text('scripts/verify-production.ps1')
has('phase16_production_gate',prod,'phase16-seal-release.ps1','verify-ga-seal.ps1','General Availability release seal: PASS')
docs=text('docs/FINAL_PRODUCTION_QUALIFICATION.md')+'\n'+text('docs/adr/0018-final-production-qualification-and-ga-seal.md')
has('phase16_docs_evidence_model',docs,'Source/native qualification','Windows host qualification','extended soak','GA-SEAL.json','GA-SEAL.p7s')
has('phase16_docs_honest_boundary',docs,'does not constitute GA','Windows-native','evidence')
workflow=text('.github/workflows/release.yml')
enterprise=text('scripts/verify-enterprise.ps1')
ok('phase16_release_workflow_uses_phase16',(('verify-phase16.ps1' in workflow) or ('verify-enterprise.ps1' in workflow and 'verify-phase16.ps1' in enterprise)) and 'verify-phase15.ps1 -ReleasePackaging' not in workflow)
ci=text('.github/workflows/ci.yml')
ok('phase16_ci_uses_phase16',('verify-phase16.ps1' in ci) or ('verify-enterprise.ps1' in ci and 'verify-phase16.ps1' in enterprise))
readme=text('README.md')
ok('phase16_readme_surface','Phase 16' in readme and 'verify-production.ps1' in readme)
all_ok=all(v['ok'] for v in checks.values())
result={'phase':'16','ok':all_ok,'check_count':len(checks),'checks':checks,'failed':[k for k,v in checks.items() if not v['ok']]}
if ARGS.output:
    ARGS.output.parent.mkdir(parents=True,exist_ok=True)
    ARGS.output.write_text(json.dumps(result,indent=2,sort_keys=True)+'\n',encoding='utf-8')
print(json.dumps({'ok':all_ok,'checks':len(checks),'failed':result['failed']},indent=2))
sys.exit(0 if all_ok else 1)
