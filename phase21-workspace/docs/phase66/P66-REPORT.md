# P66 — the report

Host: the Mac at `/Users/hasanalaaa/dev/aethercore`. Date: 2026-09-14. Branch
`main` throughout. Every figure names the command or the run that produced it.
Three claims in the brief for this session are contradicted by measurement, and
each is corrected below with the command that did it. Where something is
reasoning rather than measurement it says so.

The short version. **Step 21's wall was not one failure of twenty entries. It
was thirty entries of three different kinds, and the gate was printing twenty of
them.** Sixteen were a real EN/AR parity violation and are translated. Fourteen
were the gate asking the wrong question, and the gate is fixed with a negative
control on every change. `test-phase12-localization.py` goes **30/32 → 34/34**.
**Clippy still has not run on Windows** — the brief says it did; the log says it
did not. Dependabot stays paused.

---

## The three corrections that come first

**1. The failure is 30 entries, not 20.** The brief lists twenty and describes
them as the whole set. The gate's message was
`str(uncovered_native[:20])` — a slice with no total. The list is sorted, so the
truncation fell on `title`:

```
FAILING TOTAL : 30      (the gate printed the first 20)
  11 detail   prose
   5 title    prose   <- entirely invisible in every handoff since P62
  13 result_code       <- 8 visible, 5 truncated away
   1 detail  'installed'
```

The five hidden `title` strings — `Windows Update service`, `Windows servicing
state`, `Windows Recovery Environment`, `System Restore readiness`, `Start
required Windows Update service` — are the same user-facing prose class as
`detail`, rendered through the same component. A quarter of the tree failure was
not in the brief because it was not in the gate's output. `DBT-P66-004`.

**2. Clippy did not run, and `DBT-P63-010` cannot be closed.** The brief states
that run `34845989034` had "clippy RAN AND PASSED". Measured over the whole
21,473-line job log:

```
grep -ci clippy   ->  2
  8324:  ##[group]Run rustup toolchain install 1.97.1 --profile minimal --component rustfmt,clippy
  8325:  rustup toolchain install 1.97.1 --profile minimal --component rustfmt,clippy
```

Both are the toolchain install in step 3. `cargo clippy` never executed. That
run died at step 21 in `phase12-localization-audit.ps1:12` reporting
`Phase 12 localization audit: 30/32 checks passed` — which is reached through
`verify-enterprise.ps1:15`, **eight lines before the clippy call at `:23`**.
This is the fourth consecutive phase to expect clippy as the wall and the fourth
to find it unreached, always for the same structural reason: line 15 throws
before line 23 is read. The row stays **OPEN**.

**3. phase10 has eight inline throws, not seven.** The brief says seven and
credits P65 with counting them. P65's prose said seven and **its own table
listed eight rows**. The eighth is `phase10-architecture-audit.ps1:153-155`,
where the `if` and the `throw` are on separate lines, so every single-line grep
has missed it — P64 said four, P65 said seven, it is eight. All eight are
hand-read green below.

If a measurement in this report contradicts the brief, the measurement is what
the command printed and the command is named.

---

## ITEM 1 — the rule that separates a wire value from display text

The brief asks for the rule before the translation. Here it is, and it is not
invented for the occasion — it is read off what this product already does:

> **A string is a wire value when the program's own behaviour depends on its
> exact bytes. It is display text when only a person's understanding does.**
> Translation replaces the bytes. So anything whose meaning to the program
> survives only as those bytes cannot be translated, and anything whose meaning
> to a person survives only as language must be.

The codebase gives a second, mechanical test for the same line, and the two
agree on all thirty: **wire values reach the reader through `<TechnicalText>`**
(the LTR-isolated technical primitive) **and display text through
`<LocalizedOwnedText>`** (the localizer). They sit side by side in the same
elements, so the product decided this before the gate asked.

### Applying it

**Thirteen `result_code` values — WIRE. A check failure.**

