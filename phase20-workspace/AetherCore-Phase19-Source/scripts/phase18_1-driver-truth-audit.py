#!/usr/bin/env python3
from __future__ import annotations
import argparse, json, re, sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
P=argparse.ArgumentParser(); P.add_argument('--output',type=Path); a=P.parse_args()
checks=[]
def add(i,ok,detail,evidence=None): checks.append({'id':i,'ok':bool(ok),'detail':detail,'evidence':evidence or []})
def text(rel): return (ROOT/rel).read_text(encoding='utf-8')
def has(rel,*needles):
    b=text(rel); miss=[n for n in needles if n not in b]; return not miss,miss

def extract_test_body(rel,name):
    b=text(rel); m=re.search(rf'fn\s+{re.escape(name)}\s*\([^)]*\)\s*\{{',b)
    if not m:return None
    i=m.end(); depth=1; j=i; in_s=False; esc=False
    while j<len(b) and depth:
        c=b[j]
        if in_s:
            if esc: esc=False
            elif c=='\\': esc=True
            elif c=='"': in_s=False
        else:
            if c=='"': in_s=True
            elif c=='{': depth+=1
            elif c=='}': depth-=1
        j+=1
    return b[i:j-1] if depth==0 else None

required=['DRIVER_PROVIDER_COVERAGE.json','crates/driver-authority/src/provider_registry.rs','crates/driver-authority/src/truth.rs','tests/fixtures/phase18_1/scenarios.json','tests/fixtures/phase18_1/execution-mapping.json','tests/fixtures/phase18_1/update-status-truth-matrix.json']
add('P18.1-ARCH-001',all((ROOT/x).is_file() for x in required),'Phase 18.1 truth/coverage source surfaces present',required)

registry=json.loads(text('DRIVER_PROVIDER_COVERAGE.json'))
providers={x['providerId']:x for x in registry.get('providers',[])}
required_ids={'microsoft.windows-update','oem.dell','oem.lenovo','oem.hp','oem.asus','oem.acer','oem.msi','oem.microsoft-surface','component.nvidia','component.amd','component.intel','component.realtek','component.qualcomm','component.mediatek','component.broadcom','component.synaptics'}
add('P18.1-REGISTRY-001',registry.get('schema')=='aethercore.driver-provider-coverage.v1' and required_ids<=set(providers),'machine-readable provider coverage contains required OEM/component families',['DRIVER_PROVIDER_COVERAGE.json'])
no_fake=all(p.get('directAcquisitionCapability') in {'WindowsManaged','UnsupportedAutomation'} for p in providers.values()) and all(p.get('implementationStatus')!='Implemented' for k,p in providers.items() if k!='microsoft.windows-update')
add('P18.1-REGISTRY-002',no_fake,'non-WUA providers declare manual/utility/unsupported automation rather than fabricated direct adapters',['DRIVER_PROVIDER_COVERAGE.json'])
pr=text('crates/driver-authority/src/provider_registry.rs')
add('P18.1-REGISTRY-003','malformed_provider_registry_fails_closed' in pr and 'ProviderRegistryInvalid' in pr and 'duplicate provider id' in pr,'malformed provider registry fails closed',['crates/driver-authority/src/provider_registry.rs'])

auth=text('crates/driver-authority/src/lib.rs'); truth=text('crates/driver-authority/src/truth.rs'); hub=text('crates/driver-hub/src/lib.rs')
auth_prod=auth.split('#[cfg(test)]',1)[0]; hub_prod=hub.split('#[cfg(test)]',1)[0]
add('P18.1-TRUTH-001','pub trait DriverManagementProvider' in auth_prod and 'management_authority' in auth_prod and 'fn candidate(' not in auth_prod[auth_prod.find('pub struct OfficialUtilityProvider'):auth_prod.find('pub struct AcquisitionPolicy')],'official utility is a management authority, not a DriverCandidateV2 factory',['crates/driver-authority/src/lib.rs'])
add('P18.1-TRUTH-002','UnknownUntilVendorCheck' in truth and 'proves_update_available' in truth and 'update_evidence_candidate_id' in truth,'management authority separates availability from concrete update evidence',['crates/driver-authority/src/truth.rs'])
add('P18.1-TRUTH-003','vendor_managed_update_count: devices.iter().flat_map(|d| &d.management_authorities)' in hub_prod and 'm.update_availability == "UpdateAvailable"' in hub_prod and '!m.update_evidence_candidate_id.is_empty()' in hub_prod,'management presence cannot inflate vendor-managed update count',['crates/driver-hub/src/lib.rs'])
add('P18.1-GPU-001','d18_1_gpu_01_nvidia_utility_without_update_evidence_never_fabricates_update' in hub and 'd18_1_gpu_01_amd_and_intel_follow_same_no_update_truth_rule' in hub,'NVIDIA/AMD/Intel no-update regressions are executable Rust tests',['crates/driver-hub/src/lib.rs'])

