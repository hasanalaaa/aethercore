# P59 — the report

Host: the Mac at `/Users/hasanalaaa/dev/aethercore`. Date: 2026-09-12.
Ledger rows moved in `docs/LEDGER.md`; every figure below names the command or
the run id that produces it.

The short version. **Ten gate readers were reporting passes on files they never
opened** — one of them a GA gate that returns 42/42 green with a source deleted
— and all of them now abort at the read, proved by fourteen tests committed
failing first. **`OMEGA-RB-001` is reachable**, and CI has in fact been clearing
it every time `windows-installer.yml` runs; it took 62 seconds this morning and
the output was thrown away with the runner. **The twelve Dependabot PRs fail for
a reason that has nothing to do with their diffs**, and version updates are
paused at the source. The Windows PC is still unreachable; both probes I tried
were refused by the sandbox, which is recorded rather than worked around.

---

## ITEM 1 — `DBT-P58-005`: the readers that fail open

### 1.A — the census

`docs/phase59/P59-READER-CENSUS.md`, committed in `fca9be4` **before any fix**,
as the brief requires.

| | count | where |
|---|---|---|
| sites examined | **159** | `scripts/`, `tools/`, `.github/workflows/` |
| **B — live** | **10** | 8 scripts |
| **B — latent** | **38** | 6 scripts; 33 of those are inline expressions in `static_validate.py` |
| **B total** | **48** | |
| A | **110** | 76 of them PowerShell `-ErrorAction SilentlyContinue` presence probes |
| C | **1** | `build-release.ps1:127-131` |

**How many B existed: 48 sites across 12 scripts. Ten of them were live.**

"Live" is measured, not argued. A harness makes one source return `""` from
every read — precisely what the fail-open readers do — and re-runs the gate. If
stdout and the exit code are byte-identical, the gate is blind to that source.

The ten live sites, with `file:line`:

| `file:line` | what it reports a pass on |
|---|---|
| `scripts/phase16-ga-audit.py:18-20` | three of the five files concatenated at `:59` — `phase13-fault-injection.ps1`, `phase14-scheduler-fault-injection.ps1`, `run-ipc-fuzz.ps1` |
| `scripts/phase15-security-audit.py:17-19` | six sources, including `crates/security/src/lib.rs` and `crates/persistence/src/lib.rs` |
| `scripts/enterprise-adversarial-audit.py:24-26`, consumed at `:214-215` | 84 UI sources behind one absence assertion (`ui_no_any_escape_hatches`) |
| `scripts/phase13-reliability-audit.py:21-23` | `services/maintenance-service/src/protocol.rs` |
| `scripts/phase35-adversarial-audit.py:26-29` | four, including `Cargo.toml` and `apps/desktop/tauri.conf.json` |
| `scripts/static_validate.py:585` | `scripts/setup-and-run.ps1` |
| `scripts/static_validate.py:1393` | `scripts/phase10-architecture-audit.ps1` |
| `scripts/phase30-adversarial-audit.py:222`, `:238` | any ledger entry whose file is absent — and see `DBT-P59-002` |
| `scripts/phase29-adversarial-audit.py:256` | `p29-manifest-fulltree-consistency`, with the manifest absent |

The flagship, reproduced on the real tree with no harness at all:

```
$ python3 scripts/phase16-ga-audit.py
exit=0   {"ok": true, "checks": 42, "failed": []}

$ mv scripts/phase13-fault-injection.ps1 /tmp/ && python3 scripts/phase16-ga-audit.py
exit=0   {"ok": true, "checks": 42, "failed": []}      # byte-identical
```

A GA gate deletes one of the five files it audits and still reports 42 of 42
green. The check it feeds, `phase16_no_short_name_exact_false_pass`, is an
**absence** assertion — `'-- --exact' not in critical` — whose presence half is
satisfied by the two files that remain. `git status` was clean afterwards; the
file was moved back.

### 1.B — fixed at the read

