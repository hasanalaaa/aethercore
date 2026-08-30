#!/usr/bin/env python3
"""Create deterministic Sigma Master source/master archives and binary-safe reconstruction proof.

The exact Sigma baseline is the git baseline ref. All Sigma Master changes must be staged so the
binary patch includes modified, added and deleted paths. Evidence is supplied from an external
source-closure directory and never generated inside the source tree.
"""
from __future__ import annotations
import argparse, hashlib, io, json, os, re, shutil, subprocess, tarfile, tempfile, time, zipfile
from pathlib import Path
from typing import Any

ROOT=Path(__file__).resolve().parents[1]
EXCLUDED_DIRS={'.git','target','node_modules','out','__pycache__','dist'}
EXCLUDED_SUFFIXES={'.pyc','.pyo'}


def run(cmd:list[str],cwd:Path=ROOT)->subprocess.CompletedProcess[bytes]:
    return subprocess.run(cmd,cwd=cwd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=False)

def sha_bytes(data:bytes)->str:return hashlib.sha256(data).hexdigest()
def sha_file(path:Path)->str:
    h=hashlib.sha256()
    with path.open('rb') as f:
        for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
    return h.hexdigest()

def included(path:Path,root:Path)->bool:
    rel=path.relative_to(root)
    return path.is_file() and not path.is_symlink() and not any(part in EXCLUDED_DIRS for part in rel.parts) and path.suffix not in EXCLUDED_SUFFIXES

def files(root:Path)->list[Path]:
    symlinks=[p for p in root.rglob('*') if p.is_symlink() and not any(part in EXCLUDED_DIRS for part in p.relative_to(root).parts)]
    if symlinks:raise RuntimeError('source package refuses symlinks: '+', '.join(p.relative_to(root).as_posix() for p in symlinks[:8]))
    return sorted((p for p in root.rglob('*') if included(p,root)),key=lambda p:p.relative_to(root).as_posix())

def state(root:Path)->dict[str,str]:return {p.relative_to(root).as_posix():sha_file(p) for p in files(root)}

def zip_datetime(epoch:int)->tuple[int,int,int,int,int,int]:
    t=time.gmtime(max(epoch,315532800));year=min(max(t.tm_year,1980),2107);return(year,t.tm_mon,t.tm_mday,t.tm_hour,t.tm_min,t.tm_sec-(t.tm_sec%2))

def add_bytes(z:zipfile.ZipFile,name:str,data:bytes,epoch:int,executable:bool=False)->None:
    info=zipfile.ZipInfo(name,zip_datetime(epoch));info.compress_type=zipfile.ZIP_DEFLATED;info.create_system=3;info.external_attr=((0o755 if executable else 0o644)<<16);z.writestr(info,data,compress_type=zipfile.ZIP_DEFLATED,compresslevel=9)

def deterministic_zip(path:Path,entries:list[tuple[str,Path]],epoch:int)->None:
    with zipfile.ZipFile(path,'w') as z:
        for name,src in sorted(entries):add_bytes(z,name,src.read_bytes(),epoch,bool(src.stat().st_mode & 0o111))

