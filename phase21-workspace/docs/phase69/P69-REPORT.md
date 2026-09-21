# P69 — the report

Host: the Mac at `/Users/hasanalaaa/dev/aethercore`. Date: 2026-09-19 to
2026-09-20. Branch `main` from `218a180`, plus one Dependabot branch. Every
figure names the run or the command that produced it.

The short version. Three items and one blocker that outgrew them.
**Item 1** turned a text assertion that proved nothing into a boundary check the
compiler enforces, and its regression test was observed passing **six times** on
the Windows runner. **Item 2** measured the phase 26/27/28 audits instead of
trusting the handoff's description of them, found more failures than described,
and separated three *gate defects* from the rest — three files were being
accused of things they do not do. **The blocker**: the delivered-source seal and
Dependabot are structurally incompatible, nine of the ten open bump PRs died at
`ci.yml` step 10 in under two minutes with seventeen gates unrun, and the fix is
a documented human step rather than a hole in the seal. Running that step once
end to end on PR #19 turned step 10 green **and uncovered a second wall at step
11** that no transcript had recorded: the approved dependency freeze. That one
is an owner's decision by its own documentation and was deliberately not
touched.

---

## 1. `SERVICE_DEMAND_START` — a gate guarding a symbol with no behaviour

`startup_service_is_next_start_only` (`scripts/static_validate.py:468`) asserted
two things about `crates/startup-manager/src/windows_impl.rs`: that the token
`SERVICE_DEMAND_START` appears in it, and that `SERVICE_DISABLED` does not.

**The first half proved nothing.** The token was an import no code referenced,
held alive only by `#[allow(unused_imports)]` — an allow added in P68's
`51259a7` precisely because deleting the import turned step 20 red. The actual
demand-start decision was the bare literal `3` at
`crates/startup-manager/src/lib.rs:1345`, in the platform-neutral planner, which
cannot name a Windows constant. A text gate was guarding a symbol with no
behaviour, and P68 had just spent a round trip discovering that the symbol's
only consumer was the gate itself.

The fix puts the behaviour where the gate already looks. `set_service_start` is
the **single writer** of a service start type in this crate — its only two
callers are the restore and apply arms at `windows_impl.rs:171` and `:173` — so
the floor belongs there and not at each caller:

```rust
// crates/startup-manager/src/windows_impl.rs:798, inside set_service_start (:787)
if start > SERVICE_DEMAND_START.0 { ... }
```

SCM start types are ordered by how early the service launches, so a larger value
is a *weaker* start. Refusing anything past demand start states Phase 5's
mandate as a boundary check instead of a comment. It is written as a comparison
against the demand-start constant and never by naming the disabled constant,
because the same gate asserts that token is absent from this file.

**Why the guard cannot reject a legitimate write, measured rather than assumed.**
`scan_services` admits a service into the inventory only when `start == 2`
(`windows_impl.rs:524`, `if start != 2 { continue }`), so restore replays `2`
and apply writes the `3` from
`lib.rs:1345`. Both are at or below the floor. A weaker value can only reach
this function from a corrupted or hand-edited state JSON — the failure
`DBT-P46-B13` already documents in the `scan_services` header comment at
`windows_impl.rs:486-492` — where writing it
would leave a real service unable to start with no record of its previous value.

`#[allow(unused_imports)]` is dropped and the import folded into the `Services`
block. **Clippy now enforces what the text gate only asserted**: deleting the
guard makes the import unused again, and step 21 fails.

The regression test is
`set_service_start_refuses_start_types_weaker_than_demand` (`windows_impl.rs:1202`),
and it checks both
directions — a value past the floor is refused before any SCM call, and demand
start itself reaches the SCM and fails there for the SCM's own reason — so it
fails if the guard is removed *and* if it is made too strict.

**Observed on the runner six times, all six `... ok`.** Run `35476215704` at
`cb9607d`, counted out of the job log:

| step | occurrences |
|---|---|
| 16 `Rust unit/integration tests` | 1 |
| 21 `Enterprise convergence master source/native qualification gate` | 5 |

Step 21 re-runs the Rust test suite five times inside `verify-enterprise.ps1`.
That is worth knowing on its own: the suite's wall-clock cost on the runner is
paid six times per green run, not once. Step 16 alone reported **661 passed** on
Windows against 649 on macOS — the 12-test difference is the `cfg(windows)`
surface, which is the same surface no host in this project can compile.

## 2. Three false accusations in the phase 26/27 audits

The handoff described two findings. Measurement found more, so this is what ran
rather than what was expected:

| audit | checks | failures before | failures after |
|---|---|---|---|
| phase26 | 376 | 2 | **0** |
| phase27 | 438 | 6 | 4 |
| phase28 | 563 | 13 | 11 |

Three of those failures were **gate defects where the source is innocent**.
They are the three false accusations, and they are fixed. Everything still
failing is a decision about the product and is deliberately left failing.

**1. `p26-cfg-windows-freeze-scan` — operator precedence.** The condition was

```python
if "// Phase 26" in text or "phase 26" in text.lower() and "cfg(windows)" in text
```

which Python parses as `A or (B and C)` because `and` binds tighter than `or`.
The first arm alone condemned any file that merely **mentions** Phase 26. It
reported `services/maintenance-service/src/unix_composition.rs` as a
`cfg(windows)` freeze violation; that file contains `cfg(windows)` **exactly
zero times** and was convicted by its own line 97 comment,
`// Phase 26 contract enforcement happens HERE`. Both halves must hold. Fixed.

**2. `p26-deps-allowlist:ipc` — allowlist stale, widened with reasons.**
`aethercore-product-identity` is a workspace path dep added by P46 3.3 D1
(`71fc629`) so the Windows service name has one decider; `ipc` uses it at
`crates/ipc/src/windows_impl.rs:130` for the pipe security descriptor's service
SID. Removing it to satisfy the list would re-duplicate the service name — the
exact defect P46 removed. `tempfile` is a `[dev-dependencies]` entry used only
by `tests/unix_adversarial.rs`; the scanner matches every section whose name
*contains* `"dependencies"`, so dev-only crates are swept in as if shipped.
Neither is network-capable, which is what the companion `p26-no-network-deps`
check actually defends, and that check still passes untouched.

**3. `p27-main-mod-gates` — gate stale, re-baselined to what P31 decided.** The
check demanded the literal `#[cfg(all(unix, feature = "unix-ipc"))]` +
`mod composition;`. That cannot match a P31+ tree for two independent reasons:
P31 (W7) retired the manual feature gate — `main.rs:50-52` states the feature
survives only as a force-on alias for exotic hosts — and the module is named
`unix_composition`, not `composition`. Now asserts `#[cfg(unix)]` +
`mod unix_composition;`, so a *narrower* gate reappearing is still caught.

**Not fixed on purpose**: `p27-cfg-windows-freeze-scan` has the identical
precedence shape as (1) and never inspects `cfg(windows)` either — but it
currently passes, and a passing gate is outside that change.

**What is left failing, and why each needs its own phase:**

- **Wirefreeze maxima** (EventKind 30 vs 28, request 94 vs 88, response 93 vs
  50, envelope 39 vs 37). Every tag above the line is **additive** — security
  audit, assistant, performance window, journal export — which is correct
  protobuf evolution. These are §4 published wire contracts; re-baselining a
  frozen ceiling is not a gate repair and was not done unilaterally.
- **`p28-mutation-guard:offline.rs fs::write`.** Real, and deliberate:
  `keys_generate` (`apps/aetherctl/src/offline.rs:592`) writes a 32-byte seed at
  `0600` as a documented explicit owner action. Either it moves out of the
  embedded path or the gate narrows to that one write — an owner's decision
  about key material.
- **`p28-deps:allowlist-only`.** `aetherctl` gained six workspace path deps and
  `ed25519-dalek` since P28; the same point-in-time-snapshot problem as (2),
  wider.
- **`p28-renderer-sealed-snapshot-available`.** Environmental: it wants
  `AetherCore-Phase27-Master-Delivery.zip` extracted to `/tmp`. That file is
  1.13 GB and **is not tracked by git**.

**Should these run in CI? No — not as they stand**, and the last item is why in
one line: phase28 depends on a 1.13 GB archive that is not in the repository, so
a CI checkout can never satisfy it. More fundamentally they are point-in-time
*phase-acceptance* audits that pin exact maxima and exact dependency sets as of
their phase, and the tree is now past Phase 35. Phase 21/22/23 still pass
because their invariants are structural; 26/27/28 fail because theirs are
snapshots. Wiring them in today would take `main` from its first green run to
permanently red, for findings that are mostly staleness. The useful move is the
inverse: promote their genuinely continuous invariants (mutation guards,
network-dep bans, composition wiring, RAII) into the gates that already run at
steps 20–23, and either re-baseline the snapshots as additive-only rules or
retire them as phase artifacts. That is a phase of work, not a CI line.

## 3. The blocker: the seal and Dependabot cannot both be left alone

