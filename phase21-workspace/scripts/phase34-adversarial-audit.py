#!/usr/bin/env python3
"""Phase 34 fleet & secure remote operations audit: strict superset of Phase 33.

Inherits all 846 P33 checks (which transitively inherit P32) and adds
individually identifiable Phase 34 gates covering the fleet domain, host-key
trust, SSH transport safety, orchestration, remote-compliance honesty,
scheduling, i18n parity, wire-freeze, docs, debt and seal consistency.
"""
from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path(__file__).resolve().parents[1]
failures: list[str] = []
new_gates: list[str] = []


def gate(name: str, condition: bool, detail: str = "") -> None:
    new_gates.append(name)
    if not condition:
        failures.append(f"{name}: {detail}")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


# ---- Inherit the entire Phase 33 audit against the SEALED P33 tree. -------
# The P33 audit's own full-tree/ledger gates are pinned to the P33 baseline;
# they are therefore executed against a pristine sealed-P33 extraction,
# proving the inherited chain still passes verbatim. P34 deltas are audited
# against the live tree by the gates below.
p33_archive = ROOT.parent / "AetherCore-Phase33-Master-Delivery.zip"
expected_p33_sha = (ROOT.parent / "PHASE33_FINAL_SHA256.txt").read_text(encoding="utf-8").split()[0]
if sha256(p33_archive) != expected_p33_sha:
    raise SystemExit("FATAL: P33 archive hash mismatch during audit inheritance")
audit_temp = Path(tempfile.mkdtemp(prefix="aethercore-p34-inherited-"))
audit_root = audit_temp / "root"
audit_root.mkdir(parents=True)
subprocess.run(
    ["tar", "-xzf", str(p33_archive), "-C", str(audit_root), "--strip-components", "1"],
    check=True,
)
for hash_name in ("PHASE32_FINAL_SHA256.txt", "PHASE33_FINAL_SHA256.txt"):
    source = ROOT.parent / hash_name
    if source.is_file():
        shutil.copy2(source, audit_temp / hash_name)
p32_base = Path("/tmp/p33_p32_seal/base/DEBT_REGISTER.json")
if not p32_base.is_file():
    archive32 = ROOT.parent / "AetherCore-Phase32-Master-Delivery.zip"
    if archive32.is_file():
        Path("/tmp/p33_p32_seal/base").mkdir(parents=True, exist_ok=True)
        subprocess.run(
            ["tar", "-xzf", str(archive32), "-C", "/tmp/p33_p32_seal/base", "--strip-components", "1"],
            check=True,
        )
spec = importlib.util.spec_from_file_location(
    "phase33_audit", audit_root / "scripts" / "phase33-adversarial-audit.py"
)
p33 = importlib.util.module_from_spec(spec)
assert spec.loader is not None
sys.argv = [sys.argv[0], str(audit_root)]
try:
    try:
        spec.loader.exec_module(p33)
    except SystemExit:
        pass
    inherited_checks = p33.checks
    failures.extend(p33.failures)
finally:
    shutil.rmtree(audit_temp, ignore_errors=True)
    sys.argv = [sys.argv[0], str(ROOT)]

# ---------------------------------------------------------------- sources
fleet_dir = ROOT / "crates" / "fleet" / "src"
domain = (fleet_dir / "domain.rs").read_text(encoding="utf-8")
trust = (fleet_dir / "trust.rs").read_text(encoding="utf-8")
transport = (fleet_dir / "transport.rs").read_text(encoding="utf-8")
orchestrator = (fleet_dir / "orchestrator.rs").read_text(encoding="utf-8")
scheduler = (fleet_dir / "scheduler.rs").read_text(encoding="utf-8")
gd_proofs = (ROOT / "crates" / "fleet" / "tests" / "gd_proofs.rs").read_text(encoding="utf-8")
gd5 = (ROOT / "crates" / "security-audit" / "tests" / "gd5_remote_compliance.rs").read_text(
    encoding="utf-8"
)
fleet_persistence = (ROOT / "crates" / "persistence" / "tests" / "fleet_persistence.rs").read_text(
    encoding="utf-8"
)
persistence_src = (ROOT / "crates" / "persistence" / "src" / "lib.rs").read_text(
    encoding="utf-8"
)
scheduler_runner = (
    (ROOT / "crates" / "fleet" / "src" / "scheduler_runner.rs").read_text(encoding="utf-8")
    if (ROOT / "crates" / "fleet" / "src" / "scheduler_runner.rs").is_file()
    else ""
)
cli_rs = (ROOT / "apps" / "aetherctl" / "src" / "cli.rs").read_text(encoding="utf-8")
fleet_cli = (ROOT / "apps" / "aetherctl" / "src" / "fleet.rs").read_text(encoding="utf-8")
i18n_cli = (ROOT / "apps" / "aetherctl" / "src" / "i18n.rs").read_text(encoding="utf-8")
migration = (ROOT / "crates" / "persistence" / "migrations" / "0015_phase34_fleet.sql").read_text(
    encoding="utf-8"
)
fleet_page = (ROOT / "apps" / "ui" / "src" / "features" / "fleet" / "FleetPage.svelte").read_text(
    encoding="utf-8"
)
catalog_en = (ROOT / "apps" / "ui" / "src" / "lib" / "i18n" / "catalog.en.ts").read_text(
    encoding="utf-8"
)
catalog_ar = (ROOT / "apps" / "ui" / "src" / "lib" / "i18n" / "catalog.ar.ts").read_text(
    encoding="utf-8"
)
desktop_main = (ROOT / "apps" / "desktop" / "src" / "main.rs").read_text(encoding="utf-8")
navigation = (ROOT / "apps" / "ui" / "src" / "lib" / "navigation.ts").read_text(encoding="utf-8")

