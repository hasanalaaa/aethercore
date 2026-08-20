#!/usr/bin/env python3
"""Inventory AetherCore fuzz/property/model coverage without pretending unexecuted targets passed."""
from __future__ import annotations
import argparse, json, re, shutil
from pathlib import Path
from typing import Any

ROOT=Path(__file__).resolve().parents[1]
FUZZ=ROOT/'fuzz'

TARGET_PURPOSE={
 'ipc_frame':['IPC frame decoding','persistent session envelopes','protocol message boundaries'],
 'update_manifest':['update manifest parsing','Ed25519 envelope/manifest validation','HTTPS URL/version/hash field validation'],
 'support_archive':['support archive parsing','USTAR metadata/checksum','fingerprint/signature rejection'],
 'pii_redaction':['PII redaction','SID-like strings','email-like strings','user path masking'],
 'windows_multisz':['checked MULTI_SZ parsing'],
 'scheduler_eligibility':['scheduler eligibility determinism','block-reason uniqueness','mutation exclusion'],
 'operation_state':['operation state parsing','terminal-state classification'],
}
ALTERNATIVE_EVIDENCE={
 'NVMe binary parser':('table-driven Rust tests','nvme_parser_rejects_truncated_vendor_response'),
 'ATA SMART parser':('table-driven Rust tests','ata_smart_sector_parser_rejects_short_buffers'),
 'WHEA/event interpretation':('table-driven Rust tests','whea_memory_is_evidence_not_dimm_diagnosis'),
 'scheduler jitter/backoff':('property/table Rust tests','jitter_is_bounded_and_zero_configuration_is_deterministic'),
 'consent token handling':('state-machine Rust tests','consent_intent_is_principal_bound_and_one_shot'),
 'immutable plan verification':('state-machine Rust tests','duplicated_plan_columns_cannot_diverge_from_hashed_material'),
 'update state transitions':('coordinator Rust tests','signed_manifest_floor_rejects_rollback_and_same_sequence_equivocation'),
}

def source_corpus()->str:
    return '\n'.join(p.read_text(encoding='utf-8',errors='replace') for base in [ROOT/'crates',ROOT/'services',ROOT/'apps'] for p in base.rglob('*.rs'))

def main()->int:
    ap=argparse.ArgumentParser();ap.add_argument('--json',type=Path);args=ap.parse_args()
    cargo_toml=(FUZZ/'Cargo.toml').read_text(encoding='utf-8')
    bins={name:path for name,path in re.findall(r'(?ms)\[\[bin\]\]\s*name\s*=\s*"([^"]+)"\s*path\s*=\s*"([^"]+)"',cargo_toml)}
    targets=[]
    for name,path in sorted(bins.items()):
        file=FUZZ/path
        targets.append({'name':name,'path':file.relative_to(ROOT).as_posix(),'present':file.is_file(),'meaningful_invariants':TARGET_PURPOSE.get(name,[])})
    corpus=source_corpus()
    alternatives=[]
    for candidate,(technique,marker) in ALTERNATIVE_EVIDENCE.items():
        alternatives.append({'candidate':candidate,'technique':technique,'marker':marker,'source_present':marker in corpus,'executed_in_this_host':False})
    cargo=shutil.which('cargo');cargo_fuzz=shutil.which('cargo-fuzz')
    inventory_ok=all(t['present'] and t['meaningful_invariants'] for t in targets) and all(a['source_present'] for a in alternatives)
    evidence:dict[str,Any]={
      'schema':'aethercore.sigma-master-fuzz-inventory.v1',
      'inventory_source_ready':inventory_ok,
      'registered_targets':targets,
      'alternative_property_table_model_evidence':alternatives,
      'execution':{'cargo':cargo,'cargo_fuzz':cargo_fuzz,'executed':False},
    }
    if cargo and cargo_fuzz:
        evidence['status']='READY_TO_EXECUTE'
        evidence['execution']['note']='This inventory command does not launch an unbounded fuzzer; run the approved bounded Windows/CI fuzz campaign.'
        code=0 if inventory_ok else 1
    else:
        evidence['status']='BLOCKED'
        evidence['blocker']={'class':'FUZZ_BLOCKER','condition':'Rust/cargo-fuzz toolchain is unavailable on this host; registered targets are not counted as passed.'}
        code=2 if inventory_ok else 1
    rendered=json.dumps(evidence,indent=2,sort_keys=True)+'\n'
    if args.json:args.json.parent.mkdir(parents=True,exist_ok=True);args.json.write_text(rendered,encoding='utf-8')
    print(rendered,end='');return code
if __name__=='__main__':raise SystemExit(main())
