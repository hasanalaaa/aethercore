# P58 — the report

Host: the Mac at `/Users/hasanalaaa/dev/aethercore`. Date: 2026-09-12.
Ledger rows moved in `docs/LEDGER.md`; every figure below is reproducible by the
command printed beside it.

The short version: the three workflows were never workflows, and making them
real made them fail. That is the point. One of the three is green, one queues
because the machine it wants does not exist, and one fails at its first step on
a release blocker that has been committed since Phase 19 and whose stated reason
is still true. Nothing was removed, skipped, or marked `continue-on-error` to
change any of that.

---

## ITEM 1 — `PageId` no longer claims the assistant is a route

`apps/ui/src/lib/navigation.ts:4`. `'assistant'` removed from `PageId`, kept in
`IconName`.

Nothing depended on the page membership, checked three ways: no `NAVIGATION`
entry has that id, no call site passes it to `setPage`, and `verify-arabic
--page assistant` compares a CLI string (`verify-arabic.mjs:39`), not a typed
`PageId`.

| check | expected | observed |
|---|---|---|
| `npm run check` | 0 errors, 0 warnings | **229 files, 0 errors, 0 warnings**, exit 0 |
| `npm run build` | exit 0 | **exit 0** |
| `Ctrl+/` opens and focuses the drawer, every screen | 12/12 | **12/12**, `document.activeElement.id === 'assistant-input'` on each |

The third row was measured over CDP against the fixture, routing to each screen
by its nav button and dispatching the real `keydown`, so it exercises the path
a user has. The script was a one-off and is not committed; its output is
twelve `PASS` lines, one per screen.

Commit `85641e1`.

---

## ITEM 2 — the gates

### 2.A — why they never dispatched

Measured against the API, not inferred from the filesystem:

```
$ gh api repos/hasanalaaa/aethercore/actions/workflows --jq '.total_count'
1
```

One. `.github/workflows/windows-installer.yml`. GitHub Actions reads
`.github/workflows/` at the repository **root** and nowhere else, and
`ci.yml`, `fuzz.yml` and `release.yml` sat in
`phase21-workspace/.github/workflows/`. They were not disabled, not filtered by
a path or branch rule, and not failing. **They were never workflows at all** —
three YAML files that GitHub had no reason to look at.

`DBT-P55-002` is the corroboration rather than a coincidence: `ci.yml`'s own
`cargo fmt --all -- --check` step would have failed on the first push, and
1,254 violations accumulated instead.

The same defect covered `.github/dependabot.yml`, which also sat one directory
down — while `release.yml`'s header comment asserts "Dependabot is enabled to
review future updates". Its `directory` values were wrong too (`/` and
`/apps/ui`, neither of which holds a manifest at the repo root) and were
corrected to `/phase21-workspace`, `/phase21-workspace/apps/ui`, and `/` for the
github-actions ecosystem, which is where the workflows now live.

After the move:

```
$ gh api repos/hasanalaaa/aethercore/actions/workflows --jq '.total_count'
5
356644539  active  .github/workflows/ci.yml
356644541  active  .github/workflows/fuzz.yml
356644542  active  .github/workflows/release.yml
353735631  active  .github/workflows/windows-installer.yml
356644581  active  dynamic/dependabot/dependabot-updates
```

Corrections made for the new location, in `8888d31`:
`defaults.run.working-directory: phase21-workspace` on each (as
`windows-installer.yml` already did); every `upload-artifact` `path:` prefixed
explicitly, because `defaults.run` does not reach `uses:` inputs;
`workflow_dispatch` added to `ci.yml`; and `continue-on-error: true` removed
from `fuzz.yml`'s fuzz step.

### 2.B — the formatting

| | before | after |
|---|---|---|
| `cargo fmt --all -- --check` | exit 1 | **exit 0**, empty output |
| hunks (`Diff in <path>:<line>:` lines) | **1,254** | 0 |
| distinct files | **128** | 0 |

**The ledger's old figure was wrong in kind.** `DBT-P55-002` recorded "1,247
files". That number counts `Diff in` lines, which are hunks — one file appears
once per hunk. Today's equivalent figure is 1,254, across 128 files. The row now
carries both so the next reader is not misled.

Committed as `b4023ec`, 128 `.rs` files and nothing else, so the diff reads as
exactly what it is. Proof the reformat is inert, taken by exit code on both
sides of the change rather than by reading a piped tail:

```
cargo check --workspace --locked   before: exit 0, 85 warnings, 0 errors
cargo check --workspace --locked   after:  exit 0, 85 warnings, 0 errors
```