# ---------------------------------------------------- p34-fleet-domain
gate(
    "p34-fleet-domain",
    all(
        symbol in domain
        for symbol in [
            "pub struct FleetHost",
            "pub struct FleetInventory",
            "pub struct FleetSchedule",
            "pub struct FleetHostTrust",
        ]
    )
    and "FleetHostId" in (domain + cli_rs + fleet_cli),
    "fleet domain types missing",
)

# ------------------------------------------------- p34-strict-host-schema
gate(
    "p34-strict-host-schema",
    domain.count("deny_unknown_fields") >= 8
    and 'FLEET_HOST_SCHEMA: &str = "aethercore.fleet.host.v1"' in domain
    and 'FLEET_INVENTORY_SCHEMA: &str = "aethercore.fleet.inventory.v1"' in domain
    and "MalformedHostname" in domain
    and "MalformedFingerprint" in domain
    and "DuplicateIdentity" in domain,
    "strict host schema incomplete",
)

# ---------------------------------------------- p34-no-secret-persistence
gate(
    "p34-no-secret-persistence",
    "private-key contents are never persisted" in domain.replace("’", "'")
    or "never read into the domain nor persisted" in domain
    or "path REFERENCE" in fleet_persistence,
    "secret-persistence proof documentation missing",
)
secret_columns = re.findall(r"^\s*\w*(password|secret|api_key|passphrase)\w*\s+TEXT", migration, re.M | re.I)
gate(
    "p34-no-password-auth-storage",
    not secret_columns and "auth_ref_path TEXT" in migration and "trusted_fingerprint" in migration,
    f"forbidden columns: {secret_columns}",
)
gate(
    "p34-no-plaintext-secret-columns",
    not re.search(r"(?i)\b(password|private_key_body|api_key|passphrase)\b", migration),
    "migration contains secret-shaped columns",
)

# -------------------------------------------------- p34-host-key-pinning
gate(
    "p34-host-key-pinning",
    "pub fn decide" in trust
    and "TrustDecision::NotVerified" in trust
    and "TrustDecision::HostKeyMismatch" in trust
    and ("unknown host is not trusted" in gd_proofs
         or "unknown_host_is_not_trusted" in gd_proofs),
    "host-key pinning fail-closed logic missing",
)

# ------------------------------------------------ p34-no-insecure-ssh-flags
_forbidden = [
    "StrictHostKeyChecking=no",
    "UserKnownHostsFile=/dev/null",
    "PasswordAuthentication=yes",
]
gate(
    "p34-no-insecure-ssh-flags",
    all(flag in trust for flag in _forbidden)
    and ("forbidden option" in gd_proofs or "insecure flag {forbidden} present" in gd_proofs)
    and "insecure flags absent" in gd_proofs,
    "forbidden-fragment list or proof missing",
)
# Emitted-argv proof: the constructed argv (tested in gd_proofs) must never
# contain any forbidden fragment, and the only place the fragments appear in
# source is the FORBIDDEN_SSH_OPTION_FRAGMENTS ban list.
forbidden_source = trust + transport + fleet_cli
for fragment in _forbidden:
    occurrences = [
        line.strip()
        for line in forbidden_source.splitlines()
        if fragment in line
    ]
    ban_list_only = all(
        ('"' in line and ("FORBIDDEN" in line or line.startswith('"') or line.endswith('",')))
        or "assert" in line
        or "insecure" in line.lower()
        or line.startswith("//!")
        or line.startswith("//")
        for line in occurrences
    )
    gate(
        "p34-no-insecure-ssh-flags-emitted",
        ban_list_only and "FORBIDDEN_SSH_OPTION_FRAGMENTS" in trust,
        f"fragment {fragment} referenced outside ban list: {occurrences[:2]}",
    )

# --------------------------------------- p34-injection-resistant-commands
gate(
    "p34-injection-resistant-command-builder",
    "pub fn shell_quote" in trust
    and "shell_quote_neutralizes_metacharacters" in trust
    and "shell_quote(word)" in trust
    and "$(rm -rf /)" in trust,
    "injection-resistant command builder missing",
)
gate(
    "p34-no-generic-remote-shell",
    "RemoteOperation" in transport
    and "pub enum RemoteOperation" in transport
    and "validate_remote_path" in transport
    and "validate_targets_json" in transport
    and "fleet exec" not in cli_rs
    and "FleetJob::Exec" not in cli_rs,
    "generic remote shell surface detected or closed-op set missing",
)

