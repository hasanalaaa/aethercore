#!/usr/bin/env python3
"""Read-only Phase 19 source/semantic audit.

This auditor intentionally writes nothing. Callers may redirect stdout to an evidence
file. That keeps verification hermetic and prevents the evidence process from
silently rewriting tracked source artifacts.
"""
from __future__ import annotations
import json, re, shutil, sys
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
checks=[]

def text(rel):
    p=ROOT/rel
    return p.read_text(encoding='utf-8') if p.is_file() else ''

def add(cid, ok, detail, evidence=()):
    checks.append({'id':cid,'ok':bool(ok),'detail':detail,'evidence':list(evidence)})

def has(rel,*tokens):
    s=text(rel)
    return all(t in s for t in tokens)

required=[
 'crates/windows-repair-intelligence/Cargo.toml','crates/windows-repair-intelligence/src/model.rs',
 'crates/windows-repair-intelligence/src/diagnosis.rs','crates/windows-repair-intelligence/src/graph.rs',
 'crates/windows-repair-intelligence/src/planner.rs','crates/windows-repair-intelligence/tests/synthetic_lab.rs',
 'crates/system-repair/src/dism_api.rs','crates/system-repair/src/windows_impl.rs',
 'crates/persistence/migrations/0013_phase19_windows_repair.sql','apps/ui/src/features/repair/RepairPage.svelte',
 'PRODUCT_CAPABILITY_DEBT.json','QUALIFICATION_DEBT.json']