The 85 are pre-existing macOS dead-code warnings on platform-gated code.

### 2.C — making them run, and what running them found

**All three dispatch.** Two of the three do not pass, and both reasons are real.

#### fuzz — green, after a defect that had made it meaningless twice over

The first real dispatch failed all five matrix legs with one error, reproduced
locally at exit 101 before it was touched:

```
$ cargo metadata --manifest-path fuzz/Cargo.toml
error: current package believes it's in a workspace when it's not:
current:   phase21-workspace/fuzz/Cargo.toml
workspace: phase21-workspace/Cargo.toml
```

`fuzz/Cargo.toml` had no `[workspace]` table and the parent workspace neither
included nor excluded it, so cargo refused the manifest before compiling
anything. **`cargo fuzz build` and `cargo fuzz run` have never succeeded for any
target in this repository.**

So the fuzz gate was fiction on two independent counts: it was never dispatched,
and had it been, `continue-on-error: true` would have reported success while
every leg failed to build. Both are gone. Fixed with the empty `[workspace]`
table `cargo fuzz init` generates — the crate stays out of the parent workspace
deliberately, because it needs a nightly toolchain and a sanitizer runtime and
must not be dragged into `cargo check --workspace --locked`.

After: `cargo metadata` exit 0, five bin targets resolved;
`cargo check --workspace --locked` unchanged at exit 0 / 85 warnings / 0 errors.
Commit `22e4a21`. `DBT-P58-002`.

Then the next dispatch failed on a second, independent defect that the first
had been hiding:

```
error: the option `Z` is only accepted on the nightly compiler
```

`phase21-workspace/rust-toolchain.toml` pins `channel = "1.97.1"`, and a
toolchain file outranks the installed default for every cargo and rustc
invocation inside that directory. So `dtolnay/rust-toolchain@nightly` installed
nightly and **nothing used it** — cargo-fuzz's `-Zsanitizer=address` went to
stable. Reproduced locally: `rustc -Zunstable-options --version` inside the
workspace gives the identical error, and `RUSTUP_TOOLCHAIN` is what overrides
the file. Set on the step, so the `cargo build` cargo-fuzz spawns inherits it.
This was only ever reachable once `DBT-P58-002` was fixed — cargo refused the
manifest before the compiler was invoked at all. `DBT-P58-006`, `a8946bb`.

A related finding, recorded and not fixed: **seven of the twelve sources in
`fuzz/fuzz_targets/` have no `[[bin]]` entry** — `ipc_frame`,
`operation_state`, `pii_redaction`, `scheduler_eligibility`, `support_archive`,
`update_manifest`, `windows_multisz`. The matrix's five match the manifest's
five, so the gate is honest about what it runs and silent about what it does
not. Wiring them means seven crate dependencies and compiling code that has
never built once; that is its own phase. `DBT-P58-004`.

#### ci — `deny-check` green, `windows` fails at step 1 of 16

Run `34707875531`, commit `8888d31`:

| job | conclusion |
|---|---|
| `deny-check` | **success** |
| `windows` | **failure**, at step 1 of 16 |

```
Exception: phase21-workspace\scripts\freeze-dependencies.ps1:44
Dependency freeze release blocker is still present. Refresh on the trusted
workstation before verification can pass.
```

`release/dependency-freeze.blocker.json` — `OMEGA-RB-001`, `severity:
release_blocker`, `status: OPEN` — is committed, and has been since `b024df8`,
the Phase 19 source drop.

**Its stated reason is still true**, measured rather than assumed. It names five
files; three of them do not exist:

| file | state |
|---|---|
| `Cargo.lock` | PRESENT |
| `pnpm-lock.yaml` | PRESENT |
| `release/dependency-locks.sha256` | **ABSENT** |
| `release/dependency-manifests.sha256` | **ABSENT** |
| `release/dependency-freeze.json` | **ABSENT** |

So `-VerifyOnly` would fail at `Assert-Lines` even with the blocker file
deleted. The dependency freeze set has never been generated. `windows-installer.yml`
goes green because it runs `-Refresh`, not `-VerifyOnly` — **no workflow in this
repository has ever passed `-VerifyOnly`.**

The blocker was not removed, the step was not skipped, and nothing was marked
`continue-on-error`. Its own closure text requires `-Refresh` on "the trusted
dependency-freeze workstation" followed by a review of the resulting graph.
That is an approval of the current dependency graph — a risk acceptance, not a
measurement — and `DBT-P55-007` already records that a developer machine cannot
verify signature enforcement, so producing the freeze set on one would carry
precisely the weakness the register names. Owner-gated: §3 row 5.
`DBT-P58-001`.

