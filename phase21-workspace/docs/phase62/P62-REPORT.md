# P62 — the report

Host: the Mac at `/Users/hasanalaaa/dev/aethercore`. Date: 2026-09-13. Branch `main`
throughout. Every figure below names the command, the commit or the run id that
produced it; where a claim is reasoning rather than measurement it says so, and two
of them are.

The short version. **`cargo test --locked --workspace` passes on Windows** — 135
binaries, 660 passed, 0 failed, run `34766915417` step 13 — for the first time in this
repository's history. P61 got the suite to compile and measured 5 binaries; getting
from 5 to 135 took four defects, and **all four are the same defect**: a function
reaching past its own injectable seam for the real machine, invisible on macOS because
the Windows call degrades to a no-op or a default. Steps 14, 15 and 16 are green
behind it, and the job now stops at **step 17 on `DBT-P61-001`**, exactly where P61
said it would. That set is classified — **6 tree / 81 check / 21 unread** — and nothing
in it was fixed, because the two halves have opposite cures.

---

## Every CI attempt, in order

| run | at | reached | step 13 asserted | outcome |
|---|---|---|---|---|
| — | `f2ddec9` (P61) | step 13 | 5 binaries, 17 passed, 2 failed | the P62 baseline |
| `34758155671` | `d8662a1` | step 13 | **20 binaries**, 107 passed, 6 failed | `DBT-P61-002` gone; `driver-hub` OEM authority |
| `34759620649` | `bb2abf9` | step 13 | 20 binaries, 113 passed, 1 failed | 6 → 1; the survivor drives the production scan path |
| `34760963140` | `fbb31b4` | step 13 | **68 binaries**, 493 passed, 2 failed | `security-audit` owner-scope |
| `34762268577` | `254da25` | step 13 | 68 binaries, 493 passed, 2 failed | **nothing measured — the commit was empty** |
| `34763617017` | `4897b4c` | step 13 | 5 binaries, 19 passed, 1 failed | a race P62 itself made reachable |
| `34765013157` | `fa99a33` | step 13 | 33 binaries, 250 passed, 4 failed | embedded generation, first measured on Windows |
| `34766915417` | `9fe48ea` | **step 17 of 23** | **135 binaries, 660 passed, 0 failed, 11 ignored** | `DBT-P61-001` |

The column that matters is the fourth. A step's conclusion says `failure` in seven of
those rows and the same word hides 5 binaries and 68.

---

## ITEM 1 — `DBT-P61-002`, and the audit of the other three

### The change

`run_cleanup` called a free `acquire_cleanup_mutation_guard()` that reached the real
`%ProgramData%\AetherCore\state\machine-mutation.lock`, bypassing the `CleanupPlatform`
every other effect in those tests is injected through. It is now
`CleanupPlatform::acquire_mutation_lease`. `d8662a1`.

Production is unchanged: `WindowsCleanupPlatform` acquires the same
`MachineMutationGuard` with the same `OPEN_EXISTING` + `LockFileEx` semantics. The
method has **no default body** — a platform that mutates the real machine must name its
own cross-process boundary, and a no-op default would let a new platform inherit "no
lock at all" without anyone choosing it. The non-Windows stub returns
`UnsupportedPlatform` like its siblings, rather than the silent no-op the free function
handed it.

Measured, not assumed: the lease is `Box<dyn Any>`, not `Box<dyn Send>`. `HANDLE` has
no `Send` impl in `windows 0.62.2`, and a scratch crate cross-checked for
`x86_64-pc-windows-msvc` rejects the `Send` form —

```
error[E0277]: `*mut c_void` cannot be sent between threads safely
   note: required because it appears within the type `MachineMutationGuard`
   note: required for the cast from `Box<MachineMutationGuard>` to `Box<dyn Send>`
```

— so the `Send` spelling would not have compiled on the runner. `run_cleanup` takes and
drops the lease on one thread.

### The audit — one of the three had the same defect

| call site | where the guard sits | injectable seam above it | verdict |
|---|---|---|---|
| `cleaner/src/lib.rs:814` | free fn beside the platform | `CleanupPlatform` + fake | **same defect** — fixed |
| `startup-manager/src/lib.rs:974` | free fn beside the platform | `StartupPlatform` + in-crate `MockPlatform` | **same defect** — fixed |
| `system-repair/src/windows_impl.rs:94` | inside `WindowsRepairPlatform::repair` | `RepairPlatform` + fake | clean |
| `windows-update/src/execution_windows.rs:135` | inside `execute_driver_updates` | reached only via `InstallPlatform::execute_wua` | clean |

