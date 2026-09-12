# P59 ITEM 1.A — the reader census

`DBT-P58-005`. Every place under `scripts/` and `tools/` that reads a file, a
command's output or an API response and **continues on failure**.

Host: the Mac at `/Users/hasanalaaa/dev/aethercore`, 2026-09-12, at `f8fe3b8`.
Committed before any repair, as the brief requires: this table is the
deliverable even if nothing is fixed yet.

## Classes

- **A** — failing open is correct here. Either absence is a state the code is
  deliberately testing for, or the empty fallback itself fails the gate.
- **B** — a read failure silently produces a **pass**. Must be fixed.
- **C** — cannot tell from the code.

A **B** row is further marked:

- **live** — *measured*: the gate's verdict is unchanged with that source
  blinded, so it reports a pass on a file it never opened;
- **latent** — the reader cannot distinguish "absent" from "unreadable", and
  today an unrelated second read of the same file happens to catch it. This is
  exactly the state the eight P58 readers were in the day before the workflows
  moved. A latent B is a live B waiting for a rename.

## How the classification was measured, not argued

`blind.py` (session scratch, not committed) makes one source return `""` from
every read — precisely what the fail-open readers do — and re-runs the gate.
If stdout and the exit code are byte-identical to the baseline, that source is
invisible to the gate. Every **live** mark below is that measurement.

The flagship result was reproduced without the harness, on the real tree, by
moving the file out and back:

```
$ python3 scripts/phase16-ga-audit.py            # baseline
exit=0   {"ok": true, "checks": 42, "failed": []}

$ mv scripts/phase13-fault-injection.ps1 /tmp/ && python3 scripts/phase16-ga-audit.py
exit=0   {"ok": true, "checks": 42, "failed": []}      # byte-identical
```

**A GA gate deletes one of the five files it audits and still reports 42/42
green.** `git status` clean afterwards; the file was moved back.

---

## The table