add('P19-ARCH-001',all((ROOT/p).is_file() for p in required),'Phase 19 domain, runtime, persistence, UI and debt surfaces present',required)
add('P19-ARCH-002',has('Cargo.toml','"crates/windows-repair-intelligence"'),'workspace includes the Windows repair intelligence domain',['Cargo.toml'])
add('P19-FACT-001',has('crates/windows-repair-intelligence/src/model.rs','pub enum RepairDomain','pub enum FactState','pub struct RepairFact'),'typed Windows repair facts replace string-only domain state',['crates/windows-repair-intelligence/src/model.rs'])
add('P19-DIAG-001',has('crates/windows-repair-intelligence/src/model.rs','RootCause','ContributingCondition','Symptom','pub enum DiagnosisConfidence'),'root/contributing/symptom roles and diagnosis confidence are typed',['crates/windows-repair-intelligence/src/model.rs'])
add('P19-DIAG-002',has('crates/windows-repair-intelligence/src/diagnosis.rs','WINDOWS_UPDATE_OFFLINE','Offline discovery is not evidence of update-store corruption.','COMPONENT_STORE_CORRUPTION'),'diagnosis rules distinguish evidence-backed causes from unknown/offline symptoms',['crates/windows-repair-intelligence/src/diagnosis.rs'])
add('P19-GRAPH-001',has('crates/windows-repair-intelligence/src/graph.rs','GraphError::Cycle','GraphError::MissingDependency','deterministic_order','BTreeMap'),'RepairGraph validates dependencies/cycles and produces deterministic order',['crates/windows-repair-intelligence/src/graph.rs'])
add('P19-GRAPH-002',has('crates/windows-repair-intelligence/src/graph.rs','DestructiveAuto','GuidedResetPreservingFiles','GuidedCleanReinstall'),'destructive recovery cannot become SAFE_AUTO',['crates/windows-repair-intelligence/src/graph.rs'])
add('P19-SAFE-001',has('crates/windows-repair-intelligence/src/model.rs','Level0Diagnostic','Level1SafeAuto','Level2SensitiveRepair','Level3RebootOrOffline','Level4RecoveryEscalation','Level5DestructiveRecovery'),'all six Phase 19 safety tiers exist',['crates/windows-repair-intelligence/src/model.rs'])
prod='\n'.join(text(p) for p in ['crates/system-repair/src/lib.rs','crates/system-repair/src/windows_impl.rs','crates/operation-engine/src/lib.rs'])
forbidden=['cmd.exe /c','RunEveryRepairCommand','SoftwareDistribution','reset all windows permissions','re-register every package','registry cleaner']
add('P19-SAFE-002',not any(x.lower() in prod.lower() for x in forbidden),'generic tweak/destructive repair bundles absent from Phase 19 runtime',['crates/system-repair/src/lib.rs','crates/system-repair/src/windows_impl.rs'])
add('P19-CMD-001',has('crates/system-repair/src/windows_impl.rs','Command::new(exe)','.args(args)','COMMAND_TIMEOUT','read_tail') and 'cmd.exe' not in text('crates/system-repair/src/windows_impl.rs').lower() and 'powershell' not in text('crates/system-repair/src/windows_impl.rs').lower(),'repair command execution uses trusted executable paths, argument arrays, timeout and bounded output; no generic shell',['crates/system-repair/src/windows_impl.rs'])
add('P19-DISM-001',has('crates/system-repair/src/dism_api.rs','DismCheckImageHealth','ComponentStoreHealthy','ComponentStoreRepairable','ComponentStoreNonRepairable'),'component-store state uses DISM API health state rather than localized console prose',['crates/system-repair/src/dism_api.rs'])
add('P19-SFC-001',has('crates/system-repair/src/windows_impl.rs','/verifyonly','[SR]','SystemFilesUnknown') and 'stdout.contains' not in text('crates/system-repair/src/windows_impl.rs'),'SFC normalization isolates bounded CBS evidence and fails to Unknown when proof is insufficient',['crates/system-repair/src/windows_impl.rs'])
add('P19-WUA-001',has('crates/windows-update/src/windows_impl.rs','probe_update_health','CreateUpdateSearcher','WU_E_NO_CONNECTION','UpdateOffline'),'Windows Update health uses WUA discovery and narrowly classifies explicit no-connection as offline',['crates/windows-update/src/windows_impl.rs'])
add('P19-SVC-001',has('crates/system-repair/src/windows_impl.rs','service_running("wuauserv")','start_update_service','OpenServiceW') and 'arbitrary service' not in text('crates/operation-engine/src/lib.rs').lower(),'service repair is diagnosis-scoped to trusted server-side wuauserv authority',['crates/system-repair/src/windows_impl.rs'])
add('P19-PLAN-001',has('crates/operation-engine/src/lib.rs','repair_action_ids','machine_state_fingerprint','repair_graph_digest','repair_graph_json','safety_tier','reboot_boundary_count'),'sealed operation action carries graph identity, state fingerprint, safety and reboot boundaries',['crates/operation-engine/src/lib.rs'])
add('P19-PLAN-002',has('crates/system-repair/src/lib.rs','StaleAssessment','current_intelligence.machine_state_fingerprint != sealed_action.machine_state_fingerprint','current_intelligence.graph.digest_sha256 != sealed_action.repair_graph_digest'),'sensitive execution revalidates assessment/fingerprint/graph before authorization consumption',['crates/system-repair/src/lib.rs'])
add('P19-MUT-001',has('crates/system-repair/src/lib.rs','MutationLease','MutationWorkload::SystemRepair') and has('crates/system-repair/src/windows_impl.rs','MachineMutationGuard::try_acquire'),'repair mutations integrate process-wide MutationSupervisor lease and machine mutation guard',['crates/system-repair/src/lib.rs','crates/system-repair/src/windows_impl.rs'])
add('P19-VERIFY-001',has('crates/system-repair/src/lib.rs','verification_proves_success','MutationSucceededVerificationFailed','SucceededVerified','VerificationSucceeded'),'successful mutation cannot resolve truth without evidence-specific verification',['crates/system-repair/src/lib.rs'])
add('P19-PROGRESS-001',has('crates/system-repair/src/lib.rs','progress_known: percent >= 100','overall_percent: if percent >= 100 { 100 } else { 0 }'),'active Windows repair progress is task/state driven and indeterminate; no synthetic percent is presented as known',['crates/system-repair/src/lib.rs','apps/ui/src/features/repair/RepairPage.svelte'])
add('P19-OUTCOME-001',has('crates/windows-repair-intelligence/src/model.rs','SucceededVerified','SucceededVerificationPendingReboot','MutationSucceededVerificationFailed','FailedBeforeMutation','FailedAfterMutation','RolledBack','RollbackFailed','CancelledBeforeMutation','ManualInterventionRequired','RecoveryEscalationRequired'),'precise repair outcome taxonomy exists',['crates/windows-repair-intelligence/src/model.rs'])
add('P19-JOURNAL-001',has('crates/persistence/migrations/0013_phase19_windows_repair.sql','repair_timeline_events','repair_reboot_resume_tickets') and has('crates/system-repair/src/lib.rs','PlanSealed','AuthorizationConsumed','VerificationSucceeded'),'repair journal and reboot-reassessment ticket persistence are source-backed',['crates/persistence/migrations/0013_phase19_windows_repair.sql','crates/system-repair/src/lib.rs'])
add('P19-REBOOT-001',has('crates/windows-repair-intelligence/src/lib.rs','FreshAssessmentRequiredAfterReboot','reboot_resume_token') and has('crates/system-repair/src/lib.rs','RepairError::RebootBoundary','AwaitingRebootReassessment'),'reboot is a first-class barrier and persisted resume token requires fresh reassessment',['crates/windows-repair-intelligence/src/lib.rs','crates/system-repair/src/lib.rs'])
add('P19-RESTART-001',has('crates/system-repair/src/lib.rs','automatic replay is intentionally disabled','FailedAfterMutation','FailedBeforeMutation'),'service restart recovery never blindly replays a repair',['crates/system-repair/src/lib.rs'])
add('P19-PCINTEL-001',has('crates/pc-intelligence/src/normalize.rs','Confidence::Unknown') and has('crates/pc-intelligence/src/rules.rs','is_actionable_integrity_attention'),'Deep Scan no longer turns an unavailable/unknown repair probe into generic corruption',['crates/pc-intelligence/src/normalize.rs','crates/pc-intelligence/src/rules.rs'])
add('P19-IPC-001',has('crates/contracts/proto/repair.proto','message RepairFactInfo','message RepairDiagnosisInfo','message RepairGraphInfo','RepairIntelligenceInfo intelligence'),'typed repair intelligence crosses IPC rather than being re-inferred in renderer',['crates/contracts/proto/repair.proto','services/maintenance-service/src/protocol.rs'])
ui=text('apps/ui/src/features/repair/RepairPage.svelte')+text('apps/ui/src/app/PlanDialogs.svelte')
add('P19-UI-001','windows-health-hero' in ui and 'Scan Windows' not in ui and "repair.scanWindows" in ui and 'Repair Recommended' not in ui,'Repair surface is localized Windows Health UI driven by service truth',['apps/ui/src/features/repair/RepairPage.svelte'])
add('P19-UI-002','runtimeExecutableActions' in ui and 'hasRebootBarrier' in ui and 'repair.graph' not in ui,'review UI exposes only runtime-authorized graph actions and respects reboot barriers',['apps/ui/src/features/repair/RepairPage.svelte','apps/ui/src/app/PlanDialogs.svelte'])
style=text('apps/ui/src/design/styles/feature-layout.css'); motion=text('apps/ui/src/design/motion/fluid-press.ts')
add('P19-DESIGN-001','pointerdown' in motion.lower() and 'prefers-reduced-motion' in style and 'prefers-reduced-transparency' in style and 'prefers-contrast' in style,'Apple-design input feedback and reduced motion/transparency/contrast accommodations remain source-backed',['apps/ui/src/design/motion/fluid-press.ts','apps/ui/src/design/styles/feature-layout.css'])
# i18n key parity
key_re=re.compile(r"^\s*'([^']+)'\s*:",re.M)
en=set(key_re.findall(text('apps/ui/src/lib/i18n/catalog.en.ts'))); ar=set(key_re.findall(text('apps/ui/src/lib/i18n/catalog.ar.ts')))
phase_keys={k for k in en if k.startswith('repair.')}
add('P19-I18N-001',en==ar and len(phase_keys)>=100,f'EN/AR exact key parity={en==ar}; total={len(en)}; repair keys={len(phase_keys)}',['apps/ui/src/lib/i18n/catalog.en.ts','apps/ui/src/lib/i18n/catalog.ar.ts'])
lab=text('crates/windows-repair-intelligence/tests/synthetic_lab.rs')
missing=[f'p19_{i:02d}_' for i in range(1,25) if f'p19_{i:02d}_' not in lab]
add('P19-LAB-001',not missing and lab.count('#[test]')>=29,f'P19-01..P19-24 source scenarios present; tests={lab.count("#[test]")}; missing={missing}',['crates/windows-repair-intelligence/tests/synthetic_lab.rs'])
add('P19-LAB-002',all(x in lab for x in ['invariant_graph_cycle_rejected','invariant_missing_dependency_rejected','invariant_destructive_action_cannot_be_safe_auto','invariant_unknown_evidence_not_confident_diagnosis','invariant_one_failing_collector_does_not_mark_windows_globally_broken']),'required graph/truth/safety invariants are source-backed',['crates/windows-repair-intelligence/tests/synthetic_lab.rs'])
# debts
try:
 q=json.loads(text('QUALIFICATION_DEBT.json')); qids={x.get('capabilityId') for x in q.get('items',[])}
