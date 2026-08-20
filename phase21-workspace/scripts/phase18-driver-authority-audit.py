#!/usr/bin/env python3
from __future__ import annotations
import argparse, json, re, sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
P=argparse.ArgumentParser()
P.add_argument('--output',type=Path)
a=P.parse_args()
checks=[]
def add(i,ok,detail,evidence=None): checks.append({'id':i,'ok':bool(ok),'detail':detail,'evidence':evidence or []})
def text(rel): return (ROOT/rel).read_text(encoding='utf-8')
def has(rel,*needles):
    body=text(rel); missing=[n for n in needles if n not in body]; return not missing,missing
required=[
 'crates/driver-authority/Cargo.toml','crates/driver-authority/src/lib.rs','crates/driver-acquisition/Cargo.toml','crates/driver-acquisition/src/lib.rs',
 'crates/persistence/migrations/0012_phase18_driver_authority.sql','tests/fixtures/phase18_1/scenarios.json','tests/fixtures/phase18/authority-ranking-matrix.json',
 'apps/ui/src/features/drivers/DriversPage.svelte','apps/ui/src/features/drivers/controller.ts','QUALIFICATION_DEBT.json']
add('P18-ARCH-001',all((ROOT/p).is_file() for p in required),'Phase 18 source surfaces present',required)
for cid,rel,needles in [
 ('P18-AUTH-001','crates/driver-authority/src/lib.rs',['pub struct DriverAuthorityEngine','pub trait DriverProvider','AuthorityType','CoverageState','RecommendationState','VersionOrdering::Unknown']),
 ('P18-ID-001','crates/driver-authority/src/lib.rs',['pub struct DeviceIdentity','hardware_ids','compatible_ids','subsystem_vendor_id','privacy_device_key']),
 ('P18-MACHINE-001','crates/windows-foundation/src/lib.rs',['pub struct MachineIdentity','SystemManufacturer','SystemProductName','BaseBoardProduct']),
 ('P18-WUA-001','crates/driver-authority/src/lib.rs',['WindowsUpdateProvider','microsoft.windows-update','offer.update_id','offer.revision']),
 ('P18-WUA-OFFLINE-001','crates/windows-update/src/windows_impl.rs',['WU_E_NO_CONNECTION','0x8024_001F','UpdateError::Offline']),
 ('P18-UTILITY-001','crates/driver-authority/src/lib.rs',['OfficialUtilityProvider','DriverManagementProvider','management_authority']),
 ('P18-TRUST-001','crates/driver-authority/src/lib.rs',['PackageTrustState','UnexpectedPublisher','Unsigned','TestSigned','TrustRejected']),
 ('P18-PLAN-001','crates/driver-authority/src/lib.rs',['pub struct ImmutableDriverPlan','pub digest: String','PlanInvalidated','package_identity']),
 ('P18-ACQ-001','crates/driver-acquisition/src/lib.rs',['redirect(reqwest::redirect::Policy::none())','https','max_redirects','expected_sha256','PlatformVerifier','rename']),
 ('P18-PREF-001','crates/driver-authority/src/lib.rs',['IgnoreExactVersion','RemindLater','IgnoreOptional','candidate_version']),
 ('P18-IPC-001','crates/contracts/proto/operations.proto',['SetDriverCandidatePolicyRequest set_driver_candidate_policy = 70']),
 ('P18-FW-001','crates/driver-authority/src/lib.rs',['FirmwareProtected','recommended_update_all','!c.firmware']),
 ('P18-PCINTEL-001','crates/pc-intelligence/src/rules.rs',['NO_TRUSTED_CANDIDATE','DRIVER_MANAGEMENT_AUTHORITY_AVAILABLE','FIRMWARE_REVIEW_REQUIRED']),
 ('P18-REGRESS-001','crates/pc-intelligence/src/rules.rs',['corrected_or_after_crash_evidence_exposes_conflict','AfterCrash','CorrelationStrength::Moderate']),
]:
    ok,missing=has(rel,*needles); add(cid,ok,'required source markers present' if ok else f'missing markers: {missing}',[rel])