| evidence | measurement |
|---|---|
| Rust branches on it | `match check.result_code.as_str()` at `crates/system-repair/src/lib.rs:911, :929, :943, :958, :972, :985, :995` |
| the UI branches on it | `check.resultCode!=='NoErrors'` — `RepairPage.svelte:130` |
| the UI renders it as evidence | `<TechnicalText value={item.resultCode}/>` at **all four** render sites (Drivers, Cleanup, Startup, Repair) |
| **it is an open set** | `result_code: format!("ExitCode{code}")` (`windows_impl.rs:655`), `format!("ChkdskExit{}", ...)` (`:517`) |

The last row is the one that settles it. Values are generated at runtime, so no
catalog can ever enumerate them: the gate was not merely wrong, it was asking
for something **unsatisfiable**. Translating `ServiceRunning` would also break
`check_to_fact`, which maps it to `FactState::Healthy` by exact string.

Only 8 of the 13 were visible. Three others — `RebootPending`, `Verified`,
`orcSucceeded` — passed the gate all along for a reason worth naming: the test
is `value not in semantic`, a **substring search** over `semantic.ts`, and those
three appear there as *stage* enum keys. They were passing by coincidence, not
by assertion. `DBT-P66-001`.

**One `detail: "installed"` — TEST FIXTURE. A check failure of a second kind.**
`crates/driver-install/src/lib.rs:1617`, inside the `#[cfg(test)] mod tests`
that opens at `:1450`. The scan skipped `tests` **directories**
(`if "tests" in path.parts`), which cannot see a unit-test module inside a
shipping file. The evidence that this had happened before and was papered over:
**three of the six entries in the `coded_or_fixture` allowlist were test
fixtures**, added by hand one at a time as each surfaced. `DBT-P66-003`.

**Sixteen `title`/`detail` — PROSE. The tree failure.** Eleven `detail` and five
`title`, from `crates/system-repair` and `crates/windows-update`, all rendered
through `<LocalizedOwnedText>`, none branched on by anything. These are Phase 19
servicing and recovery-readiness sentences a user reads:

* *The diagnosis-scoped Windows Update service is running.*
* *Windows Update Agent reports that a restart is required before another installation can safely begin.*
* *Windows reports that another servicing installation is active. AetherCore will not compete with it.*

English-only on Arabic Windows is a parity violation against the product's own
invariant, and these are now translated. Catalog **1678 → 1694** keys, parity
held. `DBT-P66-005`.

### The Arabic was wrong first, and the project's own gates said so

The first draft kept the Latin phrase "Windows Update" inside the Arabic, on the
reasoning that Microsoft keeps feature brand names Latin in Arabic Windows.
**Two independent gates rejected it** —
`phase12-localization-audit.py:148` and `static_validate.py:1777`'s
`phase12_arabic_windows_update_localized` — both of which require
`تحديث Windows`. That is a project decision that predates this session, the
gates were right, and the assumption was mine. The translations use
`تحديث Windows`.

Round-trip verified for all sixteen, not assumed: each Rust literal is present
in shipping source, resolves through `exactOwnedText` to its key, and the key
yields a distinct Arabic string. **16/16.**

---

## ITEM 2 — `Ctrl /` is a technical token, and the gate was not asking that

The brief is right that this is a real localization question rather than an
obvious pass. It is also true that the gate was never asking it. `allowed_phrases`
was an **enumeration of literals**:

```python
allowed_phrases = { "AetherCore", "Windows Update", "Esc", "Ctrl K" }
```

`NavigationRail.svelte:49` is `<kbd>Ctrl K</kbd>` and `:56` is `<kbd>Ctrl /</kbd>`.
Two identical constructs, one passing and one failing, for no reason but which
strings someone had typed into a set.

**The decision: it is a technical token and must not get an AR string.** Three
measured reasons, in increasing order of force:

1. **It names a physical object.** A shortcut hint tells the reader which key to
   press, and on Arabic keyboards the modifier row carries Latin legends.
   Translating `Ctrl` would describe a key that is not on the keycap — it makes
   the hint less usable, not more localized.