One further correction in `22e4a21`: `ci.yml`'s `push:` was unfiltered, so once
it became real every pull-request branch ran the ~1 h Windows job twice — once
for the branch push, once for the PR. The four Dependabot PRs opened the moment
Dependabot started demonstrated it immediately. Narrowed to `branches: [main]`;
PR branches stay covered by `pull_request`.

#### release — dispatches, and queues forever

Run `34707902097`, `workflow_dispatch`, status `queued` with the single job
`signed-release` queued behind it. It will not start, and all three reasons were
measured:

| precondition | API | value |
|---|---|---|
| a runner matching `[self-hosted, windows, x64, aethercore-signing]` | `actions/runners` | `total_count: 0` |
| the `production-signing` environment | `environments` | `total_count: 0` |
| `AETHERCORE_CODESIGN_THUMBPRINT`, `AETHERCORE_TIMESTAMP_URL` | `actions/variables` | `total_count: 0` |

With zero variables the "Verify signer configuration" step would throw even if a
runner appeared. **This is the deferred code-signing certificate, not a defect
in the file.** What would make it run, in order: a production Authenticode
certificate; a patched self-hosted Windows runner registered with those four
labels and the certificate non-exportable in its store; a `production-signing`
environment; and the three repository variables. `DBT-P58-003`.

---

## The move broke ten gates, and the way it broke them is the finding

Moving the workflows to the repository root broke every gate script that reads
them, because all of them resolve `.github/...` against the **workspace**.
Caught by running them, not by reading the diff.

`static_validate.py` — a `ci.yml` step — crashed outright:

```
FileNotFoundError: .../phase21-workspace/.github/workflows/ci.yml
```

**The other eight were worse than a crash.** Their `read()` / `text()` helpers
have the shape `p.read_text(...) if p.exists() else ''`, so they kept reporting
normally while reading `""` for `ci.yml` and `release.yml` — and **any check
asserting that something is absent from a workflow would have passed against a
file it never opened.** That is precisely the instrument-that-lies shape this
project has been bitten by four times, and it would have been introduced by the
commit whose entire purpose was fixing that class of defect.

Repaired in `a8946bb` by resolving `.github/` against the repository root while
every other path stays workspace relative — a `workflow_root()` helper in the
eight helper-based audits, a `REPO` constant in `static_validate.py` and
`phase31-adversarial-audit.py`, and `tools/lint_fuzz_workflow.py` moved off a
cwd-relative `open` to a path derived from its own location.

Verified by running every script on both sides of the move and comparing the
failure sets:

| script | before (`b4023ec`) | after |
|---|---|---|
| `static_validate.py` | 347 checks, 29 failed | **347 checks, 29 failed** |
| `enterprise-adversarial-audit.py` | 14 failed | **14** |
| `phase13-reliability-audit.py` | 6 | **6** |
| `phase14-scheduler-audit.py` | 5 | **5** |
| `phase15-security-audit.py` | 29 | **29** |
| `phase16-ga-audit.py` | 0 | **0** |
| `phase31-adversarial-audit.py` | — | **output identical** |
| `phase35-adversarial-audit.py` | — | **output identical** |
| `zenith-adversarial-audit.py` | 0 | **0** |
| `zenith-recursive-audit.py` | 26 | **26** |
| `tools/lint_fuzz_workflow.py` | `FileNotFoundError` | **`FUZZ_WORKFLOW_LINT_OK`** |

**Newly failing: none. Newly passing: none.** The non-zero counts are
pre-existing — these gates are written for the Windows CI runner and have always
failed a subset on macOS. That they are unchanged in *both* directions is the
point: a path fix that made a check start passing would be as suspicious as one
that made it start failing.

**What was fixed is one instance; the class is still open.** The fail-open
reader is in ten scripts and was not touched — the next file that moves or gets
renamed does the same thing just as silently. `DBT-P58-005`. It was not folded
into a commit whose job was repairing a regression, and it is a sweep across ten
files rather than a line.

One process note, recorded because it cost time: several of these audits
**write** files as a side effect — running them regenerated `SBOM.cdx.json` and
three `PHASE_30_BINARY_SAFE_PATCH/` artefacts. Those writes were discarded with
`git checkout --` and are not in any commit of this phase.

---

## ITEM 3 — the icon

**It regenerates byte-identically, and the row's premise was a measurement
error.**

`design/icon/aethercore-mark.svg` **is in the tree and always has been**:
tracked on `main`, added in `25a6a5a`, sha256
`65f441712c7fc8e21d5969001c027be05dd908380b06d08623edf5772b36441f` — equal to
`SOURCE.json`'s `sourceSha256` to the character. It is byte-identical to the
copy in the design worktree.

