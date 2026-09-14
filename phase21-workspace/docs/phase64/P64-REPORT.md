# P64 — the report

Host: the Mac at `/Users/hasanalaaa/dev/aethercore`. Date: 2026-09-14. Branch
`main` throughout. Every figure names the command that produced it. Where a claim
is reasoning rather than measurement it says so, and three of them are.

The short version. **`DBT-P63-004` is closed.** `router.rs` is 116 lines and
`main.rs` is 153, both by decomposition and neither by raising a budget;
`phase10-architecture-audit.ps1` passes end to end on this host. The test suite is
the same three times over — **135 binaries, 649 passed, 0 failed**, measured
before the first commit and after the second. And the run that would have proved
it on Windows **never started**: every job on the account is now refused for
billing before its first step. That is `§3` row 6, it is the only thing between
here and a verdict on step 18, and no session can supply it.

---

## What the brief said and what the runner said

The brief named `phase10-architecture-audit.ps1:155` — the router's line budget —
as the thing to make stop throwing. It is `:153` that CI actually stopped on, and
the evidence is the last run on `main` before this work:

```
run 34781562952, at 3ced10b, steps 1-17 success
step 18  FAILURE
  Auditing service decomposition and production sanitation...
  Exception: …\scripts\phase10-architecture-audit.ps1:153
  maintenance-service/main.rs remains monolithic (397 lines).
```

`:153` is `main.rs` and it runs two lines before the router's `:155`. Decomposing
only the router would have moved the throw two lines and left the job exactly as
red. So both files are decomposed here. Nothing in P63's report covers run
`34781562952` — it was created after the report commit.

Two smaller corrections of the same kind:

* **"135 binaries, 660 passed, 0 failed must hold."** On this Mac it is **649
  passed / 1 ignored**, and that is what held, three times. 660/11 is the
  runner's number for the same 135 binaries with the platform-gated tests
  enabled; P63's own step-13 row says so. The measurement wins.
* **"the blast radius is `phase15_security` and `zenith_recursive`'s two leased-route
  checks — four gates."** It was **fifteen gate scripts**, and the census is below.

---

## ITEM 1 — the router: 1,868 lines, 81 verbs, 17 modules

`handle_request` was lines 86–1753 of `router.rs` as one `match`. It is now:

| | lines |
|---|---|
| `router.rs` — module root: the shared vocabulary, `ServiceContext`, `Call`, `Routed`, `err` | **116** |
| `router/dispatch.rs` — the per-request preamble, the 81-arm dispatch table, the response envelope | 258 |
| 17 domain modules — `consent` `drivers` `repair` `cleanup` `startup` `diagnostics` `updates` `support` `performance` `timeline` `care` `insights` `assistant` `platform` `journal` `security_audit` `session` | 24–251 |

Every domain module opens with `use super::*;`, so the import block is written
once, in the root, where it reads as the router's vocabulary.

**It is a MOVE, and that is measured rather than asserted.** Each arm body was
extracted mechanically and compared against `HEAD`'s, whitespace-squashed:

```
verbs compared: 79   functions found: 79
identical ignoring comma placement: 76
```

The three that differ, each read by hand:

* `GetPlatformCapabilities` and `RunSecurityAudit` — rustfmt removed a redundant
  block around a single-expression match arm once the code sat at column 4
  instead of column 16: `=> { "degraded" }` became `=> "degraded",`. A formatter
  transformation, semantics-preserving by construction.
* `ExportJournal` — `v.owner_principal_key == principal_key` became
  `== *principal_key`, because the handler holds `principal_key` as a `&String`
  where the arm held an owned `String`.

The control for "is this just rustfmt?" is worth stating: `HEAD`'s `router.rs`
run through `rustfmt --edition 2024` produces **zero diff lines**, so the reflow
is caused by the change of indentation, not by a tree that was unformatted.

**Two arms deliberately did not move.** `CheckForUpdates` and `StageUpdate` are
retired verbs whose entire behaviour is the refusal, and `phase15_security`'s
`legacy_service_download_rpc_disabled` asserts that refusal *at the dispatcher*.
They stay inline in `dispatch.rs`, which is truer to the check than delegating
would have been.

### What clippy said, which changed the answer

The first working version kept every arm body byte-for-byte, including
`&principal_key`. Clippy did not accept it:

```
cargo clippy --locked -p aethercore-maintenance-service --all-targets
    242  clippy::needless_borrow
```