| `file:line` | shape | class | what it means |
|---|---|---|---|
| `scripts/phase16-ga-audit.py:18-20` | `def text(path)` → `p.read_text() if p.exists() else ''` | **B — live** | Three of the five sources concatenated at `:59` can vanish with the report unchanged at `ok: true, 42 checks, 0 failed`: `scripts/phase13-fault-injection.ps1`, `scripts/phase14-scheduler-fault-injection.ps1`, `scripts/run-ipc-fuzz.ps1`. The check they feed, `phase16_no_short_name_exact_false_pass`, is an **absence** assertion — `'-- --exact' not in critical` — and its presence half is satisfied by the other two files |
| `scripts/phase15-security-audit.py:17-19` | `def read(rel)` → same | **B — live** | Six sources blind-pass: `services/maintenance-service/src/composition.rs`, `services/maintenance-service/Cargo.toml`, `services/maintenance-service/src/protocol.rs`, `crates/security/src/lib.rs`, `crates/persistence/src/lib.rs`, `apps/ui/src/lib/contracts.ts` |
| `scripts/enterprise-adversarial-audit.py:24-26`, consumed at `:214-215` | `read()` over an `rglob` of `apps/ui/src` | **B — live** | `ui_no_any_escape_hatches` is a pure absence assertion (`" as any" not in ui_text` …) over 84 concatenated UI sources. Any of them can return `""` and the check still passes. 157 sources in total are invisible to this audit |
| `scripts/phase13-reliability-audit.py:21-23` | `def read(rel)` → same | **B — live** | `services/maintenance-service/src/protocol.rs` blind-passes |
| `scripts/phase35-adversarial-audit.py:26-29` | `try: read_text() except (OSError, UnicodeDecodeError): return ""` | **B — live** | Four blind-pass: `crates/update-engine/src/manifest.rs`, `apps/desktop/Cargo.toml`, `apps/desktop/tauri.conf.json`, `Cargo.toml` |
| `scripts/static_validate.py:585` | inline `… if …exists() else ""` | **B — live** | `scripts/setup-and-run.ps1` blind-passes |
| `scripts/static_validate.py:1393` | inline | **B — live** | `scripts/phase10-architecture-audit.ps1` blind-passes |
| `scripts/phase30-adversarial-audit.py:222`, `:238` | `if p.is_file() and sha256(p) != ledger[rel]` | **B — live** | A ledger entry whose file is **absent** is skipped, not flagged. `p30-fulltree-spot-hash-ok` passes on a tree missing files the ledger names — `.github/workflows/ci.yml`, `fuzz.yml`, `release.yml` and `dependabot.yml` are in that ledger and are absent from the workspace today, and the gate says nothing |
| `scripts/phase29-adversarial-audit.py:256` | `json.loads(…) if (patch_dir/"MANIFEST.json").exists() else {}` | **B — live** | With the manifest absent, `m_sha` is `{}`, `inconsistent` is `[]`, and `p29-manifest-fulltree-consistency` passes |
| `scripts/check-dependency-freeze.py:23-28` | `expected_pnpm()` → `except Exception: return None` | **B — latent** | An unreadable `package.json` makes the expectation `None`; if the freeze metadata also lacks `pnpm`, `checks["pnpm_pin"]` is `None == None` → **True**. The file's own docstring is "Fail-closed" |
| `scripts/static_validate.py:584, 625, 686, 687, 724, 1285, 1292, 1392, 1497, 1498, 1499, 1519, 1520, 1521, 1596-1604, 1608, 1609, 1610, 1655, 1656, 1657, 1692, 1725, 1726, 1776, 1777` | inline `… if …exists() else ""` (33 sites) | **B — latent** | Each blinds to `""`; each is caught today only because the same file is also read by a bare `read_text()` elsewhere in the same 2,006-line script. Nothing enforces that pairing |
| `scripts/phase14-scheduler-audit.py:17-19` | `def read(rel)` → same | **B — latent** | 32 sources, none currently invisible |
| `scripts/zenith-adversarial-audit.py:31-33` | `def read(rel)` → same | **B — latent** | 16 sources, none currently invisible — and this gate is **green** (exit 0), so a future invisible source would be a silent green |
| `scripts/zenith-recursive-audit.py:24-26` | `def read(rel)` → same | **B — latent** | 39 sources. A full sweep costs 23 s per run and was not affordable in session; the four sources feeding its pure-absence assertions (`streaming.rs`, `crates/ipc/src/windows_impl.rs`, `apps/install-hardener/src/main.rs`, `crates/windows-foundation/src/lib.rs`) were blinded individually and all four were **detected** |
| `scripts/phase19-windows-repair-audit.py:15-17` | `def text(rel)` → same | **B — latent** | Not workflow-aware and not in P58's ten; same shape |
| `scripts/omega-evidence.py:288`, `:408` | `except Exception: value = {"approved": False, …}` | **A** | The fallback is itself the blocking verdict — `OMEGA-RB-001` / `-002` are raised on it |
| `scripts/omega-evidence.py:421`, `:438` | `except Exception: payload = {}` | **A** | The blocker is gated on the child's `exit_code`, not on the payload, so an unreadable payload cannot buy a pass |
| `scripts/omega-evidence.py:55-59` | `trailing_json` → `except Exception: pass` | **A** | Scanning candidate `{` offsets; a parse failure per offset is the loop's normal control flow |
| `scripts/phase19-windows-repair-audit.py:75, 79` | `except Exception: q={}; qids=set()` | **A** | The consumer is `all(id in qids …)` over an empty set → False → the gate fails |
| `scripts/omega-unsafe-inventory.py:43` | `json.loads(POLICY) if exists else {"contracts":{}}` | **A** | An empty policy makes every production `unsafe` file unclassified → non-empty `unclassified` → exit non-zero |
| `scripts/sigma-evidence-integrity-test.py:58` | `json.loads(…) if evidence.is_file() else {}` | **A** | The assertion is `…get("ok") is False`; on `{}` it is `None`, so the check fails |
| `scripts/phase27-adversarial-audit.py:83, 84` | inline `… else ""` | **A** | `:86-87` are explicit `p27-macos/linux-provider-file-exists` presence gates on the same paths |
| `scripts/phase32-adversarial-audit.py:233`, `:337` | inline `… else ""` | **A** | Both consumers are presence assertions (`"gd5_tampered_db_refused_fail_closed" in golden_rs`, `bool(sec_cli) and …`), which `""` fails |
| `scripts/phase33-adversarial-audit.py:73`, `:76` | inline `… else ""` | **A** | `p33-01-compliance-module-exists` gates `compliance_path.is_file()` directly |
| `scripts/phase34-adversarial-audit.py:455` | inline `… else ""` | **A** | The same expression requires a presence token (`"Authoritative archive hash: see …" in mdr`), so the absence half cannot pass alone |
| `scripts/phase31-adversarial-audit.py:104` | `README.md … else ""` | **A** | Consumers are presence assertions on README content |
| `scripts/phase29-adversarial-audit.py:263-267` | `if p.is_file():` skip in the spot-hash loop | **A** | The verdict at `:271` is hardcoded `True` (superseded by P30). The loop's result is computed and discarded — dead code, not a false pass |
| `scripts/enterprise-adversarial-audit.py:150, 224, 233, 242` | `read_text(errors="ignore")` | **A** | Only undecodable **bytes** are dropped, not the file; the tokens scanned for are ASCII |
| `scripts/zenith-recursive-audit.py:421` | `read_text(errors="ignore")` over `rglob` | **A** | Same |
| `scripts/phase17-intelligence-audit.py:149, 160`; `scripts/phase17_1-integrity-audit.py:153, 163` | `except Exception as exc: add(…, "FAIL", …)` | **A** | The handler records a **FAIL** naming the exception |
| `scripts/sigma-master-source-closure.py:35` | `except subprocess.TimeoutExpired` → result dict | **A** | The timeout is recorded with a non-zero exit code in the result |
| `scripts/sigma-master-ui-state-tests.py:94` | `except json.JSONDecodeError:` → synthesised check | **A** | The synthesised check is a failing one |
| `tools/lint_fuzz_workflow.py:12-13` | bare `WORKFLOW.read_text()` | **A** | Raises. P58 repaired this one to the right shape |
| `scripts/build-release.ps1:40`, `:128`; `scripts/audit-dependencies.ps1:14`; `scripts/generate-sbom.ps1:18` | `& cmd 2>$null \| Out-String` inside `try {} catch {}` | **A** | Each guards on `$LASTEXITCODE -eq 0` and falls through to an explicit `throw` or an explicit fallback file |
| `scripts/*.ps1` — 76 `-ErrorAction SilentlyContinue` occurrences across 35 files | `Get-Command`/`Get-Service`/`Remove-Item … -ErrorAction SilentlyContinue` | **A** | Every one is either a presence probe whose negative branch `throw`s (`build-installer.ps1:23`, `sign-burn-bundle.ps1:22`, `omega-coverage.ps1:8`, `phase17/18-windows-qualification.ps1:11-12`, `verify-installer-security.ps1:188`) or best-effort cleanup in a `finally` |
| `scripts/build-release.ps1:127-131` | `$sourceCommit = $null` when `git rev-parse` fails | **C** | The null reaches the release manifest. Whether a consumer asserts a commit is present could not be settled on this host — the manifest is produced only by a Windows release build |