P68's last act (`218a180`) lifted the Dependabot pause that had held since P59,
on the measured condition that the windows job was green on `main`. Dependabot
then opened ten PRs. **Nine of them died in under two minutes.**

### 3.1 What was measured

| PR | ecosystem | files touched | windows | duration |
|---|---|---|---|---|
| #22 | cargo | `Cargo.lock`, `Cargo.toml` | fail | 1m56s |
| #21 | cargo | `Cargo.lock`, `crates/windows-update/Cargo.toml` | fail | 1m28s |
| #20 | npm | `apps/ui/package.json`, `pnpm-lock.yaml` | fail | 2m07s |
| #19 | cargo | `Cargo.lock` | fail | 1m08s |
| #18 | cargo | `Cargo.lock`, `Cargo.toml` | fail | 1m39s |
| #17 | npm | `apps/ui/package.json`, `pnpm-lock.yaml` | fail | 1m38s |
| #16 | cargo | `Cargo.lock`, `Cargo.toml` | fail | 1m28s |
| #15 | npm | `apps/ui/package.json`, `pnpm-lock.yaml` | fail | 1m30s |
| #14 | npm | `apps/ui/package.json`, `pnpm-lock.yaml` | fail | 1m21s |
| **#13** | **github-actions** | **`.github/workflows/ci.yml`** | **pass** | **1h03m** |

Nine of nine PRs touching `phase21-workspace/` fail. The one PR that does not
touch it is the one that passes, and it runs for an hour because it actually
reaches the gates. The failing step is the same in all nine — run `35474420447`,
job `105981083841`, PR #19:

```
Delivered source seal
  Source seal: FAILED - 1488 of 1489 tracked files verified, 1 problems
    hash         Cargo.lock
##[error]Process completed with exit code 1.
```

Steps 11 through 26 **skipped**. Seventeen gates — including the clippy wall
P68 spent eight round trips clearing — never ran on any of those nine diffs.

### 3.2 Why, in one sentence

`Delivered source seal` is step 10, it hashes every git-tracked file under
`phase21-workspace/`, a dependency bump by definition edits a git-tracked file
under `phase21-workspace/`, and Dependabot has no step that regenerates
`MANIFEST.sha256`. The seal is doing exactly what it was built to do. So is
Dependabot. The incompatibility is structural, not a bug in either.

It is also not new information about the seal: `docs/SOURCE_SEAL.md` has said
since P60 that the seal breaks on any deliberate edit and that you re-seal after
one. What P68 changed was the rate — from zero bump PRs to ten.

### 3.3 The option that was rejected, and why

The cheap fix is an exclusion: teach `source_seal.py` to skip `Cargo.lock` and
`pnpm-lock.yaml`, and all nine PRs go green with no human step. **Rejected.**

1. **It reintroduces the exact defect P60 removed.** The seal's rule is *every
   git-tracked file, no list* — and the whole of **What is inside it** exists
   because the previous rule was a directory walk minus a hardcoded exclusion
   list, which missed 14 files and caught none of them. An exclusion list is
   only ever as current as the last person who remembered it. There is no
   version of "just these two lockfiles" that stays two lockfiles.
2. **It puts the hole where it pays to have one.** The seal's stated question is
   *are the bytes in this tree the bytes the project sealed at this commit* — a
   transport-and-tamper check. A dependency lock is the single most
   tamper-relevant delivered file in the tree: it is what decides which code
   gets compiled into the product. A seal that covers 1,487 files and not the
   two that choose the dependency graph is worse than no seal, because it
   reports green.
3. **The cost it saves is one command.** Weighed against (1) and (2), the thing
   being bought is `python3 scripts/regenerate-source-manifest.py` typed once
   per merged bump.

**The decision: lockfiles stay inside the seal, and whoever merges a bump
re-seals it.** That makes the human step load-bearing, which means it has to be
written where the human will be standing — which is items 1 and 2 of this
phase's blocker work.

### 3.4 Where it is written down

`docs/SOURCE_SEAL.md` gains a named subsection under **Using it** — *Dependency
bumps: re-seal on the bump's own branch* — carrying the measurement above, the
rejection in 3.3, and the two commands **in order**:

```bash
python3 scripts/source_seal.py --json          # read this BEFORE re-sealing
python3 scripts/regenerate-source-manifest.py  # then re-seal; commit the manifest
```

`.github/dependabot.yml`'s header comment gains the same two lines, because the
person merging a Dependabot PR is reading that file's `open-pull-requests-limit`
and not necessarily the seal's documentation.