121 call sites × two targets. `&principal_key` on an owned `String` is a normal
`&String -> &str` deref coercion and is not linted; on a `&String` binding it is
`&&String`, the compiler dereferences through it, and clippy says so. **Step 18
runs `cargo clippy --workspace --all-targets --locked -- -D warnings`**, so a
verbatim body would have been a red build.

So the 121 sites lost one `&`, and `zenith_recursive`'s two leased-route tokens
lost it with them — `start_with_lease(principal_key,&v.plan_id,lease)`, still
counted `== 4`. Same value, same coercion, one fewer ampersand.

With that, the profile is identical:

| | before | after |
|---|---|---|
| `clippy::collapsible_if` | 20 | 20 |
| `unused_imports` | 18 | 18 |
| `deprecated` | 12 | 12 |
| `dead_code` | 12 | 12 |
| `unused_variables` | 6 | 6 |
| `clippy::type_complexity` | 2 | 2 |
| `clippy::unnecessary_min_or_max` | 2 | 2 |
| `unused_mut` | 2 | 2 |
| `clippy::items_after_test_module` | 1 | 1 |

Measured by swapping the decomposed tree out for `HEAD`'s and re-running, twice,
in the same target directory. **None of these is P64's**; they are the crate's
pre-existing warnings and part of `DBT-P63-010`.

One more thing the compiler decided rather than the author: four verbs name none
of the four request-state locals, so they take no `Call` at all. An unused
parameter is a warning, and a handler that ignores its request state should say
so in its signature.

---

## ITEM 2 — `main.rs`: 397 lines, 244 of them three inline modules

| file | lines | what it is |
|---|---|---|
| `main.rs` | **153** | platform dispatch, the data/log paths, the Windows host functions |
| `unix_service.rs` | 149 | the `--foreground` / `--daemon` loop |
| `windows_service_host.rs` | 69 | the SCM dispatcher, control handler, and the two status transitions |
| `ctrlc_handler.rs` | 24 | the console analogue of the SCM Stop control |

Each block's body is unchanged but for the dedent, and the three `mod x;`
declarations were moved up to sit with the other twenty rather than left where
the blocks used to be.

**The one thing this host cannot compile, and what was done instead.**
`windows_service_host.rs` opens with `use super::*;`, and it is `cfg(windows)`.
The Windows-target check is not available here, and the reason is exactly P63's:

```
cargo check --locked -p aethercore-maintenance-service --target x86_64-pc-windows-msvc
  error occurred in cc-rs … libsqlite3-sys … sqlite3.c
  fatal error: 'stdlib.h' file not found
```

So the mechanism was measured on the side that does compile. A throwaway
`#[cfg(unix)] mod _p64_probe;` file at the crate root, doing `use super::*;` and
naming `product_data_root`, `log_path`, `SERVICE_NAME`, `PathBuf` and `Result`,
builds clean. `super` of a *file* module at the crate root is the crate root,
identical to `super` of an inline one — which is the resolution
`windows_service_host` depends on. The probe was deleted in the command that ran
it. **This is the weakest link in the phase and it is named as such**: the two
`cfg(windows)` files are type-checked nowhere on this host.

---

## ITEM 3 — the blast radius, which was fifteen gates and not four

`DBT-P63-004` predicted four. The census — `grep -rn "maintenance-service/src/router\.rs"`
across `scripts/`, then the same for `main.rs` — found fifteen, and all fifteen
moved in the commit that moved the code.

**Read the router as a module tree.** The gates asking a question *of* the router
are asking about its verbs, not about the file that used to hold all 81 of them.
`SourceReader.read_module(rel)` joins `rel` with every `*.rs` in the directory
Rust names after it, root first and the rest sorted so the join is reproducible
and the ordering helpers keep reading a stable text. One definition, in
`gate_reader.py`, beside `contains`/`count`/`position`/`ordered` — the same
argument P63 made for those.