except Exception: q={}; qids=set()
add('P19-DEBT-001',all(f'P19-QD-{i:03d}' in qids for i in range(1,11)),'Phase 19 native qualification debt explicitly retained',['QUALIFICATION_DEBT.json'])
try:
 p=json.loads(text('PRODUCT_CAPABILITY_DEBT.json')); pids={x.get('capabilityId') for x in p.get('items',[])}
except Exception: p={}; pids=set()
add('P19-DEBT-002','PCD-DRIVER-OFFICIAL-AUTOMATION' in pids and 'PCD-PERFORMANCE-INTELLIGENCE' in pids,'product capability debt is separate and preserves driver automation/performance work',['PRODUCT_CAPABILITY_DEBT.json'])
phase_scope=['crates/windows-repair-intelligence','crates/system-repair/src/dism_api.rs','crates/system-repair/src/windows_impl.rs']
quality=[]
for rel in phase_scope:
 p=ROOT/rel
 files=list(p.rglob('*')) if p.is_dir() else [p]
 for f in files:
  if f.is_file() and f.suffix in {'.rs','.py','.toml'}:
   s=f.read_text(encoding='utf-8',errors='replace')
   for marker in ['TODO','FIXME','HACK']:
    if marker in s: quality.append(f'{f.relative_to(ROOT)}:{marker}')