2. **The bidi concern is already handled, so it is not an argument for
   translating.** `kbd` is inside the Phase 12 isolation group —
   `typography.css:20`: `direction: ltr; unicode-bidi: isolate` under the comment
   *"Phase 12 — bidirectional technical-data isolation"*. The neutral `/` does
   not reorder under RTL. The gate's own `technical_css_unicode_isolation` check
   asserts that rule exists.
3. **Translating it would break a machine contract.**
   `NavigationRail.svelte:72` derives the ARIA value from the display token:
   `aria-keyshortcuts={item.shortcut.replace('Ctrl', 'Control')}`.
   `aria-keyshortcuts` takes key names fixed by the UI Events spec. A translated
   token fails that `.replace` silently and emits an invalid shortcut **to screen
   reader users** — the localization change would land as an accessibility
   regression on the people least able to route around it.

**The fix is a category, not a third literal.** `kbd` bodies are stripped from
the prose scan the way `<script>` and `<style>` already are. Because that alone
would turn `kbd` into a hatch for smuggling prose past the gate, a new check
`keyboard_shortcuts_are_key_tokens` asserts every `<kbd>` body is a key
sequence. `"Esc"` and `"Ctrl K"` are deleted from the allowlist. `DBT-P66-002`.

---

## ITEM 3 — what every sub-gate inside `verify-enterprise.ps1` ASSERTS

The brief's reason for asking is right: the sub-gates are where this project
keeps finding instruments that cannot fail. CI invokes
`./scripts/verify-enterprise.ps1 -SkipOnlineSupplyChain` (`ci.yml:146`) — and no
other flag, which decides two rows below.

| line | sub-gate | what it ASSERTS | reachable | measured here |
|---|---|---|---|---|
| `:15` | `verify-phase16.ps1` | the whole Phase 0–16 chain, recursively — 13 throws of its own | yes | **the wall, five phases running** |
| `:18` | `enterprise-adversarial-audit.ps1` | 1 inline throw | yes | not run (no `pwsh` on this host) |
| `:21` | `cargo fmt --all -- --check` | zero formatting diff across the workspace | yes | **PASS**, exit 0 |
| `:23` | `cargo clippy --workspace --all-targets --locked -- -D warnings` | zero lints, all targets | yes | **exit 101, 41 findings, 10 crates** — and never once executed on Windows |
| `:25` | `cargo test --workspace --locked` | full workspace regression | yes | not run here |
| `:28-49` | 17 × `invoke-cargo-test-case.ps1` | see below — **not redundant with `:25`** | yes | selectors re-measured here |
| `:51` | `pnpm --dir apps/ui check` | `svelte-check` 0 errors 0 warnings | yes | **PASS** — 229 files |
| `:53` | `pnpm --dir apps/ui build` | `vite build` emits | yes | **PASS** |
| `:56` | `enterprise-stress-matrix.ps1` | — | **NO** — behind `-RuntimeStress`/`-ExtendedSoak` | unreachable in CI |
| `:62` | `verify-phase16.ps1 -ReleasePackaging` | — | **NO** — behind `-ReleasePackaging` | unreachable in CI |

### The one sub-gate built against the failure mode the last four phases hit

Lines `:28-49` look redundant beside `cargo test --workspace` at `:25`, and they
are not. `cargo test -p pkg name` is a **substring filter**: if the test is
renamed or deleted, it matches nothing, runs nothing and **exits 0**.
`invoke-cargo-test-case.ps1:13-15` makes that impossible:

```powershell
$pattern='(^|::)'+[regex]::Escape($TestName)+': test$'
$matches=@($listed|Where-Object{"$_" -match $pattern})
if($matches.Count -ne 1){throw "Rust test selector must resolve to exactly one test: ..."}
```

So `:25` asserts the 17 invariants still **hold**, and `:28-49` asserts they
still **exist** — which `:25` cannot, because a workspace run passes happily
with the test deleted. It is the defence this project has been re-learning by
hand for four phases, already written, in the one place nobody flagged.

The substring-filter claim is measured, not assumed:

```
cargo test --color never --locked -p aethercore-ipc this_test_does_not_exist_anywhere
  -> test result: ok. 0 passed; 0 failed; 0 ignored; 0 filtered out
  -> EXIT=0
```