| gate | what moved |
|---|---|
| `static_validate.py` | three reads (`router10`, `service_router15`, the six-module `service` join) + the ratchet |
| `phase15-security-audit.py` | `router=read_module(...)` |
| `zenith-recursive-audit.py` | `router=read_module(...)`, and the two leased-route tokens lost their `&` |
| `phase12-localization-audit.py` | the typed-message-key sweep |
| `phase17`, `phase18`, `phase29`, `phase31`, `phase32` | `module_text(ROOT, ROUTER)` |
| `phase29-adversarial-audit.py` | **and two `glob("*.rs")` sweeps that had to become `rglob`** |
| `phase9-security-audit.ps1` | `peer.binding_key()` names `router/dispatch.rs` |
| `security-hardening-audit.ps1` | `is_safe_request_id` names `router/dispatch.rs` |
| `phase10-architecture-audit.ps1` | `Get-ModuleText`, the module-file cap, the budgets |
| `zenith-recursive-audit.py` | *(main.rs)* the SCM startup ordering names `windows_service_host.rs` |
| `phase28-adversarial-audit.py` | *(main.rs)* `"pid file removed"`, `"init_json_file_rotated"` name `unix_service.rs` |
| `phase31-adversarial-audit.py` | *(main.rs)* see below |
| `phase26-adversarial-audit.py` | *(main.rs)* the allowed-prefix list |
| `static_validate.py` | *(main.rs)* the one-decider negative |

**Two of these are worth reading twice, because both would have gone quiet rather
than red.**

`phase29`'s two `events::` sweeps globbed `services/maintenance-service/src/*.rs`,
one directory deep. Every verb that moved into `router/` would have left the
sweep silently — the check would still pass, over a smaller set, and nothing
would say so. `rglob` now.

`phase31`'s `p31-unix-no-feature-gate-on-run_unix_service` asserts that
`'#[cfg(feature = "unix-ipc")]\n    pub fn run_unix_service'` is **not** in
`main.rs`. Four spaces of indentation, because the function was inside an inline
`mod`. At column 0 in its own file that string can never appear again, so the
check would have been vacuously true forever. It now reads `unix_service.rs`, at
the indentation the file actually has, and additionally asserts the function is
present — a negative check that cannot fire is not a passing check.

`phase26`'s allowed-prefix list already allowed `main.rs`; the allowance follows
the code out of it. **Measured, not assumed**: without that edit,
`p26-cfg-windows-freeze-scan` gained `services/maintenance-service/src/unix_service.rs`
as a new violation.

### The budget is the throwing gate's number now

`static_validate.py`'s `phase10_service_decomposed` was a P63 ratchet at the
measured 397/1868. It is a met budget now — and at **219/219**, not 220/240,
because `phase10-architecture-audit.ps1` throws at `>= 220` for **both** files.
P10's written 240 for `router.rs` is looser than what actually fires in CI, and a
check that permits what CI rejects buys a red step 18 after a green step 17,
which is the precise failure mode P63 spent its day on. The 220/240 note is kept
beside it as the record.

A third ceiling closes the loophole decomposition opens: an 1,868-line dispatcher
must not come back as an 1,868-line `router/updates.rs`. Every `router/*.rs`
carries a ratchet at today's largest, 258, and `phase10-architecture-audit.ps1`
carries the same cap at 260 so the two gates cannot drift apart again.

`main.rs`'s three siblings get **no** cap. The router's cap earned itself: the
81-arm match *was* the monolith, and moving it created somewhere for it to hide.
None of these three is near a limit, and a cap without that argument is
speculation.

Negative control, run and reverted:

```
main.rs -> 220, router.rs -> 220, router/updates.rs -> 259
  ok: false
  over_ceiling: {"main.rs": {"lines": 220, "ceiling": 219},
                 "router.rs": {"lines": 220, "ceiling": 219},
                 "router/updates.rs": {"lines": 259, "ceiling": 258}}
tree restored: True
```

---

## ITEM 4 — the PowerShell scanner, kept this time

P63 swept the chain with a scanner that replayed `Require-Marker`'s regex
semantics, found five defects in seconds, and **did not commit it** — so its
stated limit (`@('a','b')` array markers, `phase11-design-audit.ps1` unmeasured)
was going to be re-discovered rather than lifted. It is
`scripts/ps_marker_scan.py` now, and the limit is lifted.

The thing that made the array form readable was not a special case for it. The
scanner **reads each script's assertion helpers out of their own bodies** instead
of assuming one shape, and the shapes differ in ways that are load-bearing:

| helper | test | case | subject |
|---|---|---|---|
| `Require-Marker $File $Pattern` | `-notmatch` | **insensitive** (.NET default) | one file, regex |
| `Require-Text $File @('a','b')` | `.Contains` | **sensitive** | one file, an **array** of literals |
| `Reject-Tree $Pattern` | `-match` over `Get-ChildItem apps,services,crates -Recurse` | insensitive | a directory sweep, **no path argument at all** |

