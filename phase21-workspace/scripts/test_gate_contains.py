#!/usr/bin/env python3
"""The gate token comparison ignores whitespace and nothing else. `DBT-P61-001`.

81 of the 108 failing checks were the gates spelling a construct that is in the
tree - `hardware_gate:IsolationGate` against a `rustfmt`-clean
`hardware_gate: IsolationGate`. `gate_reader.contains` drops whitespace from both
sides, once, for all six token gates.

A comparison that is widened to make failures go away is worth nothing unless it
can still fail. Both halves are asserted here against the **real tree**, through
the gate's **own** helper and the gate's **own** loaded sources:

  present    a token whose only difference from the tree is spacing must match
  absent     a token naming something that is not in the tree must NOT match,
             and neither must one whose characters are merely reordered

The absent case is not invented: `cancel_registered_io` is the identifier
`482d480` deliberately removed when Windows IPC moved to overlapped I/O, and it
is one of the 22 checks that still fail after this change - see
`docs/phase62/P62-GATE-CLASSIFICATION.md`.

Run: `python3 scripts/test_gate_contains.py`
"""
from __future__ import annotations

import ast
import contextlib
import io
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from gate_reader import SourceReader, contains, count, position  # noqa: E402

read = SourceReader(ROOT).read

failures: list[str] = []


def expect(label: str, actual: bool, wanted: bool) -> None:
    ok = actual is wanted
    print(f"{'PASS' if ok else 'FAIL'}  {label}  -> {actual}")
    if not ok:
        failures.append(f"{label}: expected {wanted}, got {actual}")


def run_gate(filename: str) -> dict:
    """Exec a gate and hand back its namespace - its helpers and its sources."""
    script = ROOT / "scripts" / filename
    namespace = {"__file__": str(script), "__name__": "__main__"}
    argv, sys.argv = sys.argv, [script.name]
    try:
        code = compile(script.read_text(encoding="utf-8"), str(script), "exec")
        with contextlib.redirect_stdout(io.StringIO()):
            exec(code, namespace)  # noqa: S102 - running the gate is the point
    except SystemExit:
        pass
    finally:
        sys.argv = argv
    return namespace


def phase17_predicates() -> dict:
    """Load the real static predicates without running the audit's Rust/UI commands."""
    path = ROOT / "scripts" / "phase17-intelligence-audit.py"
    tree = ast.parse(path.read_text(encoding="utf-8"))
    names = {"diagnostic_fault_contract", "remediation_description_keys"}
    definitions = [node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name in names]
    assert {node.name for node in definitions} == names
    namespace = {"re": re, "contains": contains}
    exec(compile(ast.Module(body=definitions, type_ignores=[]), str(path), "exec"), namespace)
    return namespace