### Counts

| | count | where |
|---|---|---|
| sites examined | **159** | `scripts/`, `tools/`, `.github/workflows/` |
| **B — live** | **10** | 8 scripts: `phase16-ga-audit`, `phase15-security-audit`, `enterprise-adversarial-audit`, `phase13-reliability-audit`, `phase35-adversarial-audit`, `static_validate` (2), `phase30-adversarial-audit` (2), `phase29-adversarial-audit` |
| **B — latent** | **38** | 6 scripts; 33 of the 38 are the inline expressions in `static_validate.py` |
| **B total** | **48** | |
| A | **110** | 76 of them the PowerShell `-ErrorAction SilentlyContinue` probes |
| C | **1** | `build-release.ps1:127-131` |

Shell and Rust, for completeness: no `\|\| true` and no tested `2>/dev/null`
exist under `scripts/`, `tools/` or `.github/workflows/` — `grep -rnE '\|\| true\|2>/dev/null'` returns
nothing. The two `.unwrap_or_default()` calls in the Rust tools
(`tools/p39-probes/src/p39_pipe_attack.rs:46, 87`) are on an environment
variable and on an absent error field, not on a read.

---

## Why the eight P58 repairs did not close this

`a8946bb` fixed *where* eight readers looked. It did not change *what they do
when the look fails*. Every one of them still has the shape

```python
return p.read_text(encoding='utf-8') if p.exists() else ''
```

which is the defect P58 named and did not remove: **a reader that cannot
distinguish "absent" from "unreadable"**. The ten live B rows above are the
proof that this is not hypothetical — they are gates reporting green today on
files they never open.