It also unrolls `foreach ($x in @(...)) { }` and `@(...) | ForEach-Object { }`, so
the calls hidden inside them are ordinary calls by the time they are parsed —
that is `DBT-P63-014`'s other named limit. And it handles PowerShell's doubled
single quote (`'invoke\([''"]authorize_plan[''"]'` is one string containing `'`),
without which the `Reject-Tree` line `DBT-P63-013` repaired parses as garbage.

```
python3 scripts/ps_marker_scan.py
  81 scripts scanned, 7 define assertion helpers;
  the rest orchestrate and assert nothing of the source themselves.
  total assertions=234 failed=0 unmeasured=3
```

| script | assertions | failed | unmeasured |
|---|---|---|---|
| `phase10-architecture-audit.ps1` | 115 | 0 | 0 |
| `phase9-security-audit.ps1` | 45 | 0 | 0 |
| **`phase11-design-audit.ps1`** | **41** | **0** | **0** |
| `security-hardening-audit.ps1` | 26 | 0 | 0 |
| `verify-reproducible.ps1` | 4 | 0 | 0 |
| `freeze-dependencies.ps1` | 2 | 0 | 2 |
| `verify-installer-security.ps1` | 1 | 0 | 1 |

The three unmeasured are runtime-computed paths — `$lockBaseline`,
`$manifestBaseline`, `$MutationLock` — each named individually in the output.
**An assertion this tool skips is UNMEASURED, never passing**, and the exit code
counts failures only, so a skip can never be mistaken for a pass. The "7 of 81"
line exists for the same reason: the other 74 are thin wrappers around Python
gates, and a reader should be able to tell "asserts nothing" from "asserted
nothing because I could not read it".

**It can fail, and the proof is this phase's own blast radius.** `HEAD`'s two
router markers, run against the decomposed tree:

```
phase9-security-audit.ps1     FAIL  router.rs :: 'peer\.binding_key\(\)'  marker absent
security-hardening-audit.ps1  FAIL  router.rs :: 'is_safe_request_id'     marker absent
```

That is exactly the breakage the decomposition would have shipped, caught in a
second instead of in a ~50-minute round trip.

**What it does not cover, read by hand instead**, and this is the honest half of
the answer to "teach it that form, or read those files by hand and say which you
did": the inline `if (...) { throw }` statements, which are not helper calls.
`phase10-architecture-audit.ps1` has four that matter. Hand-read 2026-09-14:

```
:153  main.rs   =  153   ok
:155  router.rs =  116   ok
      router/*  =  18 modules, max 258   ok
      $serviceControl  all four MutationWorkload::, all five ReadWorkload::,
                       durable_mutation_released — present
```

So: 115 scanned, 4 hand-read, and the script passes end to end on this host.

The second tool is `scripts/test_gate_module_reader.py`, in the shape
`test_gate_contains.py` set — assert the widening works **and** that it can still
fail. 28 cases: every moved token is absent from the module root alone and
present in the tree, an invented token and the pre-P64 `&principal_key` spelling
are absent from both, the two leased-route counts are exactly 4, a file with no
module directory reads exactly as itself, and deleting a domain module from a
copy of the tree takes its verb body with it while the dispatch arm survives.

---

## ITEM 5 — `DBT-P64-001`, found because `git status` was dirty

Every gate sweep left `phase21-workspace/SBOM.cdx.json` modified. Measured three
times, restoring the file between each:

```
git checkout -- phase21-workspace/SBOM.cdx.json
python3 scripts/phase29-adversarial-audit.py   -> SBOM REWRITTEN
python3 scripts/phase31-adversarial-audit.py   -> SBOM REWRITTEN
python3 scripts/phase32-adversarial-audit.py   -> SBOM REWRITTEN
```

(31 and 32 import 29 in-process, so it is one writer.) The rewrite is not
cosmetic: the committed SBOM says `version 0.1.0` for every workspace crate and
the regenerated one says `0.1.11`. **The delivered SBOM has been stale by eleven
patch versions**, and the only thing that noticed was an audit quietly correcting
it in place and not saying so. It is also why `p30-delivered-source-seal-ok`
reports `hash:SBOM.cdx.json` on every run, at `HEAD` as well as here — that seal
has been measuring the audit's own side effect.