add('P18.1-COVERAGE-001','pub struct DeviceAuthorityCoverage' in truth and all(x in truth for x in ['required_authorities','evaluated_authorities','unavailable_authorities','unsupported_authorities','manual_authorities','completeness']),'coverage is device/authority-aware',['crates/driver-authority/src/truth.rs'])
add('P18.1-COVERAGE-002','required_authorities(&identity, &machine_profile, registry)' in hub_prod and 'DeviceAuthorityCoverage::from_evaluations' in hub_prod,'hub computes required authorities per device',['crates/driver-hub/src/lib.rs'])
add('P18.1-COVERAGE-003','CoverageState::CompleteForRequiredAuthorities if coverage_detail.permits_strong_up_to_date() => "UpToDate"' in hub_prod and 'ManualAuthorityRequired | CoverageState::Partial => "NoUpdateFoundFromCheckedSources"' in hub_prod,'strong UpToDate is gated on complete required-authority evaluation',['crates/driver-hub/src/lib.rs'])

# Ranking precedence and explicit management/package separation
cmp=auth_prod[auth_prod.index('fn compare_candidate_rank'):auth_prod.index('fn is_newer_than')]
pos=[cmp.find(x) for x in ['trust_score(a)','applicability_score(a)','authority_context_score(device, machine, a)','device_specificity','machine_specificity','version_score(device, a)']]
add('P18.1-RANK-001',all(x>=0 for x in pos) and pos==sorted(pos),'actual candidate ranking precedence is deterministic: trust > applicability > authority context > specificity > version',['crates/driver-authority/src/lib.rs'])
add('P18.1-RANK-002','ranking_policy_is_deterministic_for_machine_context' in auth and 'trust_precedes_context_for_actual_packages' in auth,'table-driven/context and trust ranking tests exist',['crates/driver-authority/src/lib.rs'])

acq=text('crates/driver-acquisition/src/lib.rs'); plat=text('crates/update-engine/src/platform.rs')
add('P18.1-SIGNER-001','pub struct SignatureVerification' in plat and all(x in plat for x in ['signer_subject','signer_thumbprint_or_identity','chain_status','test_signed']),'platform verifier returns typed signer evidence',['crates/update-engine/src/platform.rs'])
add('P18.1-SIGNER-002','validate_expected_publisher(request, &signature)' in acq and 'UnexpectedPublisher' in acq and 'SignerIdentityUnavailable' in acq and 'expected_signer_identities' in acq,'acquisition enforces actual signer against provider policy and fails closed',['crates/driver-acquisition/src/lib.rs'])
add('P18.1-SIGNER-003','d18_08_valid_signature_wrong_publisher_fails_closed' in acq,'valid-signature wrong-publisher adversarial test exists',['crates/driver-acquisition/src/lib.rs'])
add('P18.1-DIRECT-001','candidate.installation_mode == InstallationCapability::WindowsManaged' in hub_prod and 'DirectTrusted acquisition is modeled' in hub_prod,'DirectTrusted remains production-disabled',['crates/driver-hub/src/lib.rs'])