def main()->int:
    ap=argparse.ArgumentParser();ap.add_argument('--evidence-dir',type=Path,required=True);ap.add_argument('--output-dir',type=Path,required=True);ap.add_argument('--baseline-ref',default='HEAD');args=ap.parse_args()
    evidence=args.evidence_dir.expanduser().resolve();out=args.output_dir.expanduser().resolve();out.mkdir(parents=True,exist_ok=True)
    try:out.relative_to(ROOT.resolve());raise SystemExit('output-dir must be outside source tree')
    except ValueError:pass
    if not evidence.is_dir():raise SystemExit('evidence-dir missing')
    status=run(['git','status','--porcelain=v1']).stdout.decode('utf-8','replace')
    unstaged=run(['git','diff','--name-only']).stdout.decode().strip().splitlines()
    untracked=[line[3:] for line in status.splitlines() if line.startswith('?? ')]
    if unstaged or untracked:
        raise SystemExit('Package requires all Master changes staged (git add -A) so the binary patch is complete.')
    patch_proc=run(['git','diff','--cached','--binary','--full-index',args.baseline_ref,'--','.'])
    if patch_proc.returncode:raise SystemExit(patch_proc.stderr.decode())
    patch=out/'AetherCore-Sigma-to-Sigma-Master.patch';patch.write_bytes(patch_proc.stdout)
    changes_raw=run(['git','diff','--cached','--name-status',args.baseline_ref,'--','.']).stdout.decode('utf-8','replace').splitlines()
    changes=[]
    for line in changes_raw:
        if not line.strip():continue
        parts=line.split('\t');status_code=parts[0];paths=parts[1:]
        changes.append({'status':status_code,'paths':paths})
    change_doc={'schema':'aethercore.sigma-master-change-inventory.v1','baseline_ref':args.baseline_ref,'total':len(changes),'added':sum(x['status'].startswith('A') for x in changes),'modified':sum(x['status'].startswith('M') for x in changes),'deleted':sum(x['status'].startswith('D') for x in changes),'renamed':sum(x['status'].startswith('R') for x in changes),'changes':changes}
    change_path=out/'AetherCore-Sigma-Master-Change-Inventory.json';change_path.write_text(json.dumps(change_doc,indent=2,sort_keys=True)+'\n',encoding='utf-8')

    epoch=int((ROOT/'release/source-date-epoch.txt').read_text().strip()) if (ROOT/'release/source-date-epoch.txt').is_file() else 315532800
    source_paths=files(ROOT);source_zip=out/'AetherCore-Sigma-Master-Source.zip'
    deterministic_zip(source_zip,[(f'AetherCore-Sigma-Master-Source/{p.relative_to(ROOT).as_posix()}',p) for p in source_paths],epoch)
    source_state=state(ROOT)
    source_inventory={'schema':'aethercore.sigma-master-source-sha256.v1','files':len(source_state),'tree_sha256':sha_bytes(''.join(f'{k}\0{v}\n' for k,v in sorted(source_state.items())).encode()),'entries':[{'path':k,'sha256':v} for k,v in sorted(source_state.items())]}
    source_inventory_path=out/'AetherCore-Sigma-Master-Source-SHA256.json';source_inventory_path.write_text(json.dumps(source_inventory,indent=2,sort_keys=True)+'\n',encoding='utf-8')
    shutil.copy2(ROOT/'MANIFEST.sha256',out/'MANIFEST.sha256')

    with tempfile.TemporaryDirectory(prefix='aethercore-reconstruct-') as td:
        td=Path(td);base=td/'baseline';base.mkdir()
        archive=run(['git','archive','--format=tar',args.baseline_ref])
        if archive.returncode:raise RuntimeError(archive.stderr.decode())
        with tarfile.open(fileobj=io.BytesIO(archive.stdout),mode='r:') as tf:tf.extractall(base,filter='data')
        apply=subprocess.run(['git','apply','--binary','--whitespace=nowarn',str(patch)],cwd=base,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        reconstructed=state(base) if apply.returncode==0 else {}
        expected=source_state
        mismatches=[]
        for rel in sorted(set(expected)|set(reconstructed)):
            if expected.get(rel)!=reconstructed.get(rel):mismatches.append({'path':rel,'expected':expected.get(rel),'reconstructed':reconstructed.get(rel)})
        proof={'schema':'aethercore.sigma-master-patch-reconstruction.v1','baseline_ref':args.baseline_ref,'patch_sha256':sha_file(patch),'apply_exit_code':apply.returncode,'apply_stderr':apply.stderr[-4000:],'expected_files':len(expected),'reconstructed_files':len(reconstructed),'byte_identical':apply.returncode==0 and not mismatches,'mismatches':mismatches[:64],'expected_tree_sha256':source_inventory['tree_sha256'],'reconstructed_tree_sha256':sha_bytes(''.join(f'{k}\0{v}\n' for k,v in sorted(reconstructed.items())).encode()) if reconstructed else None}
    proof_path=out/'AetherCore-Sigma-Master-Patch-Reconstruction-Proof.json';proof_path.write_text(json.dumps(proof,indent=2,sort_keys=True)+'\n',encoding='utf-8')
    if not proof['byte_identical']:raise SystemExit('binary-safe patch reconstruction did not reproduce final source bytes')

    required=['AetherCore-Sigma-Master-Qualification-Summary.json','AetherCore-Sigma-Master-Issue-Ledger.json','AetherCore-Sigma-Master-Blocker-Ledger.json','AetherCore-Sigma-Master-Release-Status.json','AetherCore-Sigma-Master-Scorecard.json']
    missing=[name for name in required if not (evidence/name).is_file()]
    if missing:raise SystemExit('missing required source-closure evidence: '+', '.join(missing))

    delivery_entries=[]
    for p in sorted(evidence.rglob('*')):
        if p.is_file():delivery_entries.append((f'evidence/{p.relative_to(evidence).as_posix()}',p))
    for p in [source_zip,patch,change_path,out/'MANIFEST.sha256',source_inventory_path,proof_path]:delivery_entries.append((p.name,p))
    inventory={'schema':'aethercore.sigma-master-delivery-inventory.v1','files':[{'path':name,'sha256':sha_file(path),'size_bytes':path.stat().st_size} for name,path in sorted(delivery_entries)]}
    inventory_path=out/'AetherCore-Sigma-Master-Delivery-SHA256-Inventory.json';inventory_path.write_text(json.dumps(inventory,indent=2,sort_keys=True)+'\n',encoding='utf-8')
    delivery_entries.append((inventory_path.name,inventory_path))
    master=out/'AetherCore-Sigma-Master-Delivery.zip';deterministic_zip(master,delivery_entries,epoch)
    master_sha=sha_file(master);(out/'AetherCore-Sigma-Master-Delivery.zip.sha256').write_text(f'{master_sha}  {master.name}\n',encoding='ascii')
    # Audit the created archive itself for forbidden packaging debris/copy suffixes.
    with zipfile.ZipFile(master) as z:
        names=z.namelist();violations=[n for n in names if '__MACOSX' in n or '/out/' in n or '/target/' in n or '/node_modules/' in n or '/__pycache__/' in n or re.search(r' \(\d+\)(?:\.[^/]*)?$',Path(n).name) or Path(n).name in {'.DS_Store'} or Path(n).suffix in {'.bak','.tmp','.orig','.rej'}]
    audit={'schema':'aethercore.sigma-master-package-audit.v1','ok':not violations,'entries':len(names),'violations':violations,'master_sha256':master_sha,'source_zip_sha256':sha_file(source_zip),'patch_sha256':sha_file(patch),'patch_reconstruction':proof['byte_identical']}
    audit_path=out/'AetherCore-Sigma-Master-Package-Audit.json';audit_path.write_text(json.dumps(audit,indent=2,sort_keys=True)+'\n',encoding='utf-8')
    if violations:raise SystemExit('package audit failed: '+', '.join(violations[:8]))
    print(json.dumps({'ok':True,'master_delivery':str(master),'master_sha256':master_sha,'source_zip':str(source_zip),'source_zip_sha256':sha_file(source_zip),'patch':str(patch),'changes':change_doc},indent=2))
    return 0
if __name__=='__main__':raise SystemExit(main())