The distinction is one thing: whether the guard sits *inside* the injectable boundary
or *beside* it. `startup-manager`'s was beside it and driven by
`disable_then_restore_preserves_auditable_change_chain` through the mock — it would have
been step 13's next failure.

### What the gate caught, and the order that caught it

`static_validate.py`'s `cleanup_immutable_state_and_limits` asserts that the cleanup
path takes a mutation boundary, by naming the function. The rename broke it.
**Measured against `HEAD` first**: that check passed at `0512b1d` and this change broke
it, so the token is repointed and the invariant is unchanged. Running the gates before
touching anything is what made that a two-minute correction instead of a false entry in
the classification.

---

## ITEM 2 — the job, step by step, and the four defects between 5 and 135

### `DBT-P62-001` — `match_inventory` read the host's SMBIOS

Run `34758155671` stopped on six `driver-hub` failures naming an authority the fixtures
never mention:

```
left:  ["microsoft.windows-update:Offline", "oem.microsoft-surface:ManualAuthorityRequired"]
right: ["microsoft.windows-update:Offline"]
```

`match_inventory_with_overrides` called `MachineProfile::collect_local()` from inside
itself. On the runner the manufacturer is Microsoft, `classify_machine_kind` returns
`Oem`, and a second required authority appears. Invisible here because
`machine_identity()` falls back to `unwrap_or_default()` — `MachineKind::Unknown`, for
which `oem_provider_for_machine` returns `None` before it looks at a vendor.

`machine: &MachineProfile` is now a parameter. Matching is pure — it decides which
authorities a device requires — so the machine it decides against is an input. The two
production call sites supply the real one. `bb2abf9`.

That fixed 6 of 7. The survivor injected a `DiscoveryBackend` and drove the real scan
path, which collected the real machine **one layer further out** — the seam was in the
wrong place, not missing. `DriverHubInner` now holds a `MachineProfile`, and all six
`with_backend` call sites in the test module take `test_machine()`, not only the one
that failed, because the next test to assert authority coverage would have found this
again. `fbb31b4`.

Pinned in both directions by `authority_requirements_follow_the_machine_passed_in_not_the_host`,
which reproduces the runner's `ManualAuthorityRequired` from an OEM fixture **on this
Mac** — a test that could not be written while the function read the host.

### `DBT-P62-002` — a short name is an alias too

Run `34760963140` reached 68 binaries and stopped on `security-audit`:

```
the caller's own scope is allowed: OutsideOwnerScope {
  path: "C:\Users\RUNNER~1\AppData\Local\Temp\...\owned" }

a_link_planted_inside_an_owned_root_is_refused_not_followed
  left: "sec.targetOutsideOwnerScope"   right: "sec.reparsePointRefused"
```

`OwnerScope::new` canonicalises its roots; a caller-named path arrives however the
caller spelled it; and `resolve_within_roots` re-resolved only at a reparse point. A
Windows 8.3 short name is a second kind of alias for one directory entry and is **not**
a reparse point, so that branch never fired. Invisible on macOS because *there* the
aliasing is a symlink — `std::env::temp_dir()` is `/var/folders/...` and `/var` links to
`/private/var` — which the existing branch already handled.

Fail-closed, so not an escalation. Two real consequences all the same: a caller is
refused a path it genuinely owns, and in the link case **rule 4 never engaged** — the
refusal came from the containment branch, so the reparse-point guard was not what
stopped that link.

Containment now compares the canonicalised prefix while `inside` is false, falling back
to the raw prefix when it does not exist yet. It cannot weaken rule 4: it runs only
while `inside` is false and can only make `inside` true for a path that genuinely
resolves into a root — which then subjects every later component to the reparse
refusal. Strictly more paths are checked. `4897b4c`.

### The empty commit

`254da25` claimed that fix and contained the ledger row and the manifest — two files,
no code. The `cd` at the head of the command failed, the `&&`-chained edit never ran,
and the `git add` of `scope.rs` staged nothing. Run `34762268577` then re-measured the
identical failure, which is the only reason it surfaced.

Two things let it through: the local tests passing (21 of 21) were read as
confirmation when they had always passed on macOS and so could not distinguish "fixed"
from "unchanged"; and `git diff --cached --stat` was not read before committing, which
would have shown one file missing. Every commit after it checks the staged diff.

### `DBT-P62-003` — a race this phase made reachable

Run `34763617017` fell back to 5 binaries on

```
cleanup_failure_after_deletion_barrier_requires_recovery_review
  assertion failed: status.recovery_required
```