Replaying the helper's own `--list` assertion for all 17 selectors on this host:
**11 resolve to exactly one test and 6 resolve to zero.** The six are not a
defect and are reported as an artifact of the host, because the distinction is
the whole point of this section — all six are `aethercore-ipc` and all six live
in `crates/ipc/src/windows_impl.rs` (`:1279`, `:1295` and neighbours) inside a
`#[cfg(windows)]` module, so macOS compiles none of them. The enumeration itself
exits 0 and reports `0 tests` for `tests/windows_roundtrip.rs`, whose first line
is `#![cfg(windows)]`. On the runner they resolve; here they cannot. **11 of the
17 verified on this Mac, 6 genuinely machine-gated** — named rather than counted
as passes.

### The scanner's summary line is the instrument that cannot fail

`python3 scripts/ps_marker_scan.py` — **234 assertions, 0 failed, 3 unmeasured**,
unchanged by this phase, which touches no PowerShell. Its closing line is:

> *81 scripts scanned, 7 define assertion helpers; the rest orchestrate and
> assert nothing of the source themselves.*

That sentence is **false**, and it is load-bearing. Counted multi-line-aware,
excluding the bodies of `Require-*` helpers (which the scanner does count, at
their call sites):

| script | uncounted inline assertion-throws |
|---|---|
| `verify-enterprise.ps1` | **10** — lines 16, 19, 22, 24, 26, 48, 52, 54, 59, 67 |
| `verify-phase16.ps1` | **13** |
| `phase10-architecture-audit.ps1` | **8** |
| `phase11-design-audit.ps1` | 8 |
| `phase12-localization-audit.ps1` | 6 |
| `enterprise-adversarial-audit.ps1` | 1 |
| | **46 total** |

Nine of the 46 are unreachable under CI's flags, leaving **37 live and counted
nowhere**. The script the scanner reports as asserting nothing is the one that
gates clippy, rustfmt, the full workspace test run and both UI gates. This is
`DBT-P65-003` with its true size attached.

### phase10's eight, hand-read 2026-09-14

Corpus for `:127`–`:131` is `router.rs` + 18 `router/*.rs` + `streaming.rs`,
3,104 lines — read from the script rather than guessed, because a first attempt
against `crates/` reported ten false MISSINGs.

```
:127  MutationWorkload::  DriverInstall SystemRepair Cleanup Startup        4/4 ok
:129  durable_mutation_released                                                 ok
:131  ReadWorkload::  DriverDiscovery RepairAssessment CleanupDiscovery
                      StartupDiscovery Diagnostics                          5/5 ok
:154  setInterval/clearInterval across apps/ui/src     0 hits                   ok
:168  main.rs        = 153   (throws at >=220)                                  ok
:170  router.rs      = 116   (throws at >=220)                                  ok
:174  router/*.rs    =  18 modules (throws at <2)                               ok
:177  largest module = dispatch.rs at 258  (throws at >=260)              ok, by 2
```

`:177` has **two lines of headroom**. It is the only one of the eight that is
close to its threshold, and it is worth saying out loud before an unrelated edit
to `dispatch.rs` turns step 21 red for a reason no one will connect to this.

---

## ITEM 4 — Dependabot

**Not resumed.** The condition in `.github/dependabot.yml` is *"RESTORE THE
THREE LIMITS TO 5 WHEN `ci.yml`'s WINDOWS JOB IS GREEN ON `main`."* It is not
green: run `34845989034` is `failure` at step 21. P59, P60, P63, P64 and P65
each declined for the same reason and each was right. Twenty-one ledger rows
were open at the start of this session; this phase closes five it opened and
leaves `DBT-P63-010` and `DBT-P65-003` open with corrected measurements.

---

## The pre-push scan, and the rows that are not FAIL