def main() -> int:
    diag = read("crates/diagnostic-engine/src/lib.rs")
    ipc = read("crates/ipc/src/windows_impl.rs")

    # 1. The widening does real work: the gate's spelling is not in the tree
    #    literally, and is once the spaces are gone.
    expect("literal 'hardware_gate:IsolationGate' in diagnostic-engine",
           "hardware_gate:IsolationGate" in diag, False)
    expect("contains 'hardware_gate:IsolationGate'",
           contains(diag, "hardware_gate:IsolationGate"), True)

    # 2. It is not a blanket pass. An identifier that is not in the tree stays
    #    absent, spaces or no spaces.
    expect("contains 'hardware_gate:QuarantineGate' (invented)",
           contains(diag, "hardware_gate: QuarantineGate"), False)
    expect("contains 'cancel_registered_io(&self.peer_reader)' (removed by 482d480)",
           contains(ipc, "cancel_registered_io(&self.peer_reader)"), False)

    # 3. Order still matters - squashing removes whitespace, it does not sort.
    expect("contains 'IsolationGate:hardware_gate' (reordered)",
           contains(diag, "IsolationGate: hardware_gate"), False)

    # 4. A trailing comma is not whitespace. This is why the classification
    #    separates ABSENT from FORMAT and why those checks were read by hand
    #    rather than swept up by this comparison.
    expect("contains 'Active, Committed, Revoked }' spelled without the last comma",
           contains("enum E {\n    Active,\n    Committed,\n    Revoked,\n}",
                    "{ Active, Committed, Revoked }"), False)

    # 5. `count` and `position` carry the same widening and the same limit.
    #    `else { break; };` is a let-else body; the gate used to spell it
    #    `else { break };`, which is not Rust and is in no tree.
    streaming = read("services/maintenance-service/src/streaming.rs")
    expect("literal count of 'else { break; };' in streaming.rs",
           streaming.count("else { break; };") > 0, False)
    expect("count(streaming, 'else { break; };') >= 5",
           count(streaming, "else { break; };") >= 5, True)
    expect("count(streaming, 'else { continue; };') (invented)",
           count(streaming, "else { continue; };") > 0, False)
    #    `position` is a SQUASHED index: never a line number, but monotone, which
    #    is the only property the ordering checks use.
    text = "alpha\n  beta\n    gamma\n"
    expect("position orders 'alpha' before 'gamma'",
           0 <= position(text, "alpha") < position(text, "gamma"), True)
    expect("position of an absent token is -1",
           position(text, "delta") == -1, True)

    # 6. End to end: the gate's own `has`, over the source the gate itself read.
    gate = run_gate("phase13-reliability-audit.py")
    gate_has, gate_diag = gate["has"], gate["diag"]
    expect("phase13 gate has(diag, 'hardware_gate:IsolationGate')",
           gate_has(gate_diag, "hardware_gate:IsolationGate"), True)
    expect("phase13 gate has(diag, 'hardware_gate:QuarantineGate')",
           gate_has(gate_diag, "hardware_gate:QuarantineGate"), False)
    expect("phase13 gate has(diag, present, absent) - one absent token is enough",
           gate_has(gate_diag, "hardware_gate:IsolationGate",
                    "hardware_gate:QuarantineGate"), False)

    # P85: execute the recursive gate's actual predicates against its loaded
    # child runner, then break one safety property at a time. No source is edited.
    repair_gate = run_gate("zenith-recursive-audit.py")
    runner = repair_gate["repair_process"]
    fallible = repair_gate["repair_readers_are_fallible"]
    ownership = repair_gate["repair_timeout_preserves_ownership"]
    limited_dism = repair_gate["repair_mutation_uses_limited_dism"]
    win, api = repair_gate["repair_windows"], repair_gate["repair_dism_api"]
    for name in ("repair_pipe_readers_are_fallible", "repair_timeout_joins_pipe_readers",
                 "maintenance_service_does_not_write_restrict_arbitrary_windows_mutations"):
        expect(f"P85 real recursive gate {name}", repair_gate["checks"][name]["ok"], True)
    for stream in ("stdout", "stderr"):
        start = runner.index(f"let {stream}_thread =")
        broken = runner[:start] + runner[start:].replace("match thread::Builder::new()", "match thread::spawn", 1)
        expect(f"P85 {stream} reader lost fallible Builder", fallible(broken), False)
    for branch, end in (("if !mutating && timed_out {", "if !mutating && stopped {"),
                        ("if !mutating && stopped {", "thread::sleep(")):
        start, finish = runner.index(branch), runner.index(end, runner.index(branch) + len(branch))
        for stream in ("stdout", "stderr"):
            part = runner[start:finish]
            lost = f"let _ = {stream}_thread.join();"
            assert lost in part
            broken = runner[:start] + part.replace(lost, "", 1) + runner[finish:]
            expect(f"P85 {branch} lost {stream} join (other joins remain)", ownership(broken), False)
        expect(f"P85 {branch} allowed mutating kill", ownership(runner.replace(branch, branch.replace("!mutating && ", ""), 1)), False)
    expect("P85 unguarded mutating kill", ownership(runner.replace("if mutating {", "if mutating { let _ = child.kill();", 1)), False)
    expect("P85 creation failure killed mutating child", ownership(runner.replace("if !mutating {", "if true {", 1)), False)
    expect("P85 lost polling ownership wait", ownership(runner.replace("// Keep ownership until this exact child exits, even if polling its handle failed.\n                let _ = wait_for_child(&mut child);", "", 1)), False)
    expect("P85 removed actual owned-child wait helper", ownership(runner.replace("fn wait_for_child(", "fn missing_wait_for_child(", 1)), False)
    expect("P85 wait error fabricated exit", ownership(runner.replace("Err(_) => thread::sleep(Duration::from_millis(25))", "Err(_) => return fabricated_status", 1)), False)
    expect("P85 timeout omitted owned-child drain", ownership(runner.replace("let _ = wait_for_child(&mut child);", "", 4)), False)
    for stream in ("stdout", "stderr"):
        lost = f'joined_stream({stream}_thread.join(), "{stream}"'
        assert lost in runner
        expect(f"P85 normal completion lost {stream} join", ownership(runner.replace(lost, f'joined_stream({stream}_thread, "{stream}"', 1)), False)
    expect("P85 stderr creation failure lost stdout join", fallible(runner.replace("let _ = stdout_thread.join();", "", 1)), False)
    expect("P85 lost typed mutating timeout", ownership(runner.replace("RepairError::RepairTimedOut", "RepairError::Cancelled")), False)
    expect("P85 LimitAccess disabled", limited_dism(win, api.replace("1, // TRUE: LimitAccess", "0, // TRUE: LimitAccess")), False)
    expect("P85 missing DISM cancel event", limited_dism(win, api.replace("watcher.event()", "0")), False)
    expect("P85 missing durable mutation barrier", limited_dism(win.replace("begin_mutation()?", ""), api), False)

    # Phase17: the serialized collector kind is matched by its explicit Debug name;
    # remediation descriptions exist only for codes admitted by the action match.
    predicates = phase17_predicates()
    normalization = read("crates/pc-intelligence/src/normalize.rs")
    typed_faults = predicates["diagnostic_fault_contract"]
    expect("P17 actual producer-to-normalizer fault contract", typed_faults(normalization, diag), True)
    for token in ('"PermissionDenied" => CollectorState::PermissionDenied',
                  '_ => CollectorState::Failed'):
        assert token in normalization
        expect(f"P17 removed normalizer {token}", typed_faults(normalization.replace(token, "", 1), diag), False)
    expect("P17 permission denial incorrectly normalized as failure",
           typed_faults(normalization.replace('"PermissionDenied" => CollectorState::PermissionDenied',
                                              '"PermissionDenied" => CollectorState::Failed'), diag), False)
    for token in ('kind: format!("{:?}", fault.kind)',
                  'TelemetryError::PermissionDenied(_) => FaultKind::PermissionDenied',
                  'CrashError::PermissionDenied(_) => FaultKind::PermissionDenied'):
        assert token in diag
        expect(f"P17 removed typed producer {token}", typed_faults(normalization, diag.replace(token, "")), False)
    rules = read("crates/pc-intelligence/src/rules.rs")
    descriptions = predicates["remediation_description_keys"]
    keys = descriptions(rules)
    en = set(re.findall(r"^\s*'([^']+)'\s*:", read("apps/ui/src/lib/i18n/catalog.en.ts"), re.M))
    ar = set(re.findall(r"^\s*'([^']+)'\s*:", read("apps/ui/src/lib/i18n/catalog.ar.ts"), re.M))
    expect("P17 actual generated candidate descriptions localized", keys <= en and keys <= ar, True)
    expect("P17 thermal finding without a candidate emits no remediation description",
           "remediation.thermal_trip_exceeded" in keys, False)
    marker = '"RECENT_CRASH_EVIDENCE" => ActionType::ReviewCrashEvidence,'
    assert marker in rules
    admitted = rules.replace(marker, '"THERMAL_TRIP_EXCEEDED" => ActionType::ReviewHardwareError,\n' + marker, 1)
    newly_required = descriptions(admitted)
    expect("P17 actually admitted thermal action requires its missing EN/AR key",
           newly_required <= en and newly_required <= ar, False)
    expect("P17 removed actual candidate catalog key detected",
           keys <= (en - {"remediation.storage_attention"}) and keys <= ar, False)
    rejected = False
    try:
        descriptions(rules.replace('description_key: format!("remediation.{}", finding.code.to_ascii_lowercase())',
                                   'description_key: "unchecked".into()'))
    except ValueError:
        rejected = True
    expect("P17 unrecognized generated-description shape fails closed", rejected, True)

    print()
    if failures:
        print(f"{len(failures)} case(s) failed:")
        for item in failures:
            print(f"  - {item}")
        return 1
    print("gate token comparison: whitespace-insensitive, and still fails on absence")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