P55's probe was `Test-Path design/icon/aethercore-mark.svg` with the working
directory at `phase21-workspace`, and `phase21-workspace/design` does not exist.
The artwork lives at the repository root, which is exactly what
`build-icons.mjs` resolves against (`const REPO = resolve(ROOT, '..')`, with the
comment explaining why). Nothing needed restoring.

```
$ node tools/icon-pipeline/build-icons.mjs --check
checked 17 generated file(s) against .../apps/desktop/icons
PASS — every committed icon is what this source renders.
exit 0
```

17 of 17 byte-identical. Rendered on Chrome **152.0.7977.84**; the committed set
was produced on 152.0.7977.76, so the pipeline is stable across that patch bump
— a stronger result than a same-version match. All 17 committed hashes were
independently re-verified against `SOURCE.json`: **0 mismatches**, and the
source hash matches.

`DBT-P55-005` closed. No artwork was overwritten and no file was regenerated
into the tree — `--check` renders to a temporary directory.

---

## ITEM 4 — the two P47 decisions

### `DBT-P47-003` — `perProcessorBusyBp`: deliberately out of scope

The producer was already known. P58 measured the **consumers**, which is where
the decision actually lives.

**Nothing reads it.** Zero render sites in `apps/ui/src` — it appears only as a
type in `contracts.ts` and as data in `layout-fixture.ts`. Zero uses in
`crates/performance-bottleneck`. The only readers are the wire encoder
(`services/maintenance-service/src/performance.rs:31`) and `aetherctl offline`'s
JSON dump (`apps/aetherctl/src/offline.rs:308`).

**And no platform populates a real per-core breakdown.** `macos_impl.rs:224` and
`linux_impl.rs:515` both write `vec![total_busy_bp]` — the aggregate, once.
Windows' empty vector is the only one of the three that does not present an
aggregate as if it were a per-core array.

So implementing it on Windows alone would make Windows the only host carrying a
genuine per-core reading, and the same wire field would mean two different
things depending on where the service runs. That is worse than empty.

The cost, stated rather than left implied: the per-instance counters would join
the PDH query `sample_cpu` already opens, so the incremental cost is N extra
`PdhAddEnglishCounter` calls and N extra reads per tick — **not** a second
100 ms sleep, since the two `collect()` calls already happen inside one tick.
The observer effect is therefore small. It is not the reason. The reason is that
there is no caller.

**Decision: `ACCEPTED`, out of scope.** Retire it by wiring all three platforms
*and* a consumer, not by wiring Windows.

### `DBT-P47-004` — GPU identity and VRAM: split, because the halves differ

The two halves have different consumers, so they get different answers.

**Identity** (`adapter_id`, `adapter_name`) — nothing renders it. Grep of
`apps/ui/src` finds it only in `contracts.ts` and the fixture, and no catalog
key in either language names an adapter. Same argument as `DBT-P47-003`: no
caller. **`ACCEPTED`, out of scope.**

**VRAM total** (`dedicated_total_bytes`) — this half **does** have a consumer.
`PerformancePage.svelte:227` renders `perf.vram` as used/total. With the total
at 0, `bytesToGb` returns `—` (`:84`, `if (!bytes) return '—'`), so on Windows
the tile reads `5.0/— GB`. That is honest — it follows the project's
"a meter at rest reads em dash, never 0" rule — but it is a permanent hole in a
shipped screen, which the identity half is not.

**It was not wired this session, and the reason is the brief's own condition.**
The requirement for wiring it is "prove the number traces to it". The Windows PC
is unreachable from this Mac (see ITEMS 5–7), so that proof cannot be produced
here. Committing untested Windows-only WMI code would be the exact failure this
project has been bitten by four times: code that compiles on the wrong host,
never executes, and gets reported as done. **`BLOCKED-MACHINE`**, and owner-gated
at §3 row 6.

Recorded so the next session does not reach for the obvious source:
**`Win32_VideoController.AdapterRAM` is wrong.** It is a `uint32` and saturates
at 4,293,918,720 B, so every adapter above 4 GiB reports about 4 GiB. The
traceable sources are
`HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\<NNNN>\HardwareInformation.qwMemorySize`
(a `QWORD`) or DXGI `IDXGIAdapter::GetDesc().DedicatedVideoMemory`. WMI remains
the right source for the identity half (`Name`, `PNPDeviceID`) if it is ever
wanted.

---

## ITEMS 5–7 — `BLOCKED-MACHINE`