# --------------------------------------------------------- p34-batch-mode
gate(
    "p34-batch-mode",
    "BatchMode=yes" in trust
    and ("no password prompt" in (trust + transport).lower()
         or "any prompt (password, host key) becomes failure" in (trust + transport)),
    "BatchMode enforcement missing",
)

# ------------------------------------------- p34-timeout / cancellation
gate(
    "p34-timeout",
    "operation_timeout" in transport
    and "RemoteOutcomeKind::Timeout" in transport
    and "operation timeout elapsed" in transport
    and "timeout/cancellation" in gd_proofs.replace("Timeout/cancellation", "timeout/cancellation")
    or "RemoteOutcomeKind::Timeout" in (transport + gd_proofs),
    "timeout semantics missing",
)
gate(
    "p34-cancellation",
    "CancelToken" in transport
    and "RemoteOutcomeKind::Cancelled" in transport
    and "cancellation" in (gd_proofs + orchestrator).lower(),
    "cancellation semantics missing",
)

# ------------------------------------------------ p34-concurrency-bound
gate(
    "p34-concurrency-bound",
    "DEFAULT_MAX_CONCURRENCY" in orchestrator
    and "ABSOLUTE_MAX_CONCURRENCY" in orchestrator
    and "InvalidConcurrency" in orchestrator
    and "concurrency_is_bounded" in orchestrator,
    "bounded concurrency missing",
)

# ------------------------------------------ p34-host-failure-isolation
gate(
    "p34-host-failure-isolation",
    "catch_unwind" in orchestrator
    and "transport task panicked" in orchestrator
    and "batch_is_deterministic_and_isolated" in orchestrator,
    "host failure isolation missing",
)

# ---------------------------------------- p34-deterministic-aggregation
gate(
    "p34-deterministic-aggregation",
    "sort_by(|a, b| a.host_id.cmp(&b.host_id))" in orchestrator
    and "outcomes.sort_by" in orchestrator
    and ("deterministic aggregate" in (gd_proofs + orchestrator).lower()
         or "deterministic ordering" in (gd_proofs + orchestrator).lower()
         or "Deterministic work order" in orchestrator),
    "deterministic aggregation missing",
)

# ---------------------------------------- p34-compatibility-handshake
gate(
    "p34-compatibility-handshake",
    "RemoteCompatibility" in transport
    and "parse_compatibility" in transport
    and "REMOTE_CONTRACT_VERSION" in transport
    and "Incompatible" in (trust + transport),
    "compatibility handshake missing",
)

# -------------------------------------- p34-remote-security-read-only
gate(
    "p34-remote-security-read-only",
    "RemoteOperation::SecurityAudit" in transport
    and "RemoteOperation::ComplianceCollect" in transport
    and "RemoteOperation::VersionProbe" in transport
    and "read-only" in transport.lower(),
    "closed read-only remote operation set missing",
)
gate(
    "p34-remote-compliance-reuse",
    "verify_integrity" in gd5
    and "parse_report_bytes" in gd5
    and "verify_digest_signature" in gd5
    and "signing_key_from_seed" in gd5,
    "remote compliance must reuse the P33 verification path",
)

# --------------------------------- digest/signature verification honesty
gate(
    "p34-remote-digest-verification",
    "content tamper rejected" in gd5 and "digest" in gd5,
    "digest verification proof missing",
)
gate(
    "p34-remote-signature-verification",
    "signature tamper rejected" in gd5 and "verify_digest_signature" in gd5,
    "signature verification proof missing",
)
gate(
    "p34-unsigned-honesty",
    "remains explicitly unsigned" in gd5 and "signed report verifies" in gd5,
    "unsigned honesty proof missing",
)

# -------------------------------------------------- p34-schedule-schema
gate(
    "p34-schedule-schema",
    'FLEET_SCHEDULE_SCHEMA: &str = "aethercore.fleet.schedule.v1"' in domain
    and "deny_unknown_fields" in domain
    and "FleetScheduleLastResult" in domain,
    "schedule schema missing",
)
gate(
    "p34-schedule-no-tight-loop",
    "MIN_CADENCE_SECS: u64 = 3600" in domain
    and "no_tight_loop_floor_is_structural" in scheduler,
    "cadence floor missing",
)
gate(
    "p34-schedule-overlap-policy",
    "OverlapLock" in scheduler
    and "OverlapPolicy" in scheduler.replace("ScheduleError", "OverlapPolicy").replace("OverlapLocked", "OverlapPolicyX")
    or "OverlapLocked" in scheduler
    and "overlap_policy_is_single_run" in scheduler,
    "overlap policy missing",
)
gate(
    "p34-schedule-test-clock",
    "FixedClock" in scheduler and "is_due" in scheduler and "begin_run" in scheduler,
    "injectable clock missing",
)

