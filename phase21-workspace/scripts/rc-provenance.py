#!/usr/bin/env python3
"""Same-byte RC acceptance/promotion gate. No signing, rebuilding or publishing.

An RC is either Authenticode-signed by the pinned signer, or unsigned under owner decision D32
(`--unsigned-d32`), whose bytes must really be unsigned so no receipt can be read as a signature.

Receipt hashes come from the protected signing job; acceptance hashes come from
its dependent acceptance job. Neither is read back from the untrusted package.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path, PurePosixPath

sys.dont_write_bytecode = True
from source_seal import verify as verify_seal

INPUTS = ('MANIFEST.sha256', '../.github/MANIFEST.sha256', 'Cargo.lock', 'pnpm-lock.yaml',
          'release/dependency-locks.sha256', 'release/dependency-manifests.sha256', 'release/dependency-freeze.json')
SYMPTOMS = ('p76-hardware-owned-text','p76-performance-provider-labels','p76-cleanup-owned-text',
            'p76-care-timeline-persistence','p76-repair-assessment-terminal','p76-care-eligibility-explanation')
LIFECYCLE = ('burn-install','installed-security-boundaries','program-data-preservation-sentinel',
             'repair-closes-acl-drift','burn-uninstall','uninstall-clean-state','signed-upgrade-preserves-owner-data')
# D33: the owner's own PC. Its data is backed up and its prior install removed before, and the service runs after.
OWNER_HOST_STEPS = ('owner-host-backup','owner-host-prior-uninstall','owner-host-service-restored')
HOSTS = ('disposable-vm','owner-host-d33')
D32 = 'unsigned by owner decision D32'
PAYLOAD = ('aethercore-desktop', 'aethercore-maintenance-service', 'aethercore-consent-broker',
           'aethercore-update-broker', 'aethercore-install-hardener', 'aetherctl')
SURFACE_PAGES = {'hardware':'hardware','deep-scan':'deepScan','repair':'repair',
                 'care':'activity','timeline':'activity','assistant':'activity'}
SURFACE_SELECTORS = {'hardware':'.storage-grid','deep-scan':'.deep-scan-header','repair':'main',
                     'care':'.care-panel','timeline':'.timeline-panel','assistant':'.assistant-drawer'}

class Rejected(ValueError): pass

def require(value, message):
    if not value: raise Rejected(message)

def digest(path):
    require(path.is_file(), f'missing file: {path.name}')
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''): h.update(block)
    return h.hexdigest()

def read_json(path):
    require(path.is_file(), f'missing receipt/evidence: {path.name}')
    return json.loads(path.read_text(encoding='utf-8-sig'))

def write_json(path, document):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(document, sort_keys=True, indent=2) + '\n', encoding='utf-8')

def safe_path(root, name):
    require(isinstance(name, str) and name and '\\' not in name and ':' not in name and
            not PurePosixPath(name).is_absolute() and all(p not in ('', '.', '..') for p in name.split('/')), 'unsafe artifact path')
    path = root / name
    require(path.resolve().is_relative_to(root.resolve()) and not path.is_symlink(), 'artifact path escapes root')
    return path

def hash_lines(root, path):
    entries = {}
    for line in path.read_text(encoding='utf-8-sig').splitlines():
        if not line.strip(): continue
        match = re.fullmatch(r'([0-9a-fA-F]{64})  (.+)', line)
        require(match, 'malformed hash inventory')
        expected, name = match.groups()
        require(name not in entries, 'duplicate hash inventory entry')
        require(digest(safe_path(root, name)) == expected.lower(), f'hash mismatch: {name}')
        entries[name] = expected.lower()
    require(entries, 'empty hash inventory')
    return entries

def owner_decision(source, ident):
    text = (source / 'docs/roadmap/DECISIONS.md').read_text(encoding='utf-8')
    require(re.search(rf'^\| {ident} \|', text, re.M), f'owner decision {ident} is not recorded')

def signed_paths(version):
    require(re.fullmatch(r'\d+\.\d+\.\d+', version), 'invalid release version')
    return [f'payload/{name}.exe' for name in PAYLOAD] + [f'artifacts/AetherCore-{version}-x64.msi', f'artifacts/AetherCoreSetup-{version}-x64.exe']

def snapshot(source, expected_sha):
    actual = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=source, text=True).strip()
    require(actual == expected_sha and re.fullmatch(r'[0-9a-f]{40,64}', expected_sha), 'source SHA mismatch')
    dirty = subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=no'], cwd=source, text=True)
    require(not dirty, 'tracked source is dirty')
    require(verify_seal(source)['ok'] and verify_seal(source.parent / '.github')['ok'], 'source seal failed')
    require(not (source / 'release/dependency-freeze.blocker.json').exists(), 'dependency freeze blocked')
    for name in ('release/dependency-locks.sha256', 'release/dependency-manifests.sha256'): hash_lines(source, source / name)
    frozen = read_json(source / 'release/dependency-freeze.json')
    require(frozen.get('schema') == 'aethercore.dependency-freeze.v1', 'freeze schema mismatch')
    for key, name in (('cargo_lock_sha256','Cargo.lock'), ('pnpm_lock_sha256','pnpm-lock.yaml'),
                      ('manifest_baseline_sha256','release/dependency-manifests.sha256'), ('lock_baseline_sha256','release/dependency-locks.sha256')):
        require(frozen.get(key) == digest(source / name), f'freeze input mismatch: {name}')
    return {name:digest(source / name) for name in INPUTS}

def signature(path):
    require(sys.platform == 'win32', 'Authenticode verification requires Windows; no unsigned fallback')
    script = "$ErrorActionPreference='Stop'; $s=Get-AuthenticodeSignature -FilePath $env:RC_SIGNATURE_PATH; @{status=$s.Status.ToString();thumbprint=$s.SignerCertificate.Thumbprint}|ConvertTo-Json -Compress"
    env = dict(os.environ, RC_SIGNATURE_PATH=str(path.resolve()))
    # Release jobs run in pwsh. A nested Windows PowerShell 5 host can inherit
    # incompatible pwsh module paths; use the already required pwsh when present.
    host=shutil.which('pwsh') or 'powershell.exe'
    try:
        result = subprocess.run([host, '-NoProfile', '-NonInteractive', '-Command', script], env=env, capture_output=True, text=True, check=True)
        return json.loads(result.stdout)
    except (subprocess.SubprocessError,ValueError) as error:
        raise Rejected('native signature verification failed') from error

def assert_signed(path, thumb):
    if thumb == D32:
        require(signature(path).get('status') == 'NotSigned', f'D32 artifact is not unsigned: {path.name}')
        return
    require(isinstance(thumb, str) and re.fullmatch(r'[0-9A-F]{40}', thumb), 'configured signer is missing/invalid')
    observed = signature(path)
    require(observed.get('status') == 'Valid', f'invalid signature: {path.name}')
    require((observed.get('thumbprint') or '').upper() == thumb, f'wrong signer: {path.name}')

def capture_unsigned(release, paths):
    file = release / 'RC-UNSIGNED.json'
    record = read_json(file) if file.exists() else {}
    require(paths, 'unsigned signing inputs omitted')
    for path in paths:
        name = path.resolve().relative_to(release.resolve()).as_posix()
        require(name not in record, 'unsigned transition already captured')
        record[name] = digest(safe_path(release, name))
    write_json(file, record)

def signing_metadata(meta, thumb):
    if thumb == D32:
        require(meta.get('signing_required') is False and meta.get('signing_decision') == 'D32' and not meta.get('signer_thumbprint'), 'metadata is not a D32 unsigned RC')
    else:
        require(meta.get('signing_required') is True, 'unsigned metadata without owner decision D32')

def create(release, source, sha, thumb):
    if thumb == D32: owner_decision(source, 'D32')
    meta = read_json(release / 'RELEASE-METADATA.json')
    require(meta.get('source_commit') == sha and meta.get('dependency_baseline_approved') is True, 'wrong-source metadata')
    signing_metadata(meta, thumb)
    if thumb != D32: require((meta.get('signer_thumbprint') or '').upper() == thumb, 'metadata signer mismatch')
    names = signed_paths(meta['version'])
    unsigned = read_json(release / 'RC-UNSIGNED.json') if thumb != D32 else None
    require(thumb == D32 or set(unsigned) == set(names), 'omitted/extra unsigned signing transition')
    inventory = hash_lines(release, release / 'SHA256SUMS.txt')
    require(set(names + ['acceptance/care_smoke.exe','acceptance/p87-installed-acceptance.ps1','RELEASE-METADATA.json']).issubset(inventory), 'required RC bytes omitted from inventory')
    require(digest(release / 'acceptance/p87-installed-acceptance.ps1') == digest(source / 'scripts/p87-installed-acceptance.ps1'), 'acceptance script differs from sealed source')
    artifacts = []
    for name in names:
        path = safe_path(release, name); assert_signed(path, thumb)
        if thumb == D32:
            artifacts.append({'path':name, 'sha256':digest(path)}); continue
        require(re.fullmatch(r'[0-9a-f]{64}', unsigned[name]) and unsigned[name] != digest(path), 'invalid unsigned signing transition')
        artifacts.append({'path':name, 'unsigned_sha256':unsigned[name], 'signed_sha256':digest(path)})
    receipt = {'schema':'aethercore.rc-provenance.v1', 'source_commit':sha, 'source_inputs':snapshot(source,sha),
               'signing':D32 if thumb == D32 else 'authenticode',
               'signer_thumbprint':None if thumb == D32 else thumb, 'version':meta['version'], 'artifacts':artifacts,
               'acceptance_script_sha256':digest(source / 'scripts/p87-installed-acceptance.ps1'),
               'inventory_sha256':digest(release / 'SHA256SUMS.txt')}
    write_json(release / 'RC-PROVENANCE.json', receipt)
    if thumb != D32: (release / 'RC-UNSIGNED.json').unlink()
    return receipt

def verify(release, source, sha, thumb, receipt_hash, bundle_hash):
    if thumb == D32: owner_decision(source if source is not None else Path(__file__).resolve().parents[1], 'D32')
    require(digest(release / 'RC-PROVENANCE.json') == receipt_hash, 'receipt hash mismatch')
    receipt = read_json(release / 'RC-PROVENANCE.json')
    require(receipt.get('schema') == 'aethercore.rc-provenance.v1' and receipt.get('source_commit') == sha, 'receipt source SHA mismatch')
    if source is not None:
        require(receipt.get('source_inputs') == snapshot(source,sha), 'source/freeze snapshot mismatch')
    else:
        require(set(receipt.get('source_inputs',{})) == set(INPUTS) and all(re.fullmatch(r'[0-9a-f]{64}',v) for v in receipt['source_inputs'].values()), 'baseline source/freeze receipt omitted')
    require(receipt.get('signing') == (D32 if thumb == D32 else 'authenticode') and receipt.get('signer_thumbprint') == (None if thumb == D32 else thumb), 'receipt signer mismatch')
    require(digest(release / 'SHA256SUMS.txt') == receipt.get('inventory_sha256'), 'inventory hash mismatch')
    inventory = hash_lines(release, release / 'SHA256SUMS.txt')
    meta = read_json(release / 'RELEASE-METADATA.json')
    require(meta.get('source_commit') == sha and meta.get('dependency_baseline_approved') is True and meta.get('version') == receipt.get('version'), 'metadata source/signing mismatch')
    signing_metadata(meta, thumb)
    names = signed_paths(receipt['version'])
    artifacts = receipt.get('artifacts', [])
    require(len(artifacts) == len(names) and {p['path'] for p in artifacts} == set(names), 'required artifact signing receipts omitted/duplicated')
    require(set(names + ['acceptance/care_smoke.exe','acceptance/p87-installed-acceptance.ps1','RELEASE-METADATA.json']).issubset(inventory), 'required RC bytes omitted')
    require(inventory['acceptance/p87-installed-acceptance.ps1'] == receipt.get('acceptance_script_sha256'), 'acceptance script source hash mismatch')
    if source is not None:
        require(receipt['acceptance_script_sha256'] == digest(source / 'scripts/p87-installed-acceptance.ps1'), 'acceptance script differs from sealed source')
    for artifact in artifacts:
        name = artifact['path']; path = safe_path(release,name)
        if thumb == D32:
            require(set(artifact) == {'path','sha256'} and digest(path) == artifact['sha256'] == inventory[name], f'D32 artifact hash mismatch: {name}')
            assert_signed(path,thumb); continue
        unsigned = artifact.get('unsigned_sha256','')
        require(re.fullmatch(r'[0-9a-f]{64}',unsigned) and unsigned != artifact.get('signed_sha256'), 'omitted/invalid unsigned signing transition')
        require(digest(path) == artifact.get('signed_sha256') == inventory[name], f'signed artifact hash mismatch: {name}')
        assert_signed(path,thumb)
    require(digest(release / names[-1]) == bundle_hash, 'bundle hash mismatch')
    return receipt

def validate_surfaces(directory, doc, locale):
    surfaces=doc.get('surface_witnesses',{})
    require(isinstance(surfaces,dict) and set(surfaces) == set(SURFACE_PAGES), 'required installed surface omitted/extra')
    for surface,page in SURFACE_PAGES.items():
        witnesses=surfaces[surface]
        require(isinstance(witnesses,list) and len(witnesses) == 3
                and all(isinstance(w,dict) for w in witnesses)
                and {w.get('role') for w in witnesses} == {'runtime','accessibility','screenshot'}
                and len({w.get('path') for w in witnesses}) == 3, 'surface witness roles omitted/duplicated')
        for witness in witnesses:
            path=safe_path(directory,witness['path'])
            require(digest(path) == witness['sha256'], 'surface witness hash mismatch')
            if witness['role'] == 'runtime':
                runtime=read_json(path)
                require(isinstance(runtime,dict) and runtime.get('tauri') is True
                        and runtime.get('locale') == locale and runtime.get('page') == page
                        and runtime.get('selector') == SURFACE_SELECTORS[surface]
                        and isinstance(runtime.get('text'),str) and runtime['text'].strip()
                        and isinstance(runtime.get('service'),dict) and runtime['service'].get('connected') is True,
                        'surface runtime is absent/disconnected or belongs to another locale/page')
            elif witness['role'] == 'accessibility':
                ax=read_json(path);nodes=ax.get('nodes') if isinstance(ax,dict) else None
                require(isinstance(nodes,list) and nodes
                        and all(isinstance(n,dict) and isinstance(n.get('nodeId'),str) and n['nodeId'] for n in nodes)
                        and any(isinstance(n.get('role'),dict) and n['role'].get('value') for n in nodes),
                        'surface accessibility nodes omitted/malformed')
            else:
                with path.open('rb') as stream:
                    require(stream.read(8) == b'\x89PNG\r\n\x1a\n', 'surface screenshot is not PNG evidence')

def validate_evidence(directory, evidence, sha, bundle_hash, version, source):
    require(set(evidence) == {'lifecycle.json','installed-en.json','installed-ar.json'}, 'required installed/lifecycle evidence omitted')
    for name, expected in evidence.items():
        require(digest(safe_path(directory,name)) == expected, 'acceptance evidence hash mismatch')
    life=read_json(directory / 'lifecycle.json')
    require(life.get('schema') == 'aethercore.ga-installer-lifecycle.v1' and life.get('ok') is True and life.get('version') == version and life.get('source_commit') == sha and life.get('bundle_sha256') == bundle_hash, 'lifecycle evidence failed')
    require(re.fullmatch(r'[0-9a-f]{40,64}',life.get('previous_source_commit','')) and life['previous_source_commit'] != sha and re.fullmatch(r'[0-9a-f]{64}',life.get('previous_bundle_sha256','')) and life['previous_bundle_sha256'] != bundle_hash, 'signed upgrade baseline evidence omitted')
    host=life.get('host')
    require(host in HOSTS, 'lifecycle host is not declared')
    required=LIFECYCLE
    if host == 'owner-host-d33':
        owner_decision(source, 'D33')
        backup=life.get('owner_backup',{})
        require(isinstance(backup,dict) and isinstance(backup.get('directory'),str) and backup['directory']
                and re.fullmatch(r'[0-9a-f]{64}',backup.get('manifest_sha256','')) and isinstance(backup.get('files'),int),
                'owner-host backup evidence omitted')
        require(life.get('service_running_at_end') is True, 'owner-host service is not running again')
        require(life.get('owner_data_restored') is True, "owner-host data was not restored from the backup")
        required=LIFECYCLE + OWNER_HOST_STEPS
    steps=life.get('steps',[])
    require(len(steps) == len(required) and {p.get('name') for p in steps} == set(required) and all(p.get('ok') is True for p in steps), 'lifecycle required steps failed/omitted')
    for locale in ('en','ar'):
        doc=read_json(directory / ('installed-' + locale + '.json'))
        require(doc.get('schema') == 'aethercore.p87-installed-acceptance.v1' and doc.get('source_commit') == sha and doc.get('bundle_sha256') == bundle_hash and doc.get('locale') == locale and doc.get('host') == host, 'installed acceptance identity mismatch')
        require(doc.get('ok') is True and 'blocked_reason' not in doc and doc.get('desktop_closed') is True
                and doc.get('restart_pending') is False and re.fullmatch(r'[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}',doc.get('care_run_id','')),
                'installed acceptance failed or remains incomplete')
        require(doc.get('read_only') is False, 'read-only installed observation cannot qualify')
        require(doc.get('worker_ownership_released') is True, 'installed worker ownership was not released')
        require(isinstance(doc.get('windows_build'),int) and doc['windows_build'] >= 22000 and doc.get('product_type') == 'workstation' and 'Windows 11' in doc.get('windows_product_name',''), 'Windows 11 workstation acceptance required')
        token=doc.get('token',{})
        require(doc.get('ordinary_user') is True and token.get('elevated') is False and re.fullmatch(r'S-1-5-21-(?:[0-9]+-){3}[0-9]+',token.get('sid','')), 'ordinary-user token proof required')
        cases=doc.get('cases',[])
        require(len(cases) == len(SYMPTOMS) and {p.get('id') for p in cases} == set(SYMPTOMS) and all(p.get('disposition') == 'passed' for p in cases), 'required symptom was failed/skipped/blocked/omitted')
        for case in cases:
            required_checks = {
                'p76-repair-assessment-terminal': ('terminal','progress','unavailable_provider'),
                'p76-care-timeline-persistence': ('reconnect','restart'),
                'p76-care-eligibility-explanation': ('no_op_explained',),
            }.get(case['id'], ())
            require(all(case.get('checks',{}).get(key) is True for key in required_checks), 'required runtime check omitted/failed')
            witnesses=case.get('witnesses',[])
            require(witnesses, 'symptom raw witness omitted')
            for witness in witnesses:
                require(digest(safe_path(directory,witness['path'])) == witness['sha256'], 'symptom witness hash mismatch')
        validate_surfaces(directory,doc,locale)

def accept(release, source, sha, thumb, receipt_hash, bundle_hash, directory):
    receipt=verify(release,source,sha,thumb,receipt_hash,bundle_hash)
    evidence={name:digest(directory / name) for name in ('lifecycle.json','installed-en.json','installed-ar.json')}
    validate_evidence(directory,evidence,sha,bundle_hash,receipt['version'],source)
    return {'schema':'aethercore.rc-acceptance.v1','source_commit':sha,'bundle_sha256':bundle_hash,
            'provenance_sha256':receipt_hash,'signing':receipt['signing'],'ok':True,'evidence':evidence}

def promote(release, source, sha, thumb, receipt_hash, bundle_hash, acceptance, acceptance_hash):
    receipt=verify(release,source,sha,thumb,receipt_hash,bundle_hash)
    require(digest(acceptance) == acceptance_hash, 'acceptance hash mismatch')
    accepted = read_json(acceptance)
    require(accepted.get('schema') == 'aethercore.rc-acceptance.v1' and accepted.get('ok') is True and
            accepted.get('source_commit') == sha and accepted.get('bundle_sha256') == bundle_hash and
            accepted.get('provenance_sha256') == receipt_hash, 'acceptance is absent/failed or belongs to different bytes')
    validate_evidence(acceptance.parent,accepted.get('evidence',{}),sha,bundle_hash,receipt['version'],source)
    return {'schema':'aethercore.rc-promotion.v1', 'source_commit':sha, 'bundle_sha256':bundle_hash,
            'provenance_sha256':receipt_hash, 'acceptance_sha256':acceptance_hash, 'signing':receipt['signing'],
            'rc_eligible':True, 'ga':False}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command',choices=['capture-unsigned','create','verify','verify-baseline','accept','promote'])
    parser.add_argument('--release-root',type=Path,required=True)
    parser.add_argument('--source-root',type=Path,default=Path(__file__).resolve().parents[1])
    parser.add_argument('--expected-sha')
    signing=parser.add_mutually_exclusive_group()
    signing.add_argument('--thumbprint'); signing.add_argument('--unsigned-d32',action='store_true')
    parser.add_argument('--receipt-sha256'); parser.add_argument('--bundle-sha256')
    parser.add_argument('--evidence-directory',type=Path)
    parser.add_argument('--acceptance',type=Path); parser.add_argument('--acceptance-sha256')
    parser.add_argument('--out',type=Path); parser.add_argument('--paths',nargs='+',type=Path)
    args=parser.parse_args()
    try:
        if args.command == 'capture-unsigned': capture_unsigned(args.release_root,args.paths); return 0
        thumb=D32 if args.unsigned_d32 else args.thumbprint
        common=(args.release_root,args.source_root,args.expected_sha,thumb)
        if args.command == 'create': value=create(*common)
        elif args.command == 'verify-baseline': value=verify(args.release_root,None,args.expected_sha,thumb,args.receipt_sha256,args.bundle_sha256)
        elif args.command == 'verify': value=verify(*common,args.receipt_sha256,args.bundle_sha256)
        elif args.command == 'accept': value=accept(*common,args.receipt_sha256,args.bundle_sha256,args.evidence_directory)
        else: value=promote(*common,args.receipt_sha256,args.bundle_sha256,args.acceptance,args.acceptance_sha256)
        if args.out: write_json(args.out,value)
        print(json.dumps(value,sort_keys=True)); return 0
    except (Rejected,OSError,ValueError,KeyError,TypeError,subprocess.SubprocessError) as error:
        print(f'RC BLOCKED: {error}',file=sys.stderr); return 1

if __name__ == '__main__': sys.exit(main())
