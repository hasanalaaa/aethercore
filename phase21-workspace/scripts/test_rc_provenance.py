#!/usr/bin/env python3
"""Execute the RC acceptance/promotion verifier against hostile temporary packages.

The signature API is substituted only for portable fixtures, never in the CLI.
These prove the gate's decisions, not a production signing qualification.
"""
from __future__ import annotations
import importlib.util
import json
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.dont_write_bytecode = True
SCRIPTS = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('rc_provenance', SCRIPTS / 'rc-provenance.py')
rc = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rc)
THUMB = 'A' * 40
SURFACE_PAGES = {'hardware':'hardware','deep-scan':'deepScan','repair':'repair',
                 'care':'activity','timeline':'activity','assistant':'activity'}
SURFACE_SELECTORS = {'hardware':'.storage-grid','deep-scan':'.deep-scan-header','repair':'main',
                     'care':'.care-panel','timeline':'.timeline-panel','assistant':'.assistant-drawer'}

class PromotionTests(unittest.TestCase):
    mode = THUMB
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.tmp.cleanup)
        base = Path(cls.tmp.name)
        cls.source = base / 'repo' / 'phase21-workspace'
        cls.source.mkdir(parents=True)
        github = cls.source.parent / '.github'
        github.mkdir()
        (github / 'workflow.txt').write_text('protected signing flow')
        for name in rc.INPUTS:
            path = cls.source / name
            if name.startswith('../') or name == 'MANIFEST.sha256': continue
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('frozen input\n')
        (cls.source / 'Cargo.toml').write_text('fixture manifest')
        (cls.source / 'docs/roadmap').mkdir(parents=True)
        (cls.source / 'docs/roadmap/DECISIONS.md').write_text('| ID | Decides |\n|---|---|\n| D32 | unsigned |\n| D33 | owner host |\n| D34 | repair on the owner host |\n')
        (cls.source / 'scripts').mkdir()
        (cls.source / 'scripts/p87-installed-acceptance.ps1').write_bytes(b'exact acceptance script')
        for name in ['dependency-locks', 'dependency-manifests']:
            entries = ['Cargo.lock', 'pnpm-lock.yaml'] if name.endswith('locks') else ['Cargo.toml']
            (cls.source / ('release/' + name + '.sha256')).write_text(''.join(rc.digest(cls.source / p) + '  ' + p + '\n' for p in entries))
        rc.write_json(cls.source / 'release/dependency-freeze.json', {
            'schema':'aethercore.dependency-freeze.v1',
            'cargo_lock_sha256':rc.digest(cls.source / 'Cargo.lock'),
            'pnpm_lock_sha256':rc.digest(cls.source / 'pnpm-lock.yaml'),
            'manifest_baseline_sha256':rc.digest(cls.source / 'release/dependency-manifests.sha256'),
            'lock_baseline_sha256':rc.digest(cls.source / 'release/dependency-locks.sha256')})
        def git(*args):
            return subprocess.check_output(['git', *args], cwd=cls.source.parent, stderr=subprocess.DEVNULL).decode().strip()
        git('init', '-q'); git('config', 'user.email', 'fixture@example.invalid'); git('config', 'user.name', 'fixture')
        paths = [str(p.relative_to(cls.source.parent)) for p in cls.source.parent.rglob('*') if p.is_file() and '.git' not in p.parts]
        git('add', '--', *paths)
        subprocess.run([sys.executable, str(SCRIPTS / 'regenerate-source-manifest.py'), '--root', str(cls.source)], check=True, stdout=subprocess.DEVNULL)
        git('add', '--', 'phase21-workspace/MANIFEST.sha256', '.github/MANIFEST.sha256')
        git('commit', '-qm', 'sealed fixture')
        cls.sha = git('rev-parse', 'HEAD')
        cls.version = '0.1.11'
    def setUp(self):
        self.candidate_tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.candidate_tmp.cleanup)
        base = Path(self.candidate_tmp.name)
        self.release = base / 'candidate'
        observed = {'status':'NotSigned','thumbprint':None} if self.mode == rc.D32 else {'status':'Valid', 'thumbprint':THUMB}
        self.signatures = patch.object(rc, 'signature', return_value=observed)
        self.signatures.start(); self.addCleanup(self.signatures.stop)
        self.release.mkdir()
        self.names = rc.signed_paths(self.version)
        for name in self.names:
            p = self.release / name; p.parent.mkdir(parents=True, exist_ok=True); p.write_bytes(b'unsigned fixture ' + name.encode())
        if self.mode != rc.D32:
            rc.capture_unsigned(self.release, [self.release / p for p in self.names])
            for name in self.names: (self.release / name).write_bytes(b'signed fixture ' + name.encode())
        smoke = self.release / 'acceptance/care_smoke.exe'; smoke.parent.mkdir(); smoke.write_bytes(b'exact smoke bytes')
        (smoke.parent / 'p87-installed-acceptance.ps1').write_bytes(b'exact acceptance script')
        signing = ({'signing_required':False,'signing_decision':'D32','signer_thumbprint':None} if self.mode == rc.D32
                   else {'signing_required':True,'signer_thumbprint':THUMB})
        rc.write_json(self.release / 'RELEASE-METADATA.json', dict({'version':self.version,'source_commit':self.sha,'dependency_baseline_approved':True}, **signing))
        paths = self.names + ['acceptance/care_smoke.exe','acceptance/p87-installed-acceptance.ps1','RELEASE-METADATA.json']
        (self.release / 'SHA256SUMS.txt').write_text(''.join(rc.digest(self.release / p) + '  ' + p + '\n' for p in paths))
        rc.create(self.release, self.source, self.sha, self.mode)
        self.receipt = rc.digest(self.release / 'RC-PROVENANCE.json')
        self.bundle = rc.digest(self.release / f'artifacts/AetherCoreSetup-{self.version}-x64.exe')
        self.acceptance = base / 'acceptance.json'
        self.lifecycle = base / 'lifecycle.json'
        rc.write_json(self.lifecycle, {'schema':'aethercore.ga-installer-lifecycle.v1','host':'disposable-vm','ok':True,'version':self.version,'source_commit':self.sha,'bundle_sha256':self.bundle,'previous_source_commit':'f'*40,'previous_bundle_sha256':'f'*64,
            'steps':[{'name':name,'ok':True} for name in rc.LIFECYCLE]})
        self.installed = []
        for locale in ['en','ar']:
            (base / (locale+'.witness.txt')).write_text('controlled portable witness')
            path=base / ('installed-' + locale + '.json'); self.installed.append(path)
            surfaces = {}
            for surface, page in SURFACE_PAGES.items():
                witnesses = []
                for role in ('runtime','accessibility','screenshot'):
                    file=base / f'{locale}.{surface}.{role}'
                    if role == 'runtime':
                        rc.write_json(file, {'tauri':True,'locale':locale,'page':page,'selector':SURFACE_SELECTORS[surface], 'text':'Controlled portable rendered surface', 'service':{'connected':True}})
                    elif role == 'accessibility':
                        rc.write_json(file, {'nodes':[{'nodeId':'1','role':{'type':'role','value':'heading'},'name':{'type':'string','value':'Portable surface'}}]})
                    else:
                        # Format control only: never installed/UI qualification.
                        file.write_bytes(b'\x89PNG\r\n\x1a\ncontrolled portable fixture')
                    witnesses.append({'role':role,'path':file.name,'sha256':rc.digest(file)})
                surfaces[surface] = witnesses
            rc.write_json(path, {'schema':'aethercore.p87-installed-acceptance.v1','source_commit':self.sha,'bundle_sha256':self.bundle,
                'locale':locale,'host':'disposable-vm','ok':True,'desktop_closed':True,'restart_pending':False,'care_run_id':'care-1791560738005',
                'surface_witnesses':surfaces,
                'read_only':False,'worker_ownership_released':True,'windows_build':26100,'windows_product_name':'Windows 11 Pro','product_type':'workstation','ordinary_user':True,'token':{'sid':'S-1-5-21-1-2-3-1001','elevated':False},
                'cases':[{'id':name,'disposition':'passed','checks':{'terminal':True,'progress':True,'unavailable_provider':True,'reconnect':True,'restart':True,'no_op_explained':True},'witnesses':[{'path':locale+'.witness.txt','sha256':rc.digest(base / (locale+'.witness.txt'))}]} for name in rc.SYMPTOMS]})
        rc.write_json(self.acceptance, {'schema':'aethercore.rc-acceptance.v1','source_commit':self.sha,'bundle_sha256':self.bundle,'provenance_sha256':self.receipt,'ok':True,
            'evidence':{path.name:rc.digest(path) for path in [self.lifecycle]+self.installed}})
        self.acceptance_hash = rc.digest(self.acceptance)
    def verify(self):
        return rc.verify(self.release, self.source, self.sha, self.mode, self.receipt, self.bundle)
    def promote(self):
        return rc.promote(self.release, self.source, self.sha, self.mode, self.receipt, self.bundle, self.acceptance, self.acceptance_hash)
    def test_installed_selectors_match_actual_surface_source(self):
        files = {'hardware':'features/diagnostics/HardwarePage.svelte',
                 'deep-scan':'features/intelligence/DeepScanPage.svelte',
                 'repair':'app/AppShell.svelte', 'care':'features/care/CarePanel.svelte',
                 'timeline':'features/timeline/TimelinePage.svelte',
                 'assistant':'features/assistant/AssistantDrawer.svelte'}
        for surface, file in files.items():
            text=(SCRIPTS.parent / 'apps/ui/src' / file).read_text(encoding='utf-8')
            selector=rc.SURFACE_SELECTORS[surface]
            if selector.startswith('.'):
                classes={c for group in re.findall(r'class="([^"{}]+)"',text) for c in group.split()}
                self.assertIn(selector[1:],classes,surface)
            else:
                self.assertRegex(text,rf'<{re.escape(selector)}\b')
    def test_exact_signed_bytes_can_promote(self):
        self.assertEqual(self.promote()['bundle_sha256'], self.bundle)
    def test_missing_receipt_blocks(self):
        (self.release / 'RC-PROVENANCE.json').unlink()
        with self.assertRaisesRegex(rc.Rejected, 'missing'): self.promote()
    def test_other_source_sha_blocks(self):
        with self.assertRaisesRegex(rc.Rejected, 'source SHA'): rc.verify(self.release,self.source,'0'*40,self.mode,self.receipt,self.bundle)
    def test_bundle_substitution_after_acceptance_blocks(self):
        (self.release / f'artifacts/AetherCoreSetup-{self.version}-x64.exe').write_bytes(b'substitution')
        with self.assertRaisesRegex(rc.Rejected, 'hash'): self.promote()
    def test_invalid_signature_blocks(self):
        with patch.object(rc,'signature',return_value={'status':'NotSigned','thumbprint':None}):
            with self.assertRaisesRegex(rc.Rejected,'signature'): self.promote()
    def test_other_valid_signer_blocks(self):
        with patch.object(rc,'signature',return_value={'status':'Valid','thumbprint':'B'*40}):
            with self.assertRaisesRegex(rc.Rejected,'signer'): self.promote()
    def test_receipt_rewrite_blocks(self):
        doc = rc.read_json(self.release / 'RC-PROVENANCE.json'); doc['source_commit']='0'*40
        rc.write_json(self.release / 'RC-PROVENANCE.json',doc)
        with self.assertRaisesRegex(rc.Rejected,'receipt hash'): self.promote()
    def test_omitted_unsigned_transition_blocks(self):
        doc=rc.read_json(self.release / 'RC-PROVENANCE.json');doc['artifacts'][0].pop('unsigned_sha256')
        rc.write_json(self.release / 'RC-PROVENANCE.json',doc);self.receipt=rc.digest(self.release / 'RC-PROVENANCE.json')
        with self.assertRaisesRegex(rc.Rejected,'unsigned'): self.verify()
    def test_frozen_input_substitution_blocks(self):
        original=(self.source / 'Cargo.lock').read_bytes()
        self.addCleanup((self.source / 'Cargo.lock').write_bytes,original)
        (self.source / 'Cargo.lock').write_text('changed dependency')
        with self.assertRaises(rc.Rejected): self.promote()
    def test_smoke_substitution_blocks(self):
        (self.release / 'acceptance/care_smoke.exe').write_bytes(b'other smoke binary')
        with self.assertRaisesRegex(rc.Rejected,'hash'): self.promote()
    def test_acceptance_from_other_bundle_blocks(self):
        doc=rc.read_json(self.acceptance);doc['bundle_sha256']='0'*64;rc.write_json(self.acceptance,doc);self.acceptance_hash=rc.digest(self.acceptance)
        with self.assertRaisesRegex(rc.Rejected,'acceptance'): self.promote()
    def test_acceptance_receipt_substitution_blocks(self):
        self.acceptance.write_text('{}')
        with self.assertRaisesRegex(rc.Rejected,'acceptance hash'): self.promote()
    def test_omitted_installed_receipt_blocks(self):
        doc=rc.read_json(self.acceptance);doc['evidence'].pop('installed-ar.json');rc.write_json(self.acceptance,doc);self.acceptance_hash=rc.digest(self.acceptance)
        with self.assertRaisesRegex(rc.Rejected,'evidence'): self.promote()
    def test_skipped_symptom_blocks(self):
        doc=rc.read_json(self.installed[0]);doc['cases'][0]['disposition']='skipped';rc.write_json(self.installed[0],doc)
        accepted=rc.read_json(self.acceptance);accepted['evidence']['installed-en.json']=rc.digest(self.installed[0]);rc.write_json(self.acceptance,accepted);self.acceptance_hash=rc.digest(self.acceptance)
        with self.assertRaisesRegex(rc.Rejected,'symptom'): self.promote()
    def test_windows_server_is_not_windows11_qualification(self):
        doc=rc.read_json(self.installed[0]);doc['windows_product_name']='Windows Server 2025';doc['product_type']='server';rc.write_json(self.installed[0],doc)
        accepted=rc.read_json(self.acceptance);accepted['evidence']['installed-en.json']=rc.digest(self.installed[0]);rc.write_json(self.acceptance,accepted);self.acceptance_hash=rc.digest(self.acceptance)
        with self.assertRaisesRegex(rc.Rejected,'Windows 11'): self.promote()
    def test_lifecycle_failure_blocks(self):
        doc=rc.read_json(self.lifecycle);doc['steps'][0]['ok']=False;rc.write_json(self.lifecycle,doc)
        accepted=rc.read_json(self.acceptance);accepted['evidence']['lifecycle.json']=rc.digest(self.lifecycle);rc.write_json(self.acceptance,accepted);self.acceptance_hash=rc.digest(self.acceptance)
        with self.assertRaisesRegex(rc.Rejected,'lifecycle'): self.promote()
    def test_ordinary_user_proof_is_required(self):
        doc=rc.read_json(self.installed[0]);doc['token']['elevated']=True;rc.write_json(self.installed[0],doc)
        accepted=rc.read_json(self.acceptance);accepted['evidence']['installed-en.json']=rc.digest(self.installed[0]);rc.write_json(self.acceptance,accepted);self.acceptance_hash=rc.digest(self.acceptance)
        with self.assertRaisesRegex(rc.Rejected,'ordinary-user'): self.promote()
    def test_raw_witness_substitution_blocks(self):
        (self.acceptance.parent / 'en.witness.txt').write_text('substituted screen')
        with self.assertRaisesRegex(rc.Rejected,'witness hash'): self.promote()
    def test_accept_producer_checks_actual_evidence(self):
        doc=rc.accept(self.release,self.source,self.sha,self.mode,self.receipt,self.bundle,self.acceptance.parent)
        self.assertTrue(doc['ok'])
        self.installed[1].unlink()
        with self.assertRaisesRegex(rc.Rejected,'missing'): rc.accept(self.release,self.source,self.sha,self.mode,self.receipt,self.bundle,self.acceptance.parent)
    def test_real_native_unsigned_artifact_has_no_fixture_signature_bypass(self):
        if sys.platform != 'win32': self.skipTest('native Authenticode API only')
        self.signatures.stop()
        observed=rc.signature(self.release / self.names[0])
        self.assertIn('status',observed)
        self.assertNotEqual(observed['status'],'Valid')
        with self.assertRaisesRegex(rc.Rejected,'signature'): self.verify()
    def missing_runtime_check(self,case_id,key):
        doc=rc.read_json(self.installed[0])
        next(p for p in doc['cases'] if p['id'] == case_id)['checks'][key]=False
        rc.write_json(self.installed[0],doc)
        accepted=rc.read_json(self.acceptance);accepted['evidence']['installed-en.json']=rc.digest(self.installed[0]);rc.write_json(self.acceptance,accepted);self.acceptance_hash=rc.digest(self.acceptance)
        with self.assertRaisesRegex(rc.Rejected,'runtime check'): self.promote()
    def test_missing_unavailable_provider_proof_blocks(self):
        self.missing_runtime_check('p76-repair-assessment-terminal','unavailable_provider')
    def test_missing_assessment_progress_proof_blocks(self):
        self.missing_runtime_check('p76-repair-assessment-terminal','progress')
    def test_failed_or_incomplete_installed_receipt_does_not_promote(self):
        original=rc.read_json(self.installed[0])
        for fields in ({'ok':False},{'ok':None},{'blocked_reason':'Restart verification failed'},
                       {'desktop_closed':False},{'restart_pending':True},{'care_run_id':'invalid'},
                       {'care_run_id':'care-'},{'care_run_id':'CARE-1791560738005'},
                       {'care_run_id':'11111111-2222-4333-8444-555555555555'}):
            with self.subTest(fields=fields):
                rc.write_json(self.installed[0],dict(original,**fields))
                accepted=rc.read_json(self.acceptance);accepted['evidence']['installed-en.json']=rc.digest(self.installed[0])
                rc.write_json(self.acceptance,accepted);self.acceptance_hash=rc.digest(self.acceptance)
                with self.assertRaisesRegex(rc.Rejected,'installed acceptance'): self.promote()
    def test_missing_care_restart_proof_blocks(self):
        self.missing_runtime_check('p76-care-timeline-persistence','restart')
    def test_missing_noop_explanation_blocks(self):
        self.missing_runtime_check('p76-care-eligibility-explanation','no_op_explained')
    def test_active_nested_worker_does_not_promote(self):
        doc=rc.read_json(self.installed[0]);doc['worker_ownership_released']=False;rc.write_json(self.installed[0],doc)
        accepted=rc.read_json(self.acceptance);accepted['evidence']['installed-en.json']=rc.digest(self.installed[0]);rc.write_json(self.acceptance,accepted);self.acceptance_hash=rc.digest(self.acceptance)
        with self.assertRaisesRegex(rc.Rejected,'worker ownership'): self.promote()
    def test_readonly_owner_observation_does_not_promote(self):
        doc=rc.read_json(self.installed[0]);doc['read_only']=True;rc.write_json(self.installed[0],doc)
        accepted=rc.read_json(self.acceptance);accepted['evidence']['installed-en.json']=rc.digest(self.installed[0]);rc.write_json(self.acceptance,accepted);self.acceptance_hash=rc.digest(self.acceptance)
        with self.assertRaisesRegex(rc.Rejected,'read-only'): self.promote()
    def test_unsigned_platform_has_no_signature_fallback(self):
        self.signatures.stop()
        if sys.platform != 'win32':
            with self.assertRaisesRegex(rc.Rejected,'Windows'): self.verify()

    def update_installed(self, doc):
        rc.write_json(self.installed[0],doc)
        accepted=rc.read_json(self.acceptance)
        accepted['evidence']['installed-en.json']=rc.digest(self.installed[0])
        rc.write_json(self.acceptance,accepted); self.acceptance_hash=rc.digest(self.acceptance)

    def test_missing_surface_map_blocks(self):
        doc=rc.read_json(self.installed[0]); del doc['surface_witnesses'];self.update_installed(doc)
        with self.assertRaisesRegex(rc.Rejected,'surface'): self.promote()

    def test_each_required_installed_surface_blocks_when_omitted(self):
        original=rc.read_json(self.installed[0])
        for surface in SURFACE_PAGES:
            with self.subTest(surface=surface):
                doc=json.loads(json.dumps(original));del doc['surface_witnesses'][surface];self.update_installed(doc)
                with self.assertRaisesRegex(rc.Rejected,'surface'): self.promote()

    def test_missing_or_duplicate_surface_roles_block(self):
        original=rc.read_json(self.installed[0])
        for role in ('runtime','accessibility','screenshot'):
            for duplicate in (False,True):
                with self.subTest(role=role,duplicate=duplicate):
                    doc=json.loads(json.dumps(original));w=doc['surface_witnesses']['assistant']
                    if duplicate: next(p for p in w if p['role']==role)['role']=next(p['role'] for p in w if p['role']!=role)
                    else: w[:]=[p for p in w if p['role']!=role]
                    self.update_installed(doc)
                    with self.assertRaisesRegex(rc.Rejected,'surface'): self.promote()

    def test_surface_runtime_must_be_actual_locale_page_and_connected(self):
        original=rc.read_json(self.installed[0]);w=original['surface_witnesses']['assistant'][0]
        file=self.acceptance.parent / w['path'];runtime=rc.read_json(file)
        for fields in ({'tauri':False},{'locale':'ar'},{'page':'overview'},{'text':''},{'service':{'connected':False}}):
            with self.subTest(fields=fields):
                rc.write_json(file,dict(runtime,**fields))
                doc=json.loads(json.dumps(original));doc['surface_witnesses']['assistant'][0]['sha256']=rc.digest(file);self.update_installed(doc)
                with self.assertRaisesRegex(rc.Rejected,'surface runtime'): self.promote()

    def test_surface_accessibility_requires_actual_nodes(self):
        original=rc.read_json(self.installed[0]);file=self.acceptance.parent / original['surface_witnesses']['timeline'][1]['path']
        for nodes in ([], 'fixture', [{}]):
            with self.subTest(nodes=nodes):
                rc.write_json(file,{'nodes':nodes});doc=json.loads(json.dumps(original))
                doc['surface_witnesses']['timeline'][1]['sha256']=rc.digest(file);self.update_installed(doc)
                with self.assertRaisesRegex(rc.Rejected,'surface accessibility'): self.promote()

    def test_text_file_cannot_qualify_as_surface_screenshot(self):
        doc=rc.read_json(self.installed[0]);w=doc['surface_witnesses']['deep-scan'][2]
        file=self.acceptance.parent / w['path'];file.write_bytes(b'This is a screenshot')
        w['sha256']=rc.digest(file);self.update_installed(doc)
        with self.assertRaisesRegex(rc.Rejected,'surface screenshot'): self.promote()

    def test_surface_witness_substitution_blocks(self):
        doc=rc.read_json(self.installed[0]);w=doc['surface_witnesses']['hardware'][0]
        (self.acceptance.parent / w['path']).write_bytes(b'replaced')
        with self.assertRaisesRegex(rc.Rejected,'surface witness'): self.promote()

    def test_care_capture_cannot_substitute_for_timeline_or_assistant(self):
        original=rc.read_json(self.installed[0])
        for surface in ('timeline','assistant'):
            with self.subTest(surface=surface):
                doc=json.loads(json.dumps(original));doc['surface_witnesses'][surface]=doc['surface_witnesses']['care']
                self.update_installed(doc)
                with self.assertRaisesRegex(rc.Rejected,'surface runtime'): self.promote()

    def set_decisions(self, text):
        path=self.source / 'docs/roadmap/DECISIONS.md';original=path.read_bytes()
        self.addCleanup(path.write_bytes,original);path.write_text(text)

    def owner_host(self, **overrides):
        doc=rc.read_json(self.lifecycle)
        doc.update({'host':'owner-host-d33','service_running_at_end':True,'owner_data_restored':True,
                    'owner_backup':{'directory':'C:\\ProgramData\\AetherCore-owner-backup-20261004T000000Z','manifest_sha256':'c'*64,'files':3},
                    'steps':[{'name':n,'ok':True} for n in rc.LIFECYCLE + rc.OWNER_HOST_STEPS]})
        doc.update(overrides);rc.write_json(self.lifecycle,doc)
        for path in self.installed:
            installed=rc.read_json(path);installed['host']='owner-host-d33';rc.write_json(path,installed)
        accepted=rc.read_json(self.acceptance)
        accepted['evidence']={p.name:rc.digest(p) for p in [self.lifecycle]+self.installed}
        rc.write_json(self.acceptance,accepted);self.acceptance_hash=rc.digest(self.acceptance)

    def test_owner_host_run_with_backup_and_running_service_can_promote(self):
        self.owner_host()
        self.assertTrue(self.promote()['rc_eligible'])

    def test_owner_host_requires_recorded_d33(self):
        self.owner_host();self.set_decisions('| D32 | unsigned |\n')
        # Called directly: through promote() the edited sealed source is already rejected as dirty.
        accepted=rc.read_json(self.acceptance)
        with self.assertRaisesRegex(rc.Rejected,'decision D33'):
            rc.validate_evidence(self.acceptance.parent,accepted['evidence'],self.sha,self.bundle,self.version,self.source)
        with self.assertRaises(rc.Rejected): self.promote()

    def test_owner_host_requires_backup_running_service_and_its_steps(self):
        for overrides,message in (({'owner_backup':{}},'backup'),({'service_running_at_end':False},'running again'),
                                  ({'owner_data_restored':False},'data was not restored'),
                                  ({'steps':[{'name':n,'ok':True} for n in rc.LIFECYCLE]},'lifecycle required steps')):
            with self.subTest(overrides=list(overrides)):
                self.owner_host(**overrides)
                with self.assertRaisesRegex(rc.Rejected,message): self.promote()

    def d34_repair(self, marker='D34'):
        # D33 run 6: a healthy owner PC has no unavailable provider to observe (owner decision D34).
        for path in self.installed:
            doc=rc.read_json(path);case=next(p for p in doc['cases'] if p['id'] == 'p76-repair-assessment-terminal')
            case['checks']['unavailable_provider']=False
            case['checks'].pop('unavailable_provider_owner_decision',None)
            if marker is not None: case['checks']['unavailable_provider_owner_decision']=marker
            rc.write_json(path,doc)
        accepted=rc.read_json(self.acceptance)
        accepted['evidence'].update({p.name:rc.digest(p) for p in self.installed})
        rc.write_json(self.acceptance,accepted);self.acceptance_hash=rc.digest(self.acceptance)

    def test_owner_host_d34_promotes_and_names_what_was_not_observed(self):
        self.owner_host();self.d34_repair()
        promoted=self.promote()
        self.assertTrue(promoted['rc_eligible'])
        self.assertEqual(promoted['not_observed'],[{'locale':l,'case':'p76-repair-assessment-terminal','check':'unavailable_provider','decision':'D34'} for l in ('en','ar')])

    def test_full_evidence_reports_nothing_unobserved(self):
        self.assertEqual(self.promote()['not_observed'],[])

    def test_d34_does_not_apply_off_the_owner_host_or_without_its_marker(self):
        self.d34_repair()
        with self.assertRaisesRegex(rc.Rejected,'runtime check'): self.promote()
        for marker in (None,'D33',True):
            with self.subTest(marker=marker):
                self.owner_host();self.d34_repair(marker)
                with self.assertRaisesRegex(rc.Rejected,'runtime check'): self.promote()

    def test_owner_host_d34_requires_recorded_decision(self):
        self.owner_host();self.d34_repair();self.set_decisions('| D32 | unsigned |\n| D33 | owner host |\n')
        accepted=rc.read_json(self.acceptance)
        with self.assertRaisesRegex(rc.Rejected,'decision D34'):
            rc.validate_evidence(self.acceptance.parent,accepted['evidence'],self.sha,self.bundle,self.version,self.source)

    def test_lifecycle_host_must_be_declared_and_match_installed_receipts(self):
        doc=rc.read_json(self.lifecycle);doc.pop('host');rc.write_json(self.lifecycle,doc)
        accepted=rc.read_json(self.acceptance);accepted['evidence']['lifecycle.json']=rc.digest(self.lifecycle);rc.write_json(self.acceptance,accepted);self.acceptance_hash=rc.digest(self.acceptance)
        with self.assertRaisesRegex(rc.Rejected,'host is not declared'): self.promote()
        self.owner_host();installed=rc.read_json(self.installed[0]);installed['host']='disposable-vm';self.update_installed(installed)
        with self.assertRaisesRegex(rc.Rejected,'identity'): self.promote()