The Windows PC is not reachable from this Mac. Measured, not assumed:

* `~/.ssh/config` **does not exist** — there is no configured route to any host.
* The only entry in `~/.ssh/known_hosts` is `[192.168.68.114]:8022`. That
  address is **absent from the ARP table** (which holds `.113` and `.115`), and
  port 8022 is not Windows OpenSSH.
* No hostname, address, or credential for the PC appears anywhere under
  `docs/` — a grep for address, hostname and remoting patterns returns nothing.
* P55's Windows work ran **natively on that machine**, not remotely:
  `P55-ITEM4C-LIFECYCLE.md:3`, "Machine: the physical Windows PC, elevated."

A port sweep of the local subnet to confirm the negative was attempted and
declined by the sandbox, so the conclusion rests on the four points above rather
than on a scan.

`DBT-P55-008` (installer English-only on an Arabic-UI machine), `DBT-P55-009`
(`/uninstall` shows the Modify chooser with Repair focused) and `DBT-P49-003`
(three elevated `%TEMP%` logs, and the sweep pattern that changed from
`AetherCore_Setup_*` to `AetherCore_*`) are all marked `BLOCKED-MACHINE` in the
ledger with that evidence attached. None was attempted and none is claimed.

---

## The owner register — recorded, not attempted

Unchanged and carried forward as written: `DBT-P55-004` (recovery media not
attached, never boot-tested, Gate 4 cannot start), the Gate 4 device choice from
`docs/phase55/GATE4-CANDIDATES.md` (printer, HID or USB class only — never
storage, chipset, GPU or anything in the boot path), `DBT-P55-003` (C: uses
571.6 GB against 410.5 GB free on `D:`, so a second image does not fit), and the
code-signing certificate.

`DBT-P55-007` is worth restating plainly now that ITEM 2 has made the CI story
concrete: `signatureValidationMode=require` is not enforced on a developer
machine — a cold restore with deliberately corrupted fingerprints still exited
0 — while it **is** enforced on CI. **A developer-machine build is therefore not
evidence that packaging dependencies were signature-checked, which leaves a
green CI as the only trustworthy build path.** That is also why `DBT-P58-001`
cannot be closed here: generating the dependency freeze set on this machine
would inherit exactly that weakness.

Two rows were added to §3 by this phase:

* **row 5** — generate and commit the dependency freeze set on the trusted
  workstation. Unblocks `ci.yml`'s `windows` job, which currently reaches none
  of its other fifteen gates.
* **row 6** — decide `DBT-P47-004`'s VRAM half once the PC is reachable.

---

## The ledger count

Recounted by counting statuses in the table, not by arithmetic on a previous
figure.

| | rows | status begins `OPEN` |
|---|---|---|
| before (`d158bf5`) | 23 | **17** — the brief's seventeen, exactly |
| after | 29 | **15** |

Five left the open count — `DBT-P55-001`, `DBT-P55-002` and `DBT-P55-005` closed
with evidence; `DBT-P47-003` and `DBT-P47-004` decided. Three entered:
`DBT-P58-001` (owner-gated), `DBT-P58-004` (low) and `DBT-P58-005` (the
fail-open readers). `DBT-P58-002` and `DBT-P58-006` were opened and closed
inside this phase and `DBT-P58-003` was recorded straight to `ACCEPTED`, so
those three do not move the count.

Two cautions against reading 17 → 15 as pure progress. `DBT-P47-004`'s VRAM half
left the open count but is not done — it is blocked. And the new ids are not new
defects: three exist only because the workflows became real enough to fail, and
`DBT-P58-005` names a class that was always there and became visible only
because moving a file exposed one instance of it. What genuinely improved is
that three of the fifteen now carry a named unblocking action in §3 rather than
being undiagnosed.

One caveat for anyone re-running the count: `DBT-P56-002`'s row is malformed in
the source — it has four cells rather than five, because the separator between
status and evidence is missing. Pre-existing, present in `d158bf5`, and left
alone: splitting it correctly means deciding where someone else's status ends
and their evidence begins. It does not affect the figures above, since the
merged cell begins `**NARROWED` and so was never counted as open.

---

## Commits

| commit | item |
|---|---|
| `85641e1` | ITEM 1 — `PageId` stops enumerating the drawer |
| `b4023ec` | ITEM 2.B — `cargo fmt --all`, 128 files, nothing else |
| `8888d31` | ITEM 2.A/2.C — the three workflows moved to the root, plus Dependabot |
| `22e4a21` | ITEM 2.C — `cargo fuzz` builds for the first time; `ci.yml` push filter |