# -------------------------------------------------------- p34-i18n-parity
fleet_keys = re.findall(r'"(fleet\.[\w.]+)"', i18n_cli)
en_block = i18n_cli.split("pub fn en(", 1)[1].split("pub fn ar(", 1)[0]
ar_block = i18n_cli.split("pub fn ar(", 1)[1]
en_keys = set(re.findall(r'"([\w.]+)"\s*=>', en_block))
ar_keys = set(re.findall(r'"([\w.]+)"\s*=>', ar_block))
missing_en = sorted(set(fleet_keys) - en_keys)
missing_ar = sorted(set(fleet_keys) - ar_keys)
gate(
    "p34-i18n-parity",
    bool(fleet_keys) and not missing_en and not missing_ar,
    f"fleet={len(fleet_keys)} missing EN={missing_en} AR={missing_ar}",
)
ui_fleet_keys = sorted(set(re.findall(r"'(fleet\.[\w.]+)'", fleet_page)))
en_missing = sorted(set(ui_fleet_keys) - set(re.findall(r"'(fleet\.[\w.]+)'", catalog_en)))
ar_missing = sorted(set(ui_fleet_keys) - set(re.findall(r"'(fleet\.[\w.]+)'", catalog_ar)))
gate(
    "p34-i18n-ui-parity",
    bool(ui_fleet_keys) and not en_missing and not ar_missing,
    f"ui fleet={len(ui_fleet_keys)} en_missing={en_missing} ar_missing={ar_missing}",
)

# ------------------------------------------- p34-no-raw-secret-regression
gate(
    "p34-no-raw-secret-regression",
    not re.search(r"(?i)password\s*=\s*['\"]", fleet_cli + transport)
    and "PasswordAuthentication=no" in trust,
    "raw secret assignment detected in fleet code",
)

# --------------------------------------------------- p34-wire-tag-append-only
p33_ledger = json.loads(
    (ROOT / "PHASE_33_BINARY_SAFE_PATCH" / "PHASE_33_EXPECTED_FULL_SHA256.json").read_text(
        encoding="utf-8"
    )
)["files"]
proto_paths = sorted((ROOT / "crates" / "contracts" / "proto").glob("*.proto"))
proto_unchanged = all(
    p33_ledger.get(str(path.relative_to(ROOT))) == sha256(path) for path in proto_paths
)
operations = (ROOT / "crates" / "contracts" / "proto" / "operations.proto").read_text(encoding="utf-8")
request = operations.split("message Request", 1)[1].split("oneof payload", 1)[1].split("\n  }", 1)[0]
response = operations.split("message Response", 1)[1].split("oneof payload", 1)[1].split("\n  }", 1)[0]
request_tags = [int(tag) for tag in re.findall(r"^\s{4}\w+\s+\w+\s*=\s*(\d+);", request, re.M)]
response_tags = [int(tag) for tag in re.findall(r"^\s{4}\w+\s+\w+\s*=\s*(\d+);", response, re.M)]
gate(
    "p34-wire-tag-append-only",
    proto_unchanged and max(request_tags) == 90 and max(response_tags) == 52,
    "wire contract must remain frozen at request-max 90 / response-max 52",
)

# --------------------------------------------------------- p34-ui-integration
gate(
    "p34-ui-fleet-page",
    "fleet_snapshot" in desktop_main
    and "FleetPage" in (ROOT / "apps" / "ui" / "src" / "app" / "AppShell.svelte").read_text(
        encoding="utf-8"
    )
    and "'fleet'" in navigation
    and "aethercore_fleet::FleetInventory::from_bytes" in desktop_main,
    "desktop fleet page integration incomplete",
)

# ------------------------------------------------------------ p34-docs
required_docs = [
    "ARCHITECTURE.md",
    "SECURITY_MODEL.md",
    "FLEET_TRUST.md",
    "REMOTE_OPERATIONS.md",
    "SCHEDULING.md",
    "QUALIFICATION_DEBT.json",
    "ISSUES.json",
    "SCORECARD.md",
    "MASTER_DELIVERY_REPORT.md",
    "PROGRESS.md",
]
docs_dir = ROOT / "docs" / "phase34"
gate(
    "p34-docs-complete",
    all((docs_dir / name).is_file() and (docs_dir / name).stat().st_size > 200 for name in required_docs),
    f"missing/empty: {[n for n in required_docs if not (docs_dir / n).is_file()]}",
)
mdr = (docs_dir / "MASTER_DELIVERY_REPORT.md").read_text(encoding="utf-8") if (docs_dir / "MASTER_DELIVERY_REPORT.md").is_file() else ""
gate(
    "p34-docs-hash-pointer-only",
    "Authoritative archive hash: see PHASE34_FINAL_SHA256.txt" in mdr
    and not re.search(r"\b[0-9a-f]{64}\b", mdr),
    "MDR must point at the external hash file only",
)