**The order is the substance, not presentation.** `regenerate-source-manifest.py`
reads nothing from the previous manifest — it hashes whatever is on disk. Run
first, it produces a valid green manifest over *any* tree, including one
carrying changes the bump had no business making. The `--json` read is the only
thing standing between "re-seal a bump" and "launder a diff into the seal". The
rule it enforces:

> Every object in `failed[]` must have `"reason": "hash"` and a `"path"` the
> bump was supposed to touch. A `hash` on anything else, or any `unlisted`,
> `missing`, `not_tracked` or `symlink` entry, means something other than a
> version bump rode in on the branch.

This is the same discipline that made P60's 202 conflicts classifiable —
classified **before** regenerating, because regenerating first destroys the
evidence of which kind they were — applied to a case that now recurs weekly.

### 3.5 The loop, run once end to end

PR **#19**, `uuid` 1.25.0 → 1.26.1, chosen because it is the only one of the five
open cargo bumps that is semver-compatible and touches nothing but the lockfile.
The point was to exercise the loop, not to fight an API break.

**1 — bring the branch up to date with `main` first.** `origin/main` at
`2215692` merged into the bump branch before anything else, so the manifest is
generated over the tree the PR will actually land as. Diff against `main`
afterwards: `Cargo.lock`, 3 lines.

**2 — read `--json` before re-sealing.**

```json
{"failed": [{"path": "Cargo.lock", "reason": "hash",
             "expected": "18c82ee5bfe5ca109444d87ada3ec97d9348f891dacc428c99fd1db042d14030",
             "actual":   "e56acd8057633f0a19f92bc316fd7d6655cdf0326ff326cfc23c8ad8132e3674"}],
 "listed": 1489, "tracked": 1489, "verified": 1488, "ok": false}
```

Exactly one entry, `reason: "hash"`, on the one path the bump was supposed to
touch. No `unlisted`, `missing`, `not_tracked` or `symlink`. The rule passes,
so re-sealing is legitimate for this branch.

Reading the diff itself also caught something `--json` cannot see and that is
worth one line: the bump moves `tempfile`'s `getrandom` edge from **0.4.3 down
to 0.3.4**. All three of 0.2.17, 0.3.4 and 0.4.3 were already resolved in this
lock, so no package enters or leaves the graph — only which of three existing
nodes `tempfile` binds to. Not a blocker; `deny-check` and all five
`cargo-fuzz` jobs were already green on this head.

**3 — re-seal and commit.**

```
$ python3 scripts/regenerate-source-manifest.py
MANIFEST.sha256 regenerated over 1489 tracked files (manifest excludes itself).
$ python3 scripts/source_seal.py
Source seal: OK - 1489 of 1489 tracked files verified
```

The manifest diff is **one line**, the `Cargo.lock` row, matching the `--json`
prediction exactly. Committed as `70cdde6`, pushed to the Dependabot branch.

**4 — the verdict.** Run **`35484566311`** at **`70cdde6`**:

| step | before (`a8d4e95`, run `35474420447`) | after (`70cdde6`) |
|---|---|---|
| 10 `Delivered source seal` | **failure** — `hash Cargo.lock` | **success** |
| 11 `Verify approved dependency freeze` | skipped | **failure** |
| 12–26 | skipped | skipped |

**The loop works, and it is not sufficient on its own.** Step 10 is green,
which is what this blocker set out to fix and what items 1 and 2 document. Step
11 is a *different instrument asking a different question*, and
`docs/SOURCE_SEAL.md`'s own table has said so since P60:

| question | instrument |
|---|---|
| does this source match the bytes we sealed? | **the seal** |
| does the dependency graph match an approved one? | `release/dependency-freeze.json` + `freeze-dependencies.ps1 -VerifyOnly` |

A dependency bump legitimately changes the answer to both, and only the seal's
answer is mechanical:

```
Exception: phase21-workspace\scripts\freeze-dependencies.ps1:43
  Dependency lock hashes differ from the approved freeze baseline. Review
  dependency-manifest/lock changes and refresh only on a freeze source that
  meets docs/RELEASE_SUPPLY_CHAIN.md's three criteria.
```

**The freeze was not refreshed, and that is a decision, not an omission.**
`docs/RELEASE_SUPPLY_CHAIN.md` is unambiguous — *"a refresh mints a candidate,
never an approved graph. Approval is the commit, and the commit is made by a
person. No workflow in this repository can approve a dependency graph, because
no workflow can commit one."* Its three criteria are:

1. the resolution is reproducible on an independent machine;
2. the run leaves a durable third-party record naming the exact source commit;
3. **the graph it produces is compiled on the machine that produced it, before
   the output leaves that machine.**