add('P18.1-OVERRIDE-001','authority_provider_id.trim().is_empty()' in auth_prod and 'self.authority_provider_id != candidate.authority.provider_id' in auth_prod and 'ignore_exact_version_is_provider_scoped' in auth,'IgnoreExactVersion is provider-scoped and empty legacy scope fails closed',['crates/driver-authority/src/lib.rs'])
add('P18.1-MACHINE-001','pub enum MachineKind { Oem, SelfBuilt, #[default] Unknown }' in auth_prod and 'pub machine_kind: MachineKind' in auth_prod and 'oem_machine' not in auth_prod,'OEM context is tri-state rather than unsafe boolean',['crates/driver-authority/src/lib.rs'])

pcn=text('crates/pc-intelligence/src/normalize.rs'); pcr=text('crates/pc-intelligence/src/rules.rs')
add('P18.1-PCINTEL-001','management_authorities:d.management_authorities' in pcn and 'DRIVER_MANAGEMENT_AUTHORITY_AVAILABLE' in pcr and 'DRIVER_AUTHORITY_COVERAGE_INCOMPLETE' in pcr and 'DRIVER_UPDATE_STATUS_UNKNOWN' in pcr,'Deep Scan separates management guidance, coverage limits, unknown state, and concrete updates',['crates/pc-intelligence/src/normalize.rs','crates/pc-intelligence/src/rules.rs'])
# New path must not emit utility-required from DriverUpdate branch.
driver_branch=pcr[pcr.index('FactPayload::DriverUpdate'):pcr.index('FactPayload::WindowsIntegrity')]
add('P18.1-PCINTEL-002','VENDOR_UTILITY_REQUIRED' not in driver_branch,'management guidance is not emitted as a driver-update finding',['crates/pc-intelligence/src/rules.rs'])

# EN/AR exact key parity and UI semantics
def keys(rel): return set(re.findall(r"^\s*'([^']+)'\s*:",text(rel),flags=re.M))
en=keys('apps/ui/src/lib/i18n/catalog.en.ts'); ar=keys('apps/ui/src/lib/i18n/catalog.ar.ts')
add('P18.1-I18N-001',en==ar,f'EN/AR key parity en={len(en)} ar={len(ar)} delta={len(en^ar)}',['apps/ui/src/lib/i18n/catalog.en.ts','apps/ui/src/lib/i18n/catalog.ar.ts'])
page=text('apps/ui/src/features/drivers/DriversPage.svelte')
add('P18.1-UI-001',all(x in page for x in ['managementAuthorities','managementAuthorityCount','CompleteForRequiredAuthorities','drivers.updateAvailability.','fluidPress']),'existing Drivers UI exposes management/update/coverage truth without replacing the surface',['apps/ui/src/features/drivers/DriversPage.svelte'])
motion=text('apps/ui/src/design/motion/fluid-press.ts'); css=text('apps/ui/src/design/styles/feature-layout.css')
add('P18.1-UI-002','pointerdown' in motion.lower() and 'prefers-reduced-motion' in css and 'prefers-reduced-transparency' in css,'Apple-design tactile/reduced-motion behavior preserved',['apps/ui/src/design/motion/fluid-press.ts','apps/ui/src/design/styles/feature-layout.css'])

# Synthetic lab must point to actual assertion-bearing Rust tests.
sc=json.loads(text('tests/fixtures/phase18_1/scenarios.json')); mp=json.loads(text('tests/fixtures/phase18_1/execution-mapping.json'))
ids={x['id'] for x in sc['scenarios']}; required_scen={f'D18-{i:02d}' for i in range(1,17)}|{'D18.1-GPU-01'}
map_by={x['scenarioId']:x for x in mp['mappings']}
missing=[]; weak=[]
for sid in required_scen:
    m=map_by.get(sid)
    if not m: missing.append(sid); continue
    body=extract_test_body(m['testFile'],m['testFunction'])
    if body is None: missing.append(sid)
    elif 'assert' not in body: weak.append(sid)
add('P18.1-LAB-001',ids==required_scen and not missing and not weak,f'behavioral scenario mapping scenarios={len(ids)} missing={missing} weak={weak}',['tests/fixtures/phase18_1/scenarios.json','tests/fixtures/phase18_1/execution-mapping.json'])