`python3 scripts/ps_marker_scan.py`: **234 assertions, 0 failed, 3 unmeasured**.
The three are unchanged from P65 and were re-read rather than re-counted:
`$lockBaseline` and `$manifestBaseline` in `freeze-dependencies.ps1` resolve to
the two committed baselines, and `$MutationLock` in
`verify-installer-security.ps1` is an ACL check on an installed machine, gated
behind `$InstallerLifecycle`, which `ci.yml` never passes — **unreachable in
CI**, like `:56` and `:62` above. P65 replayed the first two in Python and got
lock 2/2, manifest 57/57; none of P66's changed files is in either set.

The UNMEASURED rows in P65's gate map stand unchanged and are **not** reported
as passes: `check-pipe-teardown-qualification.py` (BLOCKED — needs native
teardown evidence from a Windows run), `sigma-evidence-integrity-test.py` and
`sigma-master-full-app-ui.py`.

---

## Every change carries a negative control

The recurring defect in this project is a check that cannot go red, so no new or
changed check was trusted until it was made to fail on purpose and then reverted:

| control | result |
|---|---|
| `<kbd>Press to open</kbd>` | `keyboard_shortcuts_are_key_tokens` **FAILS** |
| `{item.resultCode}` bare in `CleanupPage.svelte` | `result_code_path_isolation` **FAILS** (4 surfaces → 3) |
| `<span>Open the assistant</span>` | `no_hardcoded_visible_english` **FAILS** |
| a sentinel literal above the `#[cfg(test)]` cut | still caught by the backend scan |

Tree reverted clean after all four (`git status --porcelain` showed only the
gate script).

## Verification

```
test-phase12-localization.py   34/34   (was 30/32)   EN/AR keys 1694
phase12-localization-audit.py  PASS     phase13/14/15/16/18  PASS
static_validate.py             PASS
svelte-check --tsconfig ./tsconfig.json --threshold warning --fail-on-warnings
                               229 files, 0 errors, 0 warnings
vite build                     ok
source_seal.py                 OK - 1487 of 1487 tracked files verified
ps_marker_scan.py              234 assertions, 0 failed, 3 unmeasured
cargo fmt --all -- --check     PASS
```

## Commits

| commit | item |
|---|---|
| `8111fda` | ITEM 1 + ITEM 2 — the three check defects, the sixteen translations, `DBT-P66-001..005` |
| this one | the ledger rows, the corrections to `DBT-P63-010` and `DBT-P65-003`, and this report |

## The single next action

**Clippy, and it does not need CI to be read — but it does need CI to be
believed.** It is the next thing `verify-enterprise.ps1` reaches once the Phase
12 wall is down, and it has never executed on Windows. Measured here at
`8111fda`: **exit 101, 41 findings across 10 crates** — not P63's 30 across 8,
which is itself a reason to distrust any local number as a stand-in. Split by
what the runner actually compiles:

* **21 will fire on Windows** — 10 `collapsible_if`, `unnecessary_unsafe` at
  `crates/security/src/lib.rs:538`, `derivable_impls` at
  `crates/windows-update/src/lib.rs:40`, and the rest.
* **16 are `dead_code` that exists only because macOS cfg's out the Windows
  consumers** — `parse_nvme_health_log`, `parse_ata_smart_sector`,
  `sid_text_from_bytes`, the `driver-backup` manifest structs. They will not
  fire there.
* **4 are in `crates/performance-telemetry/src/macos_impl.rs`**, which the
  runner never builds.

And the gap runs the other way: **7,575 lines of `windows_impl.rs` across 39
cfg-gated files have never been linted by any clippy run on any host**, so the
runner's set is not a subset of this one and cannot be derived from it.

Not fixed in P66, deliberately: it is in none of this session's four items, it
changes Rust logic in eight crates, and the only thing that would validate it is
`cargo test --workspace` — a separate, verifiable unit of work rather than a tail
end of this one.

Two things to carry in:

* **A truncated list with no total is a sample, not a measurement.** Four phases
  of handoffs carried "20 entries" because a `[:20]` slice printed no length.
  This is `DBT-P65-005`'s lesson — an inventory nothing cross-checks is a number
  — with a different subject.
* **A gate that cannot be satisfied is a category error, not a backlog item.**
  `result_code` is generated by `format!`. No amount of translation work would
  ever have closed that half of the wall, and three phases treated it as
  outstanding content.