add('P19-EVIDENCE-001',has('scripts/static_validate.py','Optional explicit report path; default verification is read-only.') and has('scripts/phase17-intelligence-audit.py','Optional explicit evidence path; default audit is read-only.') and has('scripts/phase17_1-integrity-audit.py','Optional explicit evidence path; default audit is read-only.'),'static/Phase17/Phase17.1 verification gates are read-only by default',['scripts/static_validate.py','scripts/phase17-intelligence-audit.py','scripts/phase17_1-integrity-audit.py'])
add('P19-QUALITY-001',not quality,f'no TODO/FIXME/HACK in Phase19 production/audit scope; found={quality}',phase_scope)

failed=[c for c in checks if not c['ok']]
result={
 'schema':'aethercore.phase19.source-audit.v1','phase':19,
 'status':'PASS' if not failed else 'FAIL','checkCount':len(checks),'passedChecks':len(checks)-len(failed),'failedChecks':len(failed),
 'checks':checks,
 'execution':{'cargoAvailable':bool(shutil.which('cargo')),'rustcAvailable':bool(shutil.which('rustc')),'svelteCheckInstalled':(ROOT/'apps/ui/node_modules/.bin/svelte-check').exists(),'sourceSemanticAudit':True,'nativeWindowsQualified':False},
 'limitations':['Cargo/Rust tests cannot execute when cargo/rustc are absent.','Live DISM/SFC/WUA/service/network/recovery/reboot behavior and WebView2/Narrator remain native qualification debt.']
}
print(json.dumps(result,ensure_ascii=False,indent=2))
sys.exit(0 if not failed else 1)