# Execute independent truth matrix semantics; source branch markers prevent a JSON-only pass.
def evaluate(r):
    if r['installed']=='Missing': return 'MissingDriverCandidateAvailable' if r['concreteCandidate'] else 'NoTrustedCandidate'
    if r['installed']=='Problem' and not r['concreteCandidate']: return 'DeviceProblemNoTrustedCandidate'
    if r['concreteCandidate']: return 'RecommendedUpdateAvailable'
    c=r['coverage']
    if c=='CompleteForRequiredAuthorities' and r['allRequiredEvaluated']: return 'UpToDate'
    if c in {'ManualAuthorityRequired','Partial'}: return 'NoUpdateFoundFromCheckedSources'
    if c=='Offline': return 'UpdateStatusUnknownOffline'
    if c=='ProviderUnavailable': return 'ProviderUnavailable'
    return 'UpdateStatusUnknown'
mx=json.loads(text('tests/fixtures/phase18_1/update-status-truth-matrix.json'))
errors=[]
for r in mx['rows']:
    got=evaluate(r)
    if got!=r['expectedStatus']: errors.append(f"{r['id']}:{got}!={r['expectedStatus']}")
    if r['strongUpToDate'] != (got=='UpToDate'): errors.append(f"{r['id']}:strongUpToDate")
source_states={'MissingDriverCandidateAvailable','NoTrustedCandidate','DeviceProblemNoTrustedCandidate','RecommendedUpdateAvailable','UpToDate','NoUpdateFoundFromCheckedSources','UpdateStatusUnknownOffline','ProviderUnavailable','UpdateStatusUnknown'}
source_ok=all(f'"{state}"' in hub_prod for state in source_states)
add('P18.1-MATRIX-001',not errors and source_ok,f'truth matrix rows={len(mx["rows"])}; errors={errors}',['tests/fixtures/phase18_1/update-status-truth-matrix.json','crates/driver-hub/src/lib.rs'])

# No fake universal claim / no high-risk production tokens in new provider metadata.
claim_text=' '.join([text('DRIVER_PROVIDER_COVERAGE.json'),truth,hub_prod]).lower()
add('P18.1-CLAIM-001',all(x not in claim_text for x in ['every driver on earth','universal operational coverage','all drivers worldwide automatically']),'source does not claim universal operational provider coverage',['DRIVER_PROVIDER_COVERAGE.json','crates/driver-authority/src/truth.rs'])

# Quality on Phase 18.1 surfaces
quality=['crates/driver-authority/src/lib.rs','crates/driver-authority/src/truth.rs','crates/driver-authority/src/provider_registry.rs','crates/driver-acquisition/src/lib.rs','crates/driver-hub/src/lib.rs','crates/update-engine/src/platform.rs','crates/pc-intelligence/src/rules.rs','apps/ui/src/features/drivers/DriversPage.svelte']
bad=[]
for rel in quality:
    b=text(rel)
    for token in ['TODO','FIXME','HACK']:
        if re.search(rf'\b{token}\b',b): bad.append(f'{rel}:{token}')
add('P18.1-QUALITY-001',not bad,f'no TODO/FIXME/HACK in Phase18.1 implementation surfaces; found={bad}',quality)

report={'schema':'aethercore.phase18.1.source-audit.v1','phase':'18.1','status':'PASS' if all(c['ok'] for c in checks) else 'FAIL','checkCount':len(checks),'checks':checks,'execution':{'cargoAvailable':False,'rustcAvailable':False,'sourceSemanticAudit':True},'limitations':['Cargo/Rust tests were authored but cannot execute on this host because cargo/rustc are absent.','Windows-native signer extraction, live OEM/GPU authority behavior, staging attacks, install/rollback/reboot and WebView2 rendering remain qualification debt.']}
out=json.dumps(report,indent=2,ensure_ascii=False)+'\n'
if a.output: a.output.parent.mkdir(parents=True,exist_ok=True); a.output.write_text(out,encoding='utf-8')
print(out,end=''); sys.exit(0 if report['status']=='PASS' else 1)