with `status.mutation_started`, the line above it, passing. `status()` composes
`plan_state` from the operation engine and the record fields from the database, and
`fail_cleanup` transitions the plan **before** it upserts the record. A loop keyed on
`plan_state` reads the record one write early. `mutation_started` passed because the
Executing update wrote it long before.

The race is older than this phase, but P62 made it reachable: `d8662a1` deleted the P36
test-local serializer — correctly, since it existed only to work around the real
machine-wide lock `DBT-P61-002` was about — and the two execution tests began running in
parallel. The same race was hit while writing the new lease test, fixed there, and its
neighbours were not checked. That was the mistake.

`wait_terminal` polls until `plan_state` and `stage` **both** read the terminal value;
`stage` falls back to the record, and `fail_cleanup` publishes no telemetry, so it
cannot go terminal before the upsert. Three hand-rolled loops became one helper. Held by
**40 consecutive runs at `--test-threads=4`, 0 failures** — a race that reproduced in CI
means "it passed once" is not evidence. `fa99a33`.

### `DBT-P62-004` — the one that is reasoning, and is filed OPEN

Run `34765013157` reached 33 binaries and lost four of five generation tests to
`deadline exceeded after 0 token(s)`, two of them logged as running over 60 seconds.

**Not deterministic**: run `34760963140` ran the same binary on the same code and passed
it. P61 said generation had never been measured on the platform the product ships on.
It had then been measured twice, with opposite results.

`ASSISTANT_DEADLINE` is 20 seconds, covers generation only, and is a **product
constant** — what a user is promised. Widening it to turn a test green would change the
product to suit the harness, so it is untouched. The cause acted on is scheduling:
`cargo test` runs a binary's tests on parallel threads, so four llama contexts over a
1.5B Q4 model compete for the runner's two cores. They are serialised on a test-local
mutex.

This is the **opposite case** to the P36 lock deleted in `d8662a1`: that one hid a real
defect, this one stops a throughput measurement being taken under 4× CPU
oversubscription — a state the product never runs in. Same mechanism, opposite
justification, which is why the reason is written into the code.

Cost, measured here: the binary goes from 42.46 s (P61, parallel, Metal) to
**156.64 s** serialised. `9fe48ea`.

**Filed OPEN.** Run `34766915417` then passed it, making the score two passes and one
failure with the serialisation present in only the last. That is enough to stop being
the blocker and not enough to call 20 seconds reliable on 2 vCPUs. The row asks for a
run that measures the generation wall time there rather than only its exit code.

---

## ITEM 3 — `DBT-P61-001`, classified, nothing fixed

`86dbad4`, and `scripts/classify_gate_failures.py` reproduces it:

```
cd phase21-workspace && python3 scripts/classify_gate_failures.py
  FORMAT 47   ABSENT 22   MIXED 3   UNCLASSIFIED 36   TOTAL 108
```

It runs each gate in-process with its own token helper wrapped (`marker` in
`static_validate.py`, `has` in the other five), so every verdict comes from the gate's
own sources and its own tokens. It re-implements no check.

**6 tree / 81 check / 21 unread.** The 108 is the same 108 before and after every P62
commit — nine gates against a `git worktree` at `0512b1d` and against the working tree,
identical names every time.

**Confirmed on Windows.** Run `34766915417` step 17 reports `checks: 347` and the same
28 failing names in the same order as this Mac. The classification holds on the
platform that matters, name for name.

### Why most of it is the gate's

47 fail only because the tokens are spelled with the whitespace removed —
`hardware_gate:IsolationGate`, `record.claimed=true`, `if args.len()!=5` — and the tree
is `rustfmt`-clean. A further group fails one step on, where `rustfmt` also added a
trailing comma when it wrapped a signature; each of those was read to the line.

### The cluster that looks like lost safety and is not

Eight `ABSENT` checks name one IPC design that is genuinely gone. `482d480`, an ancestor
of `main`, removed it deliberately: *"Windows request/response has never worked ...
every ServiceJob verb hung."* Satisfying those checks would reintroduce the defect that
commit fixed.

Four neighbours that pattern-match into that cluster are renamed identifiers, and one
matters here: `ensure_mutation_lock_file`, `MACHINE_MUTATION_LOCK_RELATIVE_PATH` and the
`*S-1-5-18:F` / `*S-1-5-32-544:F` ACLs are all present in the install-hardener. **The
machine-mutation lock really is installer-provisioned** — P61's description of
`DBT-P61-002` is confirmed, not undermined.

### The six that are the tree's