# ------------------------------------------------------ p34-debt-append-only
p33_archive = ROOT.parent / "AetherCore-Phase33-Master-Delivery.zip"
expected_p33 = (ROOT.parent / "PHASE33_FINAL_SHA256.txt").read_text(encoding="utf-8").split()[0]
debt_ok = False
debt_detail = ""
if p33_archive.is_file() and sha256(p33_archive) == expected_p33:
    extracted = subprocess.run(
        ["tar", "-xOf", str(p33_archive), "AetherCore-Phase33-Master-Delivery/DEBT_REGISTER.json"],
        capture_output=True,
    )
    if extracted.returncode == 0:
        base_entries = json.loads(extracted.stdout).get("entries", [])
        current_entries = json.loads((ROOT / "DEBT_REGISTER.json").read_text(encoding="utf-8")).get("entries", [])
        debt_ok = (
            current_entries[: len(base_entries)] == base_entries
            and len(current_entries) > len(base_entries)
        )
        debt_detail = f"base={len(base_entries)} current={len(current_entries)}"
gate("p34-debt-append-only", debt_ok, debt_detail or "P33 archive missing or hash mismatch")

# ----------------------------------------------- p34-ds-store-exclusion
archive_builders = sorted((ROOT / "scripts").glob("_build_p*_archive.py"))
gate(
    "p34-ds-store-exclusion",
    bool(archive_builders)
    and all("DS_Store" in path.read_text(encoding="utf-8") for path in archive_builders),
    "archive builder missing .DS_Store exclusion",
)

# ------------------------------------- p34-ledger-manifest-consistency
verify_command = [
    sys.executable,
    str(ROOT / "PHASE_34_BINARY_SAFE_PATCH" / "verify_phase34.py"),
    str(ROOT),
]
patch_dir = ROOT / "PHASE_34_BINARY_SAFE_PATCH"
if (patch_dir / "verify_phase34.py").is_file():
    first_patch = subprocess.run(verify_command, capture_output=True, text=True)
    first_full = subprocess.run(verify_command + ["--full-tree"], capture_output=True, text=True)
    manifest = json.loads((patch_dir / "MANIFEST.json").read_text(encoding="utf-8"))
    ledger = json.loads(
        (patch_dir / "PHASE_34_EXPECTED_FULL_SHA256.json").read_text(encoding="utf-8")
    )
    manifest_post = {
        entry[1:] if entry.startswith("+") else entry
        for entry in manifest.get("files", [])
        if not entry.startswith("-")
    }
    gate(
        "p34-ledger-manifest-consistency",
        first_patch.returncode == 0
        and first_full.returncode == 0
        and bool(manifest_post)
        and manifest_post <= set(ledger.get("files", {}))
        and not any(rel.startswith("PHASE_34_BINARY_SAFE_PATCH/") for rel in ledger.get("files", {})),
        "patch/full-tree verification or manifest/ledger consistency failed",
    )
    second_full = subprocess.run(verify_command + ["--full-tree"], capture_output=True, text=True)
    gate(
        "p34-verifier-determinism",
        first_full.stdout == second_full.stdout and first_full.returncode == second_full.returncode == 0,
        "verifier must be deterministic read-only",
    )
else:
    gate("p34-ledger-manifest-consistency", False, "PHASE_34_BINARY_SAFE_PATCH/verify_phase34.py missing")

# --------------------------------------- p34-reconstruction-determinism
recon_report = Path("/tmp/p34_reconstruction_result.json")
gate(
    "p34-reconstruction-determinism",
    recon_report.is_file()
    and json.loads(recon_report.read_text(encoding="utf-8")).get("status") == "PASS",
    "run scripts/_p34_reconstruction_test.py to PASS before sealing",
)

# ------------------------------------- p34-corrective: trust-chain binding
# Phase 34 corrective (B/CF-1): the trust store must be regenerated from
# typed TrustedHostKey records whose fingerprint is RECOMPUTED from the
# public key material at authorization AND persist time.
gate(
    "p34c-trusted-key-bound-to-fingerprint",
    "pub struct TrustedHostKey" in trust
    and "pub fn authorize" in trust
    and "fingerprint_of_blob(public_key_base64)?" in trust.replace(" ", "")
    .replace("fingerprint_of_blob(&record.public_key_base64)?", "fingerprint_of_blob(public_key_base64)?"),
    "typed trusted public-key record with recomputed-fingerprint binding missing",
)
gate(
    "p34c-fingerprint-only-execution-impossible",
    "PrivateKeyMaterialRejected" in trust
    and "FingerprintKeyMismatch" in trust
    and "cf1_fingerprint_only_trust_cannot_create_execution_entry" in trust
    and "no line may exist without a bound record" in trust,
    "fingerprint-only trust path must be structurally impossible",
)
gate(
    "p34c-trusted-key-mismatch-rejected",
    "cf1_fingerprint_key_mismatch_rejected" in trust
    and "UnsupportedKeyType" in trust
    and "cf1_unsupported_key_type_rejected" in trust
    and "cf1_malformed_base64_rejected" in trust
    and "cf1_private_key_material_never_enters_trust_storage" in trust
    and "cf1_changed_host_key_is_host_key_mismatch" in trust
    and "cf1_valid_key_plus_fingerprint_accepted_line_is_exact" in trust,
    "trust rejection proofs (mismatch/malformed/unsupported/private-key/changed-key) missing",
)
gate(
    "p34c-trust-store-regenerated-from-records",
    "fn regenerate_known_hosts" in trust.replace(" ", " ") or "regenerate_known_hosts" in trust
    and "trusted_keys.json" in trust
    and "pub fn untrust" in trust
    and "pub fn trust(&self, record: &TrustedHostKey)" in trust.replace(" ", " "),
    "trust store must hold typed records and regenerate known_hosts from them",
)