class UnsignedD32PromotionTests(PromotionTests):
    """Every inherited evidence control, run again over a D32 unsigned RC."""
    mode = rc.D32
    def test_exact_signed_bytes_can_promote(self):
        promoted=self.promote()
        self.assertEqual(promoted['signing'],rc.D32);self.assertFalse(promoted['ga'])
        self.assertIsNone(rc.read_json(self.release / 'RC-PROVENANCE.json')['signer_thumbprint'])
    def test_invalid_signature_blocks(self):
        # D32 never claims signed: a signed or invalid file is not a D32 unsigned artifact.
        for status in ('Valid','HashMismatch'):
            with self.subTest(status=status), patch.object(rc,'signature',return_value={'status':status,'thumbprint':THUMB}):
                with self.assertRaisesRegex(rc.Rejected,'not unsigned'): self.promote()
    def test_other_valid_signer_blocks(self):
        self.skipTest('D32 has no signer; covered by test_invalid_signature_blocks')
    def test_omitted_unsigned_transition_blocks(self):
        doc=rc.read_json(self.release / 'RC-PROVENANCE.json');doc['artifacts'][0]['signed_sha256']=doc['artifacts'][0]['sha256']
        rc.write_json(self.release / 'RC-PROVENANCE.json',doc);self.receipt=rc.digest(self.release / 'RC-PROVENANCE.json')
        with self.assertRaisesRegex(rc.Rejected,'D32 artifact hash'): self.verify()
    def test_real_native_unsigned_artifact_has_no_fixture_signature_bypass(self):
        if sys.platform != 'win32': self.skipTest('native Authenticode API only')
        self.signatures.stop()
        # The fixture bytes are not a PE, so Windows reports UnknownError and D32 must reject them.
        with self.assertRaisesRegex(rc.Rejected,'not unsigned'): self.verify()
        # A real unsigned PE is what D32 accepts: build one; this control must not silently skip.
        compiler=shutil.which('rustc')
        self.assertIsNotNone(compiler,'rustc is required for the native unsigned-PE control')
        source=self.release / 'unsigned.rs'; source.write_text('fn main() {}')
        binary=self.release / 'unsigned.exe'
        subprocess.run([compiler,str(source),'-o',str(binary)],check=True,capture_output=True,timeout=120)
        self.assertEqual(rc.signature(binary)['status'],'NotSigned')
    def test_d32_requires_the_recorded_owner_decision(self):
        self.set_decisions('| D31 | other |\n')
        with self.assertRaisesRegex(rc.Rejected,'decision D32'): self.verify()
        with self.assertRaisesRegex(rc.Rejected,'decision D32'): self.promote()
    def test_signed_receipt_cannot_be_verified_as_d32_or_back(self):
        doc=rc.read_json(self.release / 'RC-PROVENANCE.json');doc['signing']='authenticode'
        rc.write_json(self.release / 'RC-PROVENANCE.json',doc);self.receipt=rc.digest(self.release / 'RC-PROVENANCE.json')
        with self.assertRaisesRegex(rc.Rejected,'signer'): self.verify()
        with self.assertRaisesRegex(rc.Rejected,'signer'): rc.verify(self.release,self.source,self.sha,THUMB,self.receipt,self.bundle)
    def test_metadata_must_name_d32(self):
        meta=rc.read_json(self.release / 'RELEASE-METADATA.json');meta['signing_decision']=None
        rc.write_json(self.release / 'RELEASE-METADATA.json',meta)
        with self.assertRaises(rc.Rejected): self.verify()

class SigningProtectionTests(unittest.TestCase):
    def setUp(self):
        spec=importlib.util.spec_from_file_location('rc_preflight', SCRIPTS / 'rc-signing-preflight.py')
        self.preflight=importlib.util.module_from_spec(spec);spec.loader.exec_module(self.preflight)
    def test_missing_reviewer_blocks(self):
        with self.assertRaisesRegex(self.preflight.Blocked,'reviewer'): self.preflight.check({'protection_rules':[]})
    def test_missing_ref_policy_blocks(self):
        with self.assertRaisesRegex(self.preflight.Blocked,'branch/tag'): self.preflight.check({'protection_rules':[{'type':'required_reviewers','reviewers':[{'id':1}]}]})
    def test_configured_review_and_refs_are_required(self):
        self.preflight.check({'protection_rules':[{'type':'required_reviewers','reviewers':[{'id':1}]}], 'deployment_branch_policy':{'protected_branches':True}})

if __name__ == '__main__': unittest.main()