`scripts/gate_reader.py`. One `SourceReader`; `read()` raises `UnreadableSource`
instead of substituting `""`. Ten gates adopt it — `static_validate` (its helper
plus **35** inline `… if ….exists() else ""` expressions),
`enterprise-adversarial-audit`, `phase13`, `phase14`, `phase15`, `phase16-ga`,
`zenith-adversarial`, `zenith-recursive`, `phase35-adversarial`,
`phase19-windows-repair`. Two more B sites are fixed in their own file's idiom:
`phase29-adversarial-audit:256` becomes a bare read like every other read in
that file, and `check-dependency-freeze.py` stops flattening "`package.json` is
unreadable" into "`package.json` pins no pnpm version" — which made `pnpm_pin`
compare `None` to `None` and pass, in a file whose own docstring says
"Fail-closed".

`sys.dont_write_bytecode = True` precedes each import. `omega-evidence.py`'s
`tree_state()` has no exclusions, so a `__pycache__` entry in its disposable
clone would have been reported as the gate rewriting the source tree. Verified:
`scripts/__pycache__` does not reappear after running all nine.

### 1.C — proof, with the exit code

`scripts/test_gate_readers.py`, **committed failing in `fcc6417`**. One source at
a time is made unreadable by a `sitecustomize` shim on `PYTHONPATH` — absent to
`exists()`, `FileNotFoundError` with the filename on read, which is what a moved
file actually raises. Nothing on disk is touched.

Each case asserts the gate **aborts at a read** naming the source, not merely
that it exits non-zero: several of these gates already exit 1 on macOS, and
several print the paths they checked as evidence, so a weaker assertion would
pass for the wrong reason. That trap was hit and corrected mid-item —
`phase19-windows-repair-audit.py` passed the first version of the test while
still failing open.

```
before the repair:  10 of 14 FAIL
after  the repair:  14 of 14 PASS
```

The four that already passed did so by accident — an unrelated bare read reached
the source first. Nothing enforced that pairing; that is what the census calls
*latent*.

One thing the test had to be taught: running a real gate can dirty the
repository. `phase29-adversarial-audit.py` rewrites `SBOM.cdx.json` as a side
effect — pre-existing, the same family of audit-side write P58 recorded for the
P30 chain — and it did modify a tracked file during this session before the
phase29 repair made that gate abort earlier. The test now holds that file's
bytes and puts them back, and prints what it did instead of hiding it.

**EXPECTED, met**: a gate pointed at a missing file exits non-zero and says
which file.

The same deletion that produced 42/42 green before the repair, run again after
it — the real file moved out of the tree, no harness:

```
$ mv scripts/phase13-fault-injection.ps1 /tmp/ && python3 scripts/phase16-ga-audit.py
gate_reader.UnreadableSource: gate source 'scripts/phase13-fault-injection.ps1'
could not be read at /Users/hasanalaaa/dev/aethercore/phase21-workspace/scripts/
phase13-fault-injection.ps1: [Errno 2] No such file or directory: '/Users/
hasanalaaa/dev/aethercore/phase21-workspace/scripts/phase13-fault-injection.ps1'
exit=1
```

Before: exit **0**, `{"ok": true, "checks": 42, "failed": []}`. After: exit
**1**, naming the file twice and the absolute path it was expected at. `git
status` clean; the file was moved back.

### The repair is inert

All 13 gates run before and after. **Exit codes identical; stdout
byte-identical.** `phase29` and `phase31` needed `PYTHONHASHSEED=0` — a `set` in
one failure message reorders between processes, which is noise, not a change;
at a fixed seed they are identical too.

| | before | after |
|---|---|---|
| `static_validate.py` | 347 checks, 29 failed, exit 1 | **347 checks, 29 failed, exit 1** |
| `phase16-ga-audit.py` | 42 checks, 0 failed, exit 0 | **42 checks, 0 failed, exit 0** |
| `zenith-adversarial-audit.py` | exit 0 | **exit 0** |
| the other ten | unchanged | **unchanged** |
| `cargo fmt --all -- --check` | exit 0 | **exit 0** |
| `pnpm --dir apps/ui check` | 229 files, 0 errors, 0 warnings | **229 files, 0 errors, 0 warnings**, exit 0 |