This host fails (3) outright — see §5, no `cargo` command that runs a build
script works here — and cannot satisfy (1) alone. So **#19 stays open at step
11**, with its seal fixed and its remaining failure being the one that requires
an owner. That separation is the correct outcome: the mechanical wall is gone,
the judgement call is visible instead of hidden behind it.

Both documents were amended after this run to say so, because a procedure that
gets you to step 10 and leaves you surprised at step 11 is half a procedure.

**Recorded, per the item:** PR **#19**; resulting sha **`70cdde6`**
(`70cdde64a7403cec0da83d0dee94d0ad215da29f`); run **`35484566311`**; `Delivered
source seal` **green**.


## 4. `#13`, the one PR the blocker does not touch

`actions/cache` 4 → 6, `.github/workflows/ci.yml` only. Dependabot's
`github-actions` ecosystem is configured at `directory: "/"`, which is the
repository root, and the seal covers `phase21-workspace/`. That is the known gap
`docs/SOURCE_SEAL.md` describes under **The known gap** — the four workflow
files that gate and build the product are sealed by nothing — showing up for
once as an advantage.

Squash-merged at **`2215692`**, green beforehand on its own head `76a5462`:
windows pass (1h03m), deny-check pass, all five cargo-fuzz jobs pass.

## 5. What this host could not verify, and why

**No `cargo` command that must run a build script works on this Mac.** Not for
want of a C toolchain — the *host* linker is broken:

```
warning: failed running `"xcrun" "--sdk" "macosx" "--show-sdk-path"` to find MacOSX.sdk
note: You have not agreed to the Xcode license agreements.
error: linking with `cc` failed: exit status: 69
```

Reproduced on `aethercore-contracts`, whose build script is `prost-build` and
contains no C at all. The same unaccepted licence makes `/usr/bin/git`
unusable, which matters beyond the terminal: `scripts/source_seal.py:88` and
`regenerate-source-manifest.py` both shell out to **`git` by name**, so on this
host they resolve to the broken shim and fail. Both were run here against a
working `git` placed ahead of `/usr/bin` on `PATH`; that is a workaround for
this session and not a repository change, and the scripts will fail for any
operator on a machine in this state.

So every Rust figure in this report comes from the runner. The python gate set
needs no compiler and was run locally in full.

## 6. Open after P69

1. **The Xcode licence.** Until `sudo xcodebuild -license` is accepted, this
   host cannot compile, test or lint any Rust in this workspace, and cannot run
   the seal scripts without a `PATH` workaround. Every Rust verdict costs a
   ~50-minute CI round trip. This is the largest single drag on the phase loop
   and it is fixed by one command that needs a password.
2. **The approved dependency freeze blocks every cargo and npm bump at step 11,
   and no amount of re-sealing helps.** `release/dependency-locks.sha256` hashes
   `Cargo.lock` and `pnpm-lock.yaml` directly, so any bump breaks it by
   definition. Refreshing it is an owner action under
   `docs/RELEASE_SUPPLY_CHAIN.md`'s three criteria, criterion 3 requires
   compiling the graph on the machine that produced it, and this host cannot
   compile. **Until an owner refreshes the freeze on a qualifying source, no
   dependency bump in this repository can be merged green** — which means the
   Dependabot pause P68 lifted has been replaced by a wall one step further on.
   That is the single highest-value thing to fix next, and it is not a code
   change.
3. **Eight open Dependabot PRs still need the loop** (#14–#18, #20–#22). Each is
   one `--json` read plus one regenerate; #19 shows the shape. The two majors
   (#16 reqwest 0.12→0.13, #18 rand 0.9→0.10) and the two large ones (#21
   windows-core 0.62→0.100, #22 criterion 0.5→0.8) are API decisions with their
   own re-measurement, not bumps, and should be judged as such — but none of
   them can be judged past step 11 until (2) is resolved.
4. **`--keep-going` is still missing from `verify-enterprise.ps1:24`** (P68 §7).
   With the host's Rust toolchain unavailable it now costs more than it did.
5. **`p27-cfg-windows-freeze-scan` carries the precedence bug of §2(1)** and is
   currently passing, so it was left alone. It will accuse the wrong file the
   first time a `phase 27` comment is added to a `cfg`-free source file.
6. **The seal still does not cover the repository root** — the known gap. #13
   passing is a reminder that `.github/workflows/{ci,fuzz,release}.yml` and
   `.github/dependabot.yml` change without any seal noticing.