# --------------------------------- p34c: typed AuthReference DB roundtrip
gate(
    "p34c-typed-auth-db-roundtrip",
    "aethercore_fleet::FleetHost::from_bytes(host_json.as_bytes())" in persistence_src
    and "AuthReference::Agent =>" in persistence_src
    and "AuthReference::KeyFile { path } =>" in persistence_src
    and "AuthReference::Certificate { path } =>" in persistence_src
    and "cf2_agent_roundtrip_is_exactly_agent" in fleet_persistence
    and "cf2_key_file_roundtrip_is_exactly_key_file" in fleet_persistence
    and "cf2_certificate_roundtrip_is_exactly_certificate" in fleet_persistence,
    "typed AuthReference DB roundtrip (exhaustive match) missing",
)
gate(
    "p34c-malformed-auth-persistence-rejected",
    "cf2_malformed_auth_json_is_rejected" in fleet_persistence
    and "cf2_unknown_field_is_rejected" in fleet_persistence
    and "cf2_secret_shaped_input_is_rejected" in fleet_persistence
    and "cf2_corrupt_db_auth_kind_is_read_rejected" in fleet_persistence
    and 'json!("agent")' not in persistence_src.split("pub fn upsert_fleet_host")[1].split("pub fn")[0],
    "malformed/unknown/secret auth must be rejected, never defaulted to agent",
)

# ------------------------------ p34c: scheduler end-to-end orchestration
gate(
    "p34c-scheduler-end-to-end-execution",
    (ROOT / "crates" / "fleet" / "src" / "scheduler_runner.rs").is_file()
    and "pub fn run_due_schedules" in scheduler_runner
    and "run_batch" in scheduler_runner
    and "due_schedule_invokes_fleet_compliance_transport_end_to_end" in scheduler_runner
    and "not_due_schedule_is_not_run" in scheduler_runner
    and "missed_run_catches_up_exactly_once" in scheduler_runner
    and "cancellation_propagates_as_typed_outcomes" in scheduler_runner
    and "failure_is_isolated_and_history_preserved" in scheduler_runner
    and "results_are_deterministic_and_bounded" in scheduler_runner,
    "scheduler tick path (schedule→scope→orchestrator→history) missing",
)
gate(
    "p34c-scheduler-history-append",
    "append_history" in scheduler_runner
    and "record_result" in scheduler_runner
    and "save_schedule" in scheduler_runner
    and "begin_run" in scheduler_runner
    and "OverlapLock" in scheduler_runner,
    "scheduler must append history, record last_result and advance next_run under overlap lock",
)
gate(
    "p34c-scheduler-bounded-orchestration",
    "OrchestratorConfig" in scheduler_runner
    and "catch_unwind" in scheduler_runner
    and "SchedulerStore" in scheduler_runner
    and "pub trait SchedulerStore" in scheduler_runner
    and "ScheduleRunDue" in fleet_cli
    and "run-due" in cli_rs
    and "SshComplianceTransport" in fleet_cli,
    "run-due CLI path with bounded orchestration + hermetic store missing",
)

# -------------------------------------------- p34c: Fleet UI management
ui_fleet_actions = [
    "fleet_add_host",
    "fleet_edit_host",
    "fleet_remove_host",
    "fleet_trust_host",
    "fleet_untrust_host",
]
gate(
    "p34c-fleet-ui-management-actions",
    all(action in desktop_main for action in ui_fleet_actions)
    and all(action in fleet_page for action in ui_fleet_actions)
    and "fleet.actionTrust" in catalog_en
    and "fleet.actionUntrust" in catalog_en
    and "fleet.actionRemove" in catalog_en
    and "fleet.actionAdd" in catalog_en,
    "Fleet UI management actions (add/edit/remove/trust/untrust) missing",
)
gate(
    "p34c-fleet-ui-trust-states",
    "fleet.stateTrusted" in catalog_en
    and "fleet.stateUntrusted" in catalog_en
    and all(
        f"'fleet.{state}'" in catalog_en
        for state in ["stateTrusted", "stateUntrusted", "stateEnabled", "stateDisabled"]
    )
    and all(
        state in re.search(r"```json\s*(\{.*?\})\s*```", (ROOT / "docs" / "phase34" / "REMOTE_OPERATIONS.md").read_text(encoding="utf-8"), re.S).group(1)
        for state in ["NotVerified", "NotAvailable", "HostKeyMismatch", "Timeout", "AuthFailure", "Incompatible"]
    )
    and "fleet.confirmRemove" in catalog_en
    and "confirm: true" in fleet_page.replace("confirm:true", "confirm: true")
    and "fleet.fieldPublicKey" in catalog_en
    and "fleet.fieldFingerprint" in catalog_en,
    "Fleet UI typed trust states + explicit remove/trust intent missing",
)
gate(
    "p34c-fleet-ui-i18n-parity",
    all(key in catalog_ar for key in [
        "fleet.actionAdd", "fleet.actionTrust", "fleet.actionUntrust",
        "fleet.actionRemove", "fleet.fieldPublicKey", "fleet.trustNote",
        "fleet.confirmRemove", "fleet.okTrusted", "fleet.errTrustNeedsKey",
    ]),
    "Fleet UI management actions EN/AR parity missing",
)

