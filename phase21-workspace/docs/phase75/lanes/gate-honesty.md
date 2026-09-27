# P75 lane `gate-honesty` (Wave 2) — evidence (ledger `DBT-P65-003`, `DBT-P75-057`…`060`)

Branch `lane/gate-honesty`, from `main` `821163f`. No dependency or lockfile change.

Every fix below is shown failing on a planted regression and passing on the tree. Windows and
POSIX facts were measured on a throwaway push-triggered probe (`probe/p75-gates`, 1–3 min per run)
before any 80-minute pipeline.

| row | gate | was | now | proof |
|---|---|---|---|---|
| `DBT-P75-057` | `static_validate.py` (3 checks), `enterprise-adversarial-audit.py`, `zenith-recursive-audit.py`, `phase10-architecture-audit.ps1` (AMBITION §0) | negative checks that could not fail: a raw `not in` over tokens spelled without rustfmt's spaces (`let _=inner.db.save_…`, `self.emit(owner,&state.snapshot)`, `Ok(file)=>match …`), and a `^` without `(?m)` on `Get-Content -Raw` | gate_reader's whitespace-insensitive `contains` (DBT-P61-001); `(?m)` | each construct planted as rustfmt writes it: the new check fails, the old raw expression on the same text passes. PowerShell on windows-2025 (probe `36240612616`): clean tree passes, a planted `message PlantedDefinition {}` throws "Forbidden Phase 10 pattern detected" |
| `DBT-P75-058` | every CI-wiring check (gate_reader `SourceReader.read`, `static_validate.py`) | asked whether a step's text appeared anywhere in the workflow, so a step disabled with `#` passed | full-line YAML comments are blanked for `.github/workflows/*` | `run: ./scripts/verify-enterprise.ps1 -SkipOnlineSupplyChain` commented out → 8 checks fail (`phase10…phase16` CI release gates); before, all 8 passed |
| `DBT-P75-059` | `static_validate.py` `parse_yaml` | without PyYAML: `{"ok": true, "count": 0}` — a pass that parsed nothing | UNMEASURED: out of the verdict, named in the summary | this Mac (no PyYAML): `"unmeasured": ["parse_yaml"]` |
| `DBT-P65-003` | `ps_marker_scan.py` | inline `if (...) { throw }` counted nowhere | each is an UNMEASURED result with its line | `phase11-design-audit.ps1`: `assertions=41 unmeasured=0` → `assertions=49 unmeasured=8`, exactly :28 :29 :30 :34 :35 :36 :37 :52; all gates: unmeasured 3 → 90 |
| `DBT-P75-060` | CI | the gates' own tests and `ps_marker_scan.py` had no caller | a `gate-self-tests` job runs all eight on `macos-latest` | measured first: Windows breaks `test_gate_readers.py`'s unreadable-source injection and prints non-ASCII to cp1252 (`36240612616`, `36240659611`); Linux lacks `plutil`, which the phase29 chain calls (`36240919751`); macOS: 8/8 pass (`36240971179`) |

## Not done

* `DBT-P63-009` (~244 exact comparisons that pass today): unchanged, as that row decided. The four
  negatives above are the ones that could not fail at all.
* `DBT-P63-014` (the PowerShell chain behind CI step 18): not touched; it needs a Windows session
  per iteration.
* Scripts without helper functions are still not scanned for inline throws; they orchestrate
  (exit codes of the tools they run), and their assertions were not triaged here.

## Local proof (macOS)

`static_validate.py` → `"failed": []`, `"unmeasured": ["parse_yaml"]`; `enterprise-adversarial`,
`zenith-recursive`, `phase13`, `phase14`, `phase15`, `phase16` → `"failed": []`;
`test_gate_readers.py` → all 14 fail closed; `ps_marker_scan.py` → assertions=321 failed=0
unmeasured=90; the eight self-tests pass locally and on `macos-latest`.