# Official source policy and no unsafe generic privileged URL/path ingress.
auth=text('crates/driver-authority/src/lib.rs'); acq=text('crates/driver-acquisition/src/lib.rs'); router=text('services/maintenance-service/src/router.rs')
auth_prod=auth.split('#[cfg(test)]',1)[0]
gpu_policy=text('crates/gpu-policy/src/lib.rs')
forbidden_prod=['attacker.example','vendor.example','driverpack','driverscape','station-drivers']
source_ok=not any(token in auth_prod.lower() or token in gpu_policy.lower() for token in forbidden_prod)
source_ok=source_ok and all(domain in gpu_policy for domain in ['nvidia.com','amd.com','intel.com'])
add('P18-SOURCE-001',source_ok,'production provider policy contains only modeled official vendor/Microsoft authority paths; hostile domains remain test-only',['crates/driver-authority/src/lib.rs','crates/gpu-policy/src/lib.rs'])
acq_prod=acq.split('#[cfg(test)]',1)[0]
add('P18-BOUNDARY-001','Url::parse(&self.package_url)' in acq_prod and 'url.scheme() != "https"' in acq_prod and 'SetDriverCandidatePolicy' in router,'acquisition validates HTTPS/provider-bound URLs and renderer preference path is candidate-bound',['crates/driver-acquisition/src/lib.rs','services/maintenance-service/src/router.rs'])
# DirectTrusted must remain unavailable from the production hub until provider-specific privileged executor exists.
hub=text('crates/driver-hub/src/lib.rs')
add('P18-DIRECT-001','candidate.installation_mode == InstallationCapability::WindowsManaged' in hub and 'DirectTrusted acquisition is modeled' in hub,'DirectTrusted is modeled but not exposed as executable production selection',['crates/driver-hub/src/lib.rs'])
# Up-to-date truth requires complete configured authority coverage.
add('P18-TRUTH-001','CompleteForRequiredAuthorities' in hub and 'permits_strong_up_to_date' in hub and 'NoUpdateFoundFromCheckedSources' in hub and 'UpdateStatusUnknown' in hub,'device-aware coverage and up-to-date truth are explicit',['crates/driver-hub/src/lib.rs'])
# EN/AR exact key parity.
def keys(rel): return set(re.findall(r"^\s*'([^']+)'\s*:",text(rel),flags=re.M))
en=keys('apps/ui/src/lib/i18n/catalog.en.ts'); ar=keys('apps/ui/src/lib/i18n/catalog.ar.ts')
add('P18-I18N-001',en==ar,f'EN/AR key parity: en={len(en)} ar={len(ar)} delta={len(en^ar)}',['apps/ui/src/lib/i18n/catalog.en.ts','apps/ui/src/lib/i18n/catalog.ar.ts'])
# Apple interaction/accessibility markers.
page=text('apps/ui/src/features/drivers/DriversPage.svelte'); css=text('apps/ui/src/design/styles/feature-layout.css'); motion=text('apps/ui/src/design/motion/fluid-press.ts')
add('P18-UX-001','fluidPress' in page and ('pointerdown' in motion.lower() or 'onpointerdown' in motion.lower()),'instant press feedback is wired to design motion primitives',['apps/ui/src/features/drivers/DriversPage.svelte','apps/ui/src/design/motion/fluid-press.ts'])
add('P18-A11Y-001','prefers-reduced-motion' in css and 'prefers-reduced-transparency' in css and ('prefers-contrast' in css or 'forced-colors' in css),'reduced-motion/transparency/contrast accommodations remain in design system',['apps/ui/src/design/styles/feature-layout.css'])
# Fixture matrix / scenarios.
sc=json.loads(text('tests/fixtures/phase18_1/scenarios.json')); ids={x['id'] for x in sc['scenarios']}; expected={f'D18-{i:02d}' for i in range(1,17)}
add('P18-LAB-001',expected.issubset(ids),f'synthetic lab scenarios={len(ids)}; D18-01..D18-16 present={expected.issubset(ids)}',['tests/fixtures/phase18_1/scenarios.json'])
mx=json.loads(text('tests/fixtures/phase18/authority-ranking-matrix.json'))
add('P18-MATRIX-001',len(mx.get('rows',[]))>=16,f'authority matrix rows={len(mx.get("rows",[]))}',['tests/fixtures/phase18/authority-ranking-matrix.json'])
# Qualification debt.
qd=json.loads(text('QUALIFICATION_DEBT.json')); p18=[x for x in qd.get('items',[]) if str(x.get('capabilityId','')).startswith('P18-')]
add('P18-DEBT-001',float(qd.get('phase',0))>=18 and len(p18)>=9,f'phase18/18.1 native debt items={len(p18)}',['QUALIFICATION_DEBT.json'])
# Source quality: scope to Phase18-specific new/changed implementation surfaces, not historic baseline docs.
quality_files=['crates/driver-authority/src/lib.rs','crates/driver-acquisition/src/lib.rs','crates/driver-hub/src/lib.rs','crates/persistence/migrations/0012_phase18_driver_authority.sql','apps/ui/src/features/drivers/DriversPage.svelte']
bad=[]
for rel in quality_files:
    b=text(rel)
    for token in ['TODO','FIXME','HACK']:
        if re.search(rf'\b{token}\b',b): bad.append(f'{rel}:{token}')
add('P18-QUALITY-001',not bad,'no TODO/FIXME/HACK in Phase18 implementation surfaces' if not bad else f'found {bad}',quality_files)
report={'schema':'aethercore.phase18.source-audit.v1','phase':18,'status':'PASS' if all(c['ok'] for c in checks) else 'FAIL','checkCount':len(checks),'checks':checks,'limitations':['Rust/Cargo Windows-native execution is outside this source audit.','Live provider endpoints, Authenticode, staging ACLs, install/rollback/reboot and WebView2 rendering remain qualification debt.']}
out=json.dumps(report,indent=2,ensure_ascii=False)+'\n'
if a.output:
    a.output.parent.mkdir(parents=True,exist_ok=True); a.output.write_text(out,encoding='utf-8')
print(out,end='')
sys.exit(0 if report['status']=='PASS' else 1)