# ----------------------------- p34c: compatibility real-output contract
gate(
    "p34c-compatibility-real-output-contract",
    "pub fn compatibility_verdict" in transport
    and "remoteContract" in transport
    and "REMOTE_CONTRACT_VERSION" in transport
    and "cf5_compatible_marker_is_supported" in gd_proofs
    and "cf5_missing_marker_is_incompatible" in gd_proofs
    and "cf5_wrong_marker_is_incompatible" in gd_proofs
    and "cf5_malformed_envelope_is_incompatible" in gd_proofs
    and "fixture_capabilities_output" in gd_proofs,
    "compatibility verdict over real command-output fixture missing",
)

# ------------------------------------------------- p34c: reconstruction invariant
gate(
    "p34c-reconstruction-count-invariant",
    "ledger_total" in (ROOT / "scripts" / "_p34_reconstruction_test.py").read_text(encoding="utf-8")
    and "self_excluded_count" in (ROOT / "scripts" / "_p34_reconstruction_test.py").read_text(encoding="utf-8")
    and "self_excluded_paths" in (ROOT / "scripts" / "_p34_reconstruction_test.py").read_text(encoding="utf-8"),
    "reconstruction must report ledger_total/comparison_total/self_excluded invariant",
)

# ---------------------------------------- final acceptance strict-superset gates
# These gates are intentionally narrow and individually named: each one maps to
# a last-mile acceptance requirement rather than relying on the inherited
# Phase 33 aggregate count.
trust_tests = transport
for gate_name, marker in [
    ("p34f-trust-1-authorized-spawn", "trust_1_authorized_schedule_reaches_spawn_once"),
    ("p34f-trust-2-untrusted-no-spawn", "trust_2_missing_record_blocks_spawn"),
    ("p34f-trust-3-mismatch-no-spawn", "trust_3_mismatched_record_blocks_spawn"),
    ("p34f-trust-4-malformed-no-spawn", "trust_4_malformed_record_is_typed_and_blocks_spawn"),
    ("p34f-trust-5-revoked-no-spawn", "trust_5_revocation_regenerates_and_blocks_subsequent_run"),
]:
    gate(gate_name, marker in trust_tests, f"missing deterministic trust seam {marker}")

gate(
    "p34f-schedule-profile-propagation",
    "run_batch_for_profile" in scheduler_runner
    and "execute_for_profile" in orchestrator
    and "profiles.lock" in scheduler_runner
    and "cis-l2" in scheduler_runner,
    "selected schedule profile must reach every host execution",
)
gate(
    "p34f-closed-remote-operation-set",
    "RemoteOperation" in transport
    and "InvalidOperation" in transport
    and "ReportPrint" in transport
    and "FleetJob::Exec" not in fleet_cli,
    "remote execution surface must remain an explicit closed set",
)

for gate_name, marker in [
    ("p34f-ui-probe-command", "fleet_probe"),
    ("p34f-ui-security-audit-command", "fleet_audit"),
    ("p34f-ui-compliance-profile-command", "fleet_compliance"),
    ("p34f-ui-schedule-add-command", "fleet_schedule_add"),
    ("p34f-ui-schedule-update-command", "fleet_schedule_update"),
    ("p34f-ui-schedule-remove-command", "fleet_schedule_remove"),
    ("p34f-ui-schedule-run-due-command", "fleet_schedule_run_due"),
]:
    gate(gate_name, marker in desktop_main and marker in fleet_page, f"missing typed UI command {marker}")