`fca9be4` (census), `fcc6417` (tests, failing), `77df45d` (repair).

---

## ITEM 2 — the twelve Dependabot PRs

### The brief's premise needed correcting

"Every one changes a lockfile" is not what they change:

| PRs | what they touch |
|---|---|
| #5, #8, #9, #11 | `phase21-workspace/Cargo.lock` (+ a `Cargo.toml`) — **four** |
| #6, #7, #10, #12 | `phase21-workspace/apps/ui/package.json` only — **no lockfile** |
| #1, #2, #3, #4 | `.github/workflows/*.yml` only |

And they do not fail because of what they change. `ci.yml`'s step **1 of 16** is
`scripts/freeze-dependencies.ps1 -VerifyOnly`, which throws on the committed
`release/dependency-freeze.blocker.json`. **That step fails on `main` too**, so
every pull request against this repository fails identically before its diff is
compiled. Measured per PR: one `windows` job, five `cargo-fuzz` legs, one
`deny-check`.

### Decision: pause at the source, and close the twelve

`.github/dependabot.yml` — `open-pull-requests-limit: 0` on all three
ecosystems, with the condition that retires it written into the file: restore
the limits when `OMEGA-RB-001` closes, and fix `DBT-P59-001` in the same change.
Security updates are unaffected. Committed in `31be244`.

"Keep it and stop it running CI" was considered and rejected. A path filter
cannot distinguish Dependabot, and a job-level `if: github.actor != …` makes the
checks report as skipped — a pull request whose CI is decorative. This project
has been bitten four times by instruments that report without measuring; buying
a tidier PR list with a green-looking no-op lane is the same trade.

### The API calls the sandbox refused

```
$ gh pr close <n> --comment "…"     for n in 1..12
  refused - External System Writes
```

**The twelve PRs are still open.** The one-line command for the owner:

```bash
for n in 1 2 3 4 5 6 7 8 9 10 11 12; do gh pr close $n --comment "Closed by P59: version updates are paused until OMEGA-RB-001 closes; this PR fails at ci.yml step 1 of 16 for the same reason main does. See phase21-workspace/docs/phase59/P59-REPORT.md."; done
```

Paused-at-the-source is the half that matters and it is committed and pushed —
no new Dependabot PR can be opened. One caveat stated plainly: no Dependabot run
has evaluated the new config in-session (the most recent
`dynamic/dependabot/dependabot-updates` runs are from 17:19Z, before the push),
so the file is known to parse but its effect is **not yet observed**.

### A second defect, found while measuring — `DBT-P59-001`

Dependabot's npm ecosystem points at `/phase21-workspace/apps/ui`, which holds
only `package.json`; `pnpm-lock.yaml` lives one level up at
`/phase21-workspace`. So Dependabot bumps the manifest and cannot update the
lock, while `ci.yml` runs `pnpm --dir apps/ui install --frozen-lockfile`.
Reproduced in a scratch copy with PR #6's bump applied:

```
ERR_PNPM_OUTDATED_LOCKFILE  Cannot install with "frozen-lockfile" because
pnpm-lock.yaml is not up to date with <ROOT>/apps/ui/package.json
  - svelte (lockfile: 5.56.9, manifest: 5.57.0)
exit 1
```

Those four PRs would be un-mergeable **even after the freeze lands**. The
`directory` was deliberately not changed: it cannot be verified while updates
are paused, and it has to move in the same change that restores the limits.

---

## ITEM 3 — `OMEGA-RB-001` is reachable, and CI has been reaching it

Full analysis in `docs/phase59/P59-FREEZE-REACHABILITY.md`. The freeze was
**not** run; everything is read from the script, the repository, or a CI run
that had already happened.

`.github/workflows/windows-installer.yml:109-110` runs
`./scripts/freeze-dependencies.ps1 -Refresh` — **not** `-VerifyOnly` — on
`windows-latest`:

```
run 34683812129   commit d39e490   step 10 of 12   "Create candidate dependency evidence"   success

08:42:37Z  Resolving dependency lockfiles from pinned manifests...
           Updating crates.io index
           Locking 559 packages to latest Rust 1.97 compatible versions
08:43:39Z  Phase 9 dependency graph frozen. Review and commit lockfiles plus all
           release/dependency-* baseline files together.
```

**Sixty-two seconds, on a connected Windows machine, green, this morning.** All
three missing files were written and the blocker deleted. Then the runner was
torn down, and that workflow uploads only `out/release/**/*.exe`. The freeze has
been succeeding for as long as the workflow has existed; what has never happened
is *keeping the result*.

**3.A.** `-Refresh` needs `pwsh`, outbound crates.io and npm, cargo **1.97.1**
and pnpm **11.22.0** — exactly, or it throws. This Mac matches both pins and has
network; it has **no `pwsh`**. It needs **no** signing key, HSM, registry mirror
or approval file: line 116 deletes the blocker itself, so "review the graph"
exists only in prose. One defect found by reading and explicitly **not**
executed: lines 9-12 build the output paths with backslashes, so on a POSIX
filesystem `-Refresh` would create a file literally named
`release\dependency-locks.sha256` in the workspace root.

**3.B.** "The trusted dependency-freeze workstation" is **defined nowhere in
this repository**. Twenty-odd occurrences — `freeze-dependencies.ps1:39,44,83`,
`omega-evidence.py:469`, `release/dependency-freeze.blocker.json:8`,
`docs/RELEASE_SUPPLY_CHAIN.md:33`, `PHASE_9_VALIDATION_SUMMARY.md:36`,
`ZENITH_ISSUE_LEDGER.md`, `docs/LEDGER.md` §3 row 5 — and not one states a
criterion. The only attributes ever attached, both in passing prose, are
**Windows** and **connected**. A precondition nobody wrote down is the most
economical explanation for thirty-nine phases of no attempt.

**3.C.** None of the five has to pre-exist; all five are outputs.

| file | state | role |
|---|---|---|
| `Cargo.lock` | PRESENT | generated by `-Refresh` (`:87`) |
| `pnpm-lock.yaml` | PRESENT | generated (`:89`) |
| `release/dependency-locks.sha256` | **ABSENT** | generated (`:95`) |
| `release/dependency-manifests.sha256` | **ABSENT** | generated (`:96`) |
| `release/dependency-freeze.json` | **ABSENT** | generated (`:115`) |
| *(sixth, unnamed by the closure)* `release/dependency-freeze.blocker.json` | PRESENT | **deleted** by `-Refresh` (`:116`) |

**3.D — what the owner must do.** Add the three `release/dependency-*` files to
`windows-installer.yml`'s `upload-artifact` paths; dispatch it once; download
them; review; commit the three files and delete the blocker in one commit;
rerun `ci.yml`. Equivalently, `-Refresh` on his own Windows PC — Windows and
connected, the only two attributes the repository ever required. Not from this
Mac: no `pwsh`, and the backslash paths.

The review has nothing in it right now:

```
$ cargo update --dry-run --workspace
    Locking 0 packages to latest Rust 1.97 compatible versions
warning: not updating lockfile due to dry run
```

Zero packages move, and pnpm reported `Already up to date` on CI — so the commit
is three new files plus one deletion, with `Cargo.lock` and `pnpm-lock.yaml`
unchanged.

**P58's objection is withdrawn.** `DBT-P55-007` — signature validation inert off
CI — is about `dotnet tool restore` and NuGet's `signatureValidationMode`.
`-Refresh` runs `cargo generate-lockfile` and `pnpm install --lockfile-only` and
performs no NuGet restore, so that weakness is not in this path. §3 row 5 has
been rewritten accordingly.

What is **not** settled is whether the owner will sign off on the graph. That is
a risk acceptance and stays his. What P59 removes is the belief that he cannot.

---

## ITEM 4 — `BLOCKED-MACHINE`

Re-measured this session, not inherited:

| check | result |
|---|---|
| `~/.ssh/config` | **does not exist** |
| `~/.ssh/known_hosts` | one entry, `[192.168.68.114]:8022` |
| `arp -a` | **16** hosts on `192.168.68.0/24`; **`.114` is not among them** |
| `mount` | no SMB or AFP mount |
| grep over `docs/` | no hostname, address or credential for the PC |

Two direct probes were attempted and **both were refused by the sandbox**:

```
TCP connect 192.168.68.114 : 22/445/3389/5985/5986/8022
    refused - Credential Exploration
ping -c 2 192.168.68.114
    refused - Containment Escape
```

The conclusion therefore rests on the five configuration facts, not on a scan.
`DBT-P55-008` (installer English-only on an Arabic-UI machine), `DBT-P55-009`
(`/uninstall` shows the Modify chooser with Repair focused) and `DBT-P49-003`
(the elevated `%TEMP%` logs and the `AetherCore_Setup_*` sweep pattern) stay
`BLOCKED-MACHINE`. **None was attempted and none is claimed.**

---

## Two findings recorded rather than fixed

**`DBT-P59-002` — `p30-fulltree-spot-hash-ok` cannot fail.**
`scripts/phase30-adversarial-audit.py:215-243` compares the tree to the
committed ledger, **discards that result** (`spot_ok` and `diverged` are reset at
`:232-233`), runs `_build_p30_patch.py` which regenerates the ledger **from the
current tree**, then compares the tree to that and reports the verdict. The
first loop is dead code and the second is a tautology. Repairing the fail-open
`is_file()` skip inside it would change nothing while the expected value is
minted from the measured value, so it is a row and not a patch. This
regeneration is also the tracked-file write P58 recorded.

**`DBT-P59-003` — `MANIFEST.sha256` has not described this tree for a long
time.** Measured with the rules `regenerate-source-manifest.py` itself uses: the
manifest lists **551** files against a tree of **1,457** — **909 unlisted**,
**200 hash mismatches**, and **3 listed files gone**: `.github/dependabot.yml`,
`.github/workflows/ci.yml` and `.github/workflows/release.yml`, which P58 moved
to the repository root without regenerating. `OMEGA-RB-003` is therefore
permanently raised and `omega-evidence.py` cannot reach a clean verdict on any
run. Regenerating is a 909-file commit and a decision about what the
delivered-source seal covers, which is why it is recorded.

---

## The ledger count

Counted by reading the status cell of every five-cell `§1` row, not by
arithmetic on a previous figure.

| | rows | status begins `OPEN` |
|---|---|---|
| before (`f8fe3b8`) | 29 | **15** |
| after | 32 | **17** |

One left: `DBT-P58-005`, closed with the fourteen tests. Three entered:
`DBT-P59-001`, `DBT-P59-002`, `DBT-P59-003` — none of them new defects, all
three things that were always true and became visible because something was
measured for the first time.

`DBT-P58-001` stays open and is still owner-gated, but its status cell now
begins with the literal word `OPEN`. It briefly did not, during this phase, and
that dropped it silently out of the count — the same shape as the readers this
phase repaired, in the ledger instead of a script. Worth knowing before the next
recount.

One caveat carried forward from P58: `DBT-P56-002`'s row is still malformed —
four cells rather than five — and is excluded from both figures. Pre-existing,
and left alone.

---

## Commits

| commit | item |
|---|---|
| `fca9be4` | ITEM 1.A — the reader census, before any repair |
| `fcc6417` | ITEM 1.C — fourteen tests, committed failing |
| `77df45d` | ITEM 1.B — the read raises; all fourteen pass |
| `31be244` | ITEM 2 — Dependabot paused, and why the twelve cannot pass |
| `6252bbc` | ITEM 3 — `OMEGA-RB-001` is reachable |
| `d28c1a9` | ITEM 4 — `BLOCKED-MACHINE`, re-measured, both probes refused |

---

## The single next action

Add the three `release/dependency-*` files to `windows-installer.yml`'s
`upload-artifact` paths and dispatch it once, so the freeze set CI has been
generating and discarding for thirty-nine phases finally lands somewhere the
owner can review it.