This is `omega-evidence.py`'s failure mode with the roles reversed: that tool
exists to catch a gate mutating its subject, and here a gate's determinism check
writes its output into the delivered tree instead of a scratch path.

Filed rather than fixed, because two separate things have to happen and one of
them is a deliberate act: the audit must write to a temporary path and compare,
and `SBOM.cdx.json` must be regenerated on purpose, in its own commit, with the
manifest after it. Neither belongs inside a decomposition.

---

## ITEM 6 — the run that never started

`14756cb` was pushed at 07:52 UTC. Run `34819921518`:

```
windows     failure  in 3s
deny-check  failure  in 3s

The job was not started because recent account payments have failed or your
spending limit needs to be increased. Please check the 'Billing & plans'
section in your settings
```

Zero steps executed on either job. Re-run attempted once — attempt 2, same
refusal, 3 seconds. The run eleven hours earlier (`34781562952`, at `3ced10b`)
executed eighteen steps normally, so this is **new, and it is the account rather
than the workflow**.

That is now `§3` row 6, and it is the only thing standing between this phase's
work and a verdict on step 18. It is `github.com/settings/billing`, and no
session can supply it.

**What this means for what can be claimed.** Steps 18 through 23 remain
unexecuted. `phase10-architecture-audit.ps1` — the gate that stopped step 18 four
times — passes here by 115 scanned assertions plus 4 hand-read throws, and
`verify-phase15.ps1`'s Python components all pass here, but PowerShell has still
never run on this host and the sub-gates behind `phase10` in the chain
(`cargo clippy -- -D warnings`, the 17 named regression tests, `pnpm check`)
have no local equivalent that has ever been run either. **Getting past `:153` is
necessary and it is not the same as the job being green.** `DBT-P63-010`'s 30
clippy findings are still unmeasured on Windows.

---

## ITEM 7 — Dependabot

**Not resumed, and the condition moved further away rather than closer.**

`.github/dependabot.yml`: *"RESTORE THE THREE LIMITS TO 5 WHEN `ci.yml`'s WINDOWS
JOB IS GREEN ON `main`. Not when a particular step passes — a pull request that
cannot get a verdict is worse than no pull request."*

A job that cannot start cannot be green. Resuming now would put four PRs into a
queue that is refused at the billing check in three seconds — the same failure
P59 recorded, P60 refused to repeat, and P63 refused again, with an even less
useful outcome than before.

---

## The ledger

`DBT-P63-004` CLOSED. `DBT-P63-014` moved: its stated limit on array-form markers
is lifted and the scanner that lifts it is committed; its blocker line is
corrected because neither `:153` nor `:155` throws any more. `DBT-P64-001`
entered, OPEN. `§3` gains row 6, the billing block.

---

## Commits

| commit | item |
|---|---|
| `e4ecac2` | ITEMS 1, 3, 4 — the router is 81 verbs in 17 modules; eleven gates move with it; the scanner and the module-reader test |
| `14756cb` | ITEMS 2, 3 — `main.rs` is 153 lines; five more gates move; `phase10-architecture-audit.ps1` passes end to end |
| this one | the report, `DBT-P64-001`, and `§3` row 6 |

---

## The single next action

**Fix the Actions billing, then re-run `main`.** `§3` row 6. Nothing this
repository can measure about steps 18–23 is worth anything until a job can start,
and every remaining open row that names CI — `DBT-P63-010` (clippy on Windows),
`DBT-P63-012`, `DBT-P63-014`, and Dependabot's own condition — is waiting behind
it.

When a run does execute, the row to watch is step 18 past
`phase10-architecture-audit.ps1`, and the next wall is almost certainly
`verify-enterprise.ps1:23` — `cargo clippy --workspace --all-targets --locked
-- -D warnings`, `DBT-P63-010`, 30 findings on macOS and never measured on
Windows. That is a prediction, not a measurement, and P63's report is the reason
to label it: it predicted clippy would stop step 18 and was wrong by four
sub-gates.

Two things to carry in:

* `scripts/ps_marker_scan.py` is the way to meet a PowerShell gate from this Mac,
  and its output separates FAIL from UNMEASURED on purpose. Read the UNMEASURED
  rows; they are the ones that will bite in CI.
* A decomposition's blast radius is a `grep` for the file's path across
  `scripts/`, and then a second pass for the checks that would go **quiet**
  rather than red — a one-directory `glob`, a negative assertion whose literal
  can no longer occur. Those are the ones that do not announce themselves.