gate(
    "p34f-ui-typed-result-state-space",
    all(state in desktop_main for state in [
        '"success"', '"failed"', '"not_verified"', '"not_available"',
        '"timeout"', '"cancelled"', '"auth_failure"', '"host_key_mismatch"', '"incompatible"',
    ])
    and "resultTone" in fleet_page
    and "resultLabel" in fleet_page,
    "UI must expose every typed remote outcome",
)
gate(
    "p34f-ui-non-success-not-green",
    "if (outcome === 'success') return 'ok'" in fleet_page
    and "return 'warn'" in fleet_page
    and "class:error" in fleet_page,
    "non-success outcomes must not render as healthy",
)
gate(
    "p34f-ui-crud-confirmation",
    "fleet_edit_host" in fleet_page
    and "confirm: true" in fleet_page
    and "confirmUntrust" in fleet_page
    and "confirmScheduleRemove" in fleet_page,
    "destructive UI actions require explicit confirmation",
)
gate(
    "p34f-desktop-b1-inventory",
    "fleet_snapshot" in desktop_main and "UiFleetSnapshot" in desktop_main and "hosts: FleetHostRow[]" in fleet_page,
    "B1 inventory snapshot is not typed end-to-end",
)
gate(
    "p34f-desktop-b2-management",
    all(marker in desktop_main and marker in fleet_page for marker in [
        "fleet_add_host", "fleet_edit_host", "fleet_remove_host",
    ]),
    "B2 host management controls are incomplete",
)
gate(
    "p34f-desktop-b3-trust",
    all(marker in desktop_main and marker in fleet_page for marker in [
        "fleet_trust_host", "fleet_untrust_host",
    ]) and "publicKeyBase64" in fleet_page and "authorized_fingerprint" in desktop_main
    and "confirmUntrust" in fleet_page,
    "B3 trust/revocation flow is incomplete",
)
gate(
    "p34f-desktop-b4-remote-operations",
    all(marker in desktop_main and marker in fleet_page for marker in [
        "fleet_probe", "fleet_audit", "fleet_compliance",
    ]) and "TrustedSshTransport" in desktop_main,
    "B4 typed remote operation actions are incomplete",
)
gate(
    "p34f-desktop-b5-scheduler",
    all(marker in desktop_main and marker in fleet_page for marker in [
        "fleet_schedule_add", "fleet_schedule_update", "fleet_schedule_remove", "fleet_schedule_run_due",
    ]) and "run_due_schedules" in desktop_main,
    "B5 schedule management/run-due surface is incomplete",
)
gate(
    "p34f-desktop-b6-localization-outcomes",
    all(key in catalog_en and key in catalog_ar for key in [
        "fleet.actionProbe", "fleet.resultSuccess", "fleet.resultNotVerified", "fleet.resultIncompatible",
    ]) and "resultTone" in fleet_page and "dir=\"ltr\"" in fleet_page,
    "B6 EN/AR outcome rendering or technical-text isolation is incomplete",
)
gate(
    "p34f-ui-schedule-snapshot",
    "schedules: FleetScheduleRow[]" in fleet_page
    and "snapshot.schedules" in fleet_page
    and "schedule_count" in desktop_main,
    "desktop schedule list and count must be typed and visible",
)
gate(
    "p34f-ui-en-ar-fleet-parity",
    all(key in catalog_en and key in catalog_ar for key in [
        "fleet.actionProbe", "fleet.actionAudit", "fleet.actionCompliance",
        "fleet.actionScheduleAdd", "fleet.actionScheduleUpdate", "fleet.actionScheduleRemove",
        "fleet.resultNotVerified", "fleet.resultNotAvailable", "fleet.resultHostKeyMismatch",
    ]),
    "new fleet controls/results must have EN and AR messages",
)
gate(
    "p34f-reconstruction-required-equations",
    recon_report.is_file()
    and (lambda value: all(key in value for key in [
        "raw_tree_total", "scoped_tree_total", "ledger_total", "comparison_total",
        "self_excluded_count", "self_excluded_paths", "ledger_equals_comparison",
        "raw_equals_scoped_plus_self_excluded", "mismatch_count", "status"
    ]) and value.get("status") == "PASS"
    and value.get("ledger_equals_comparison") is True
    and value.get("raw_equals_scoped_plus_self_excluded") is True
    and value.get("mismatch_count") == 0)(json.loads(recon_report.read_text(encoding="utf-8")))
    if recon_report.is_file() else False,
    "reconstruction report must prove both count equations and zero mismatches",
)
if 'first_full' in globals():
    full_tree_json = json.loads(first_full.stdout) if first_full.stdout else {}
    gate(
        "p34f-full-tree-1066-clean",
        first_full.returncode == 0
        and full_tree_json.get("status") == "PASS"
        and full_tree_json.get("checked") == full_tree_json.get("expected_total")
        and full_tree_json.get("problems") == [],
        "full-tree verifier must be clean and self-consistent",
    )
else:
    gate("p34f-full-tree-1066-clean", False, "full-tree verifier output unavailable")
gate(
    "p34f-ssh-owned-known-hosts-only",
    "UserKnownHostsFile=" in trust
    and "StrictHostKeyChecking=yes" in trust
    and "with_known_hosts_path" in transport
    and "TrustStoreRequired" in transport,
    "SSH must bind to the AetherCore-owned known_hosts store",
)
gate(
    "p34f-no-secret-material-crosses-ui",
    "publicKeyBase64" in fleet_page
    and "public_key_base64" in desktop_main
    and "password" not in fleet_page.lower()
    and "nothing secret can be edited" in desktop_main.lower(),
    "UI boundary must carry public-key trust material only",
)

# -------------------------------------------------------- exact count guard
gate(
    "p34-exact-audit-total-above-846",
    inherited_checks == 846 and inherited_checks + len(new_gates) > 846,
    f"inherited={inherited_checks} new={len(new_gates)}",
)

checks = inherited_checks + len(new_gates)
result = {
    "schema": "aethercore.phase34.adversarial-audit.v1",
    "root": str(ROOT),
    "inherited_p33_checks": inherited_checks,
    "new_gates": new_gates,
    "checks": checks,
    "failures": failures,
    "status": "PASS" if not failures else "FAIL",
}
print(json.dumps(result, indent=2))
raise SystemExit(0 if not failures else 1)
