#!/usr/bin/env python3
"""The router is a module tree: assert the gates can read it AND still fail.

`DBT-P63-004`. `services/maintenance-service/src/router.rs` was one 1,868-line
file; eleven gate scripts asked token questions of it. It is a module root beside
`router/*.rs` now, and those gates read the tree through
`SourceReader.read_module`.

A widening that cannot fail is worth nothing - P63's `test_gate_contains.py` is
the shape this follows. Each case below asserts both halves: that a token which
moved into the tree is found, and that a token which is not in the tree is still
absent, through the gates' own reader.

    python3 scripts/test_gate_module_reader.py
"""
from __future__ import annotations

import shutil
import sys
import tempfile
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from gate_reader import SourceReader, contains, count  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
ROUTER = "services/maintenance-service/src/router.rs"

failures: list[str] = []


def case(label: str, got, want) -> None:
    ok = got == want
    print(f"{'PASS' if ok else 'FAIL'}  {label}  -> {got!r}")
    if not ok:
        failures.append(f"{label}: got {got!r}, wanted {want!r}")


reader = SourceReader(ROOT)
root_only = reader.read(ROUTER)
tree = reader.read_module(ROUTER)

case("the tree is strictly larger than the module root", len(tree) > len(root_only), True)
case(
    "every router/*.rs is in the tree",
    all(p.read_text(encoding="utf-8") in tree
        for p in sorted((ROOT / ROUTER[: -len(".rs")]).glob("*.rs"))),
    True,
)
case(
    "a file with no module directory reads exactly as itself",
    reader.read_module("services/maintenance-service/src/streaming.rs")
    == reader.read("services/maintenance-service/src/streaming.rs"),
    True,
)

# The verbs the gates name. Each one moved out of the root, so `read` no longer
# finds it and `read_module` does - which is the whole reason the gates changed.
MOVED = {
    "phase15_security / require_update_broker": "require_update_broker(peer)?",
    "phase15_security / legacy download arm": 'request::Payload::CheckForUpdates(_) => { Err("legacy service-side update download is disabled"',
    "phase9 / principal rebinding": "peer.binding_key()",
    "security-hardening / request id syntax": "is_safe_request_id",
    "phase17 / deep scan verbs": "SealRemediationPlan",
    "phase18 / candidate policy verb": "SetDriverCandidatePolicy",
    "phase29 / signed journal export": "AETHERCORE_EXPORT_KEY",
    "phase31 / export correlation": "build_envelope_with_correlation",
    "phase32 / security audit arm": "request::Payload::RunSecurityAudit(v)",
}
for label, token in MOVED.items():
    case(f"{label}: absent from the module root alone", contains(root_only, token), False)
    case(f"{label}: present in the tree", contains(tree, token), True)

# It can still fail: tokens that name nothing in the tree.
for token in [
    "require_licence_broker(peer)?",
    "request::Payload::RunPaymentAudit(v)",
    "start_with_lease(&principal_key,&v.plan_id,lease)",  # the pre-P64 spelling
]:
    case(f"invented or retired token {token!r} is absent", contains(tree, token), False)

# zenith_recursive counts these exactly, so an exact number is the assertion.
case("leased mutation starts", count(tree, "start_with_lease(principal_key,&v.plan_id,lease)"), 4)
# P84 binds driver discovery to this request's confirmed search scope. Count
# only the two complete call shapes: owner, lease, and the driver scope remain
# mandatory, and the expected four starts do not change.
UNSCOPED_SCAN = "start_scan_with_lease(principal_key,lease)"
SCOPED_SCAN = "start_scan_with_lease(principal_key,lease,search_scope(&request))"


def leased_read_starts(text: str) -> int:
    return count(text, UNSCOPED_SCAN) + count(text, SCOPED_SCAN)


case("leased read starts", leased_read_starts(tree), 4)
case("driver scan binds its leased read to the request scope", count(tree, SCOPED_SCAN), 1)
for label, replacement, expected in [
    ("deleted driver scan", "", 3),
    ("duplicated driver scan", SCOPED_SCAN + SCOPED_SCAN, 5),
    ("driver scan with a different owner", SCOPED_SCAN.replace("principal_key", "other_owner"), 3),
    ("driver scan without its lease", "start_scan_with_lease(principal_key,search_scope(&request))", 3),
    ("driver scan with another lease", SCOPED_SCAN.replace(",lease,", ",other_lease,"), 3),
    ("driver scan with an unconfirmed online scope", SCOPED_SCAN.replace("search_scope(&request)", "SearchScope::Online"), 3),
]:
    # Use the real route's formatted call, not a synthetic copy of the router.
    driver_call = ".start_scan_with_lease(principal_key, lease, search_scope(&request))"
    mutated = tree.replace(driver_call, "." + replacement)
    case(f"{label} still changes the exact count", leased_read_starts(mutated), expected)


# Delete one module from a copy of the tree: its tokens must go with it. A reader
# that survived this would be reading something other than the files.
with tempfile.TemporaryDirectory() as tmp:
    clone = Path(tmp) / "src"
    shutil.copytree(ROOT / "services/maintenance-service/src", clone)
    (clone / "router" / "security_audit.rs").unlink()
    text = "\n".join(
        [(clone / "router.rs").read_text(encoding="utf-8")]
        + [p.read_text(encoding="utf-8") for p in sorted((clone / "router").glob("*.rs"))]
    )
    # The dispatch arm stays in `dispatch.rs` - what leaves with the module is the
    # verb's BODY, which is what `phase32`'s honest-NotAvailable check reads.
    case("the dispatch arm survives a deleted module",
         contains(text, "request::Payload::RunSecurityAudit(v)"), True)
    case("a deleted domain module takes its verb body with it",
         contains(text, '"notAvailable".to_string()'), False)

print()
if failures:
    print(f"{len(failures)} failed")
    for f in failures:
        print("  " + f)
    sys.exit(1)
print("all cases passed")