`deny.toml` with `bans.wildcards = "warn"` and three `sources` policies unset; a UTF-8
BOM in four `docs/phase36/evidence/*.json`; 10 of 80 buttons without `use:fluidPress`,
8 of them in `FleetPage.svelte`; `main.rs` and `router.rs` at 1.8× and 7.8× their P10
line budgets; `--ac-material-elevated` defined nowhere; `noRedirectionBitmap` absent
from the window config.

`deny.toml` is the one that matters beyond its check count: a supply-chain policy that
is simply not switched on, in a repository whose whole release story is a dependency
freeze.

### The two that are unresolved, and say so

`ipc_pipe_create_instance_regression_test` and
`ipc_production_pipe_owner_and_server_authority_are_service_scoped` expect the
authenticated-users ACE `(A;;0x00120003;;;AU)`; `windows_impl.rs:286` builds
`(A;;FR;;;AU)(A;;0x00000002;;;AU)`. Not a rename and not whitespace —
`FILE_GENERIC_READ | FILE_WRITE_DATA` is not `0x00120003`. The tree still declares
`EXPECTED_CLIENT_ACCESS: u32 = 0x0012_0003` and asserts `PIPE_CLIENT_ACCESS_MASK`
against it, so **the constant and the descriptor disagree with each other**. That is an
access-mask question, not a grep, and it is the one row in the 108 that could be a live
security difference. Counted as unread.

---

## ITEM 4 — Dependabot

**Not attempted.** The condition was "only if the job is green on `main`". It is not:
run `34766915417` stops at step 17 of 23 on `DBT-P61-001`. Six steps remain unexecuted
behind it — both Zenith gates, enterprise convergence, the supply-chain audit, the
unsigned candidate build and the artifact upload — and they have asserted nothing.

---

## The ledger

Counted by the rule in `docs/LEDGER.md` § 1, scoped to § 1, with the same script on
both trees:

| | rows | open | malformed |
|---|---|---|---|
| P61 end (`0512b1d`) | 39 | 17 | 1 |
| P62 end | **43** | **17** | 1 |

**Closed:** `DBT-P61-002`, `DBT-P62-001`, `DBT-P62-002`, `DBT-P62-003`.
**Entered:** `DBT-P62-001` through `DBT-P62-004`. **Narrowed:** `DBT-P62-004` (green
once on the runner, serialised). `DBT-P56-002` stays four-celled, named rather than
dropped, as in P58–P61.

Open is unchanged at 17 because `DBT-P61-002` closed and `DBT-P62-004` opened.

---

## What measurement contradicted in the brief

1. **"audit the other three call sites that take the guard the same way."** Framed as an
   audit of three; one of them, `startup-manager`, had the identical defect and needed
   the identical fix, so ITEM 1 was two fixes rather than one plus a census.

2. **"note only 5 test binaries have executed so far."** Correct at the start and the
   number moved four times: 5 → 20 → 68 → 33 → **135**. It is not monotonic, and the
   dip to 33 and to 5 are the interesting rows — `cargo test` stops at the first failing
   binary, so the count says where the run stopped, not how much works.

3. **P61's "nothing has ever measured generation on the platform it runs on."** True
   when written. It has now been measured three times with two different answers, and
   the honest status is *marginal on 2 vCPUs*, not *works on Windows*.

4. **ITEM 3's "start with `static_validate.py` because it is what stops step 17."**
   Accurate, and confirmed: the job now stops at step 17 and step 17 reports exactly the
   28 names classified here.

---

## Commits

| commit | item |
|---|---|
| `d8662a1` | ITEM 1 — the mutation boundary through `CleanupPlatform` and `StartupPlatform`. `DBT-P61-002` |
| `86dbad4` | ITEM 3 — the 108 classified, nothing fixed |
| `bb2abf9` | ITEM 2 — the machine is a parameter. `DBT-P62-001` |
| `fbb31b4` | ITEM 2 — the hub holds its machine too |
| `254da25` | **the empty commit** — ledger row without its code |
| `4897b4c` | ITEM 2 — the scope fix itself. `DBT-P62-002` |
| `fa99a33` | ITEM 2 — wait for both writers. `DBT-P62-003` |
| `9fe48ea` | ITEM 2 — one model at a time. `DBT-P62-004`, OPEN |

---

## The single next action

`DBT-P61-001`'s check half: one edit to how the six gates compare — normalise whitespace
before matching, and re-read the handful the formatter also re-punctuated. That is 81 of
the 108 and it is what stands between step 17 and the six steps behind it, none of which
have ever run. The 6 tree failures are six separate small decisions and only `deny.toml`
touches a security posture. Do not start the two unresolved pipe-descriptor rows as part
of that edit — they need an access-mask answer, not a comparison fix.
