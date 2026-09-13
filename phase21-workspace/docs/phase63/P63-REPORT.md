# P63 — the report

Host: the Mac at `/Users/hasanalaaa/dev/aethercore`. Date: 2026-09-13. Branch `main`
throughout. Every figure names the command that produced it. Where a claim is
reasoning rather than measurement it says so, and four of them are.

The short version. **`DBT-P61-001` is closed: 108 → 0**, and **step 17 is green
on the runner, three times.** Steps 19–23 have still never executed: step 18 has
now been reached four times and stopped at four different places, each one a gate
that had never run, and the fourth — the router's line budget, thrown as a hard
error by `phase10-architecture-audit.ps1` — is the one this phase deliberately
did not fix and must hand on.

**`DBT-P61-001` is closed: 108 → 0.** The nine gates report
785 checks and zero failures across the six, plus `zenith_adversarial` 35,
`phase16_policy` 42, and `phase12-localization` PASS. **Step 17 is green** —
`static_validate.py` exits 0 with `checks: 347, failed: []` — and so is every
Python gate behind it in steps 18 through 20. Four defects surfaced while working
the 108 and are fixed rather than filed; three more are filed open with the
measurement that justifies each.

Before any of it: **`main` did not pass step 8.** `a0a5b10` added
`docs/phase62/P62-REPORT.md` and regenerated `MANIFEST.sha256` in that order, so
the manifest was minted before the file existed. Not a prediction — run
`34768701075` failed there and skipped the fifteen steps behind it.

---

## The count, before and after

`cd phase21-workspace && python3 scripts/classify_gate_failures.py`

| | before | after |
|---|---|---|
| `FORMAT` | 47 | **0** |
| `ABSENT` | 22 | **0** |
| `MIXED` | 3 | **0** |
| `UNCLASSIFIED` | 36 | **0** |
| **TOTAL** | **108** | **0** |

Gate by gate, `{ok, checks, failed}` from each gate's own stdout:

| gate | checks | failed before | failed after |
|---|---|---|---|
| `static_validate.py` | 347 | 28 | **0** |
| `phase15-security-audit.py` | 106 | 29 | **0** |
| `zenith-recursive-audit.py` | 120 | 26 | **0** |
| `enterprise-adversarial-audit.py` | 88 | 14 | **0** |
| `phase13-reliability-audit.py` | 61 | 6 | **0** |
| `phase14-scheduler-audit.py` | 63 | 5 | **0** |
| `zenith-adversarial-audit.py` | 35 | 0 | 0 |
| `phase16-ga-audit.py` | 42 | 0 | 0 |
| `phase12-localization-audit.py` | — | FAIL (9) | **PASS** |

Every intermediate sweep was diffed against the previous one, not just counted.
**Zero checks newly failed at any step.** That is the number that matters for a
phase whose main instrument is a widened comparison.

---

## ITEM 0 — `DBT-P63-001`, the seal, which was not in the brief

`python3 scripts/source_seal.py` against a clean `git worktree` at `a0a5b10`:

```
Source seal: FAILED - 1459 of 1460 tracked files verified, 1 problems
  unlisted     docs/phase62/P62-REPORT.md
exit 1
```

Step 8 of the windows job is that command, and every step after it trusts those
bytes. Run `34766915417` reached step 17 because it ran at `9fe48ea`, one commit
earlier. The fix is one manifest line, regenerated **in the pristine worktree**
rather than in this phase's dirty tree, so the commit carries two hash lines and
nothing else. `2924f5a`.

**And the runner says the same thing.** Run `34768701075`, at `a0a5b10`, which
nothing in P62 mentions:

```
step 7  success  install-windows-adk
step 8  FAILURE  Delivered source seal
step 9  skipped  Verify approved dependency freeze
…       skipped  (fifteen steps, through the artifact upload)
```

So the brief's "push the windows job past step 17" was measured against a run one
commit behind `main`. From `main` the job stopped at 8.

The general lesson is in the ordering, not the line: a generator that runs before
the file it must describe produces a manifest that is correct about everything
except the commit it is part of.

---

## ITEM 1 — one comparison, defined once

81 of the 108 were the gates spelling a construct that is in the tree.
`hardware_gate:IsolationGate` against a `rustfmt`-clean
`hardware_gate: IsolationGate`; `record.claimed=true`; `if args.len()!=5`.

`gate_reader.contains(text, token)` drops whitespace from both sides. The six
token gates' `has`/`marker` call it instead of spelling `token in text`. It lives
in one place because six copies of a comparison are six things to keep in step —
and because the next three helpers had to join it there:

* `count(text, token)` — the `>= N` and `== N` checks are the same defect.
  `coordinator.count('verify_file_hash_size(&path')` counted **zero** on a
  formatted tree.
* `position(text, token, start)` — the ordering checks are the same defect.
  Squashing deletes characters without reordering them, so `position(a) <
  position(b)` means exactly what `find(a) < find(b)` meant. The index is in
  squashed coordinates and the docstring says so, because it is never a line
  number.
* `ordered(text, *tokens)` — was duplicated, character for character, in two
  gates. Now imported from the same module.

**A widening that cannot fail is worth nothing.** `scripts/test_gate_contains.py`
asserts both halves against the real tree, through the gate's own helper and the
gate's own loaded sources:

```
PASS  literal 'hardware_gate:IsolationGate' in diagnostic-engine  -> False
PASS  contains 'hardware_gate:IsolationGate'  -> True
PASS  contains 'hardware_gate:QuarantineGate' (invented)  -> False
PASS  contains 'cancel_registered_io(&self.peer_reader)' (removed by 482d480)  -> False
PASS  contains 'IsolationGate:hardware_gate' (reordered)  -> False
PASS  contains 'Active, Committed, Revoked }' spelled without the last comma  -> False
PASS  count(streaming, 'else { break; };') >= 5  -> True
PASS  count(streaming, 'else { continue; };') (invented)  -> False
PASS  position of an absent token is -1  -> True
PASS  phase13 gate has(diag, 'hardware_gate:QuarantineGate')  -> False
```

The absent case is not invented: `cancel_registered_io` is the identifier
`482d480` deliberately removed, and it is one of the checks that **stayed
failing** after this commit until it was read and repointed by hand.

`scripts/test_gate_readers.py` still passes: all 14 readers fail closed.

`4a4aae8` took the sweep 108 → 64, deletions only.

---

## ITEM 2 — the two pipe-descriptor rows, and a third that was passing for the wrong reason

P62 left `ipc_pipe_create_instance_regression_test` and
`ipc_production_pipe_owner_and_server_authority_are_service_scoped` unresolved
because the authenticated-users ACE is an access-mask question, not a grep. It
is, and the tree answers it.

`zenith-recursive-audit.py:383` demanded `(A;;0x00120003;;;AU)`.
`crates/ipc/src/windows_impl.rs:268-288` documents that exact encoding **as the
defect**: Windows' SDDL parser silently drops `SYNCHRONIZE` (`0x00100000`) from a
HEX access mask, the DACL materialized as `0x00020003`, and every client — the
shipped desktop app included — opens with `PIPE_CLIENT_ACCESS_MASK = 0x00120003`,
which synchronous I/O requires. The production pipe was **unopenable as
encoded**. `:1211` asserts, in the product's own test, that the broken encoding
must never return. The gate was demanding the bug back.

The tree grants `(A;;FR;;;AU)(A;;0x00000002;;;AU)`:

```
FR   = FILE_GENERIC_READ = READ_DATA|READ_EA|READ_ATTRIBUTES|READ_CONTROL|SYNCHRONIZE = 0x120089
0x2  = FILE_WRITE_DATA, which FR does not include
FR|0x2                                                                              = 0x12008B
```

`0x12008B` covers the client's `0x120003` and withholds
`FILE_CREATE_PIPE_INSTANCE` (`0x4`), `FILE_APPEND_DATA` and every generic server
right — which is the whole of what the checks assert. Policy identical, encoding
expressible.

Both are repointed, and **why the hex form is forbidden is written where a later
phase will meet it**: a block comment above ZR-016, and a dated amendment to
`docs/adr/0021` decisions 13, 15 and 19, whose accepted text still says
`0x00120003` and "exactly one AU allow". The ADR's decisions are left as written;
an accepted decision is a record, not a mutable field.

Two things found while doing it, both measured:

**`ipc_authenticated_users_lack_pipe_create_instance` was passing for the wrong
reason.** Its `(A;;0x00120003;;;AU)` token had exactly one match in the section
it searched, and it was not the production descriptor: it was
`DEV_PIPE_SECURITY_DESCRIPTOR` at `windows_impl.rs:143`, the debug-only one. The
check had never read the builder it claims to check. It is now scoped to
`fn production_pipe_security_descriptor` and asserts the hex form is **absent**
from it.

**`DBT-P63-002`** — `verify-ipc-pipe-security.ps1:245` compares the AU DACL grant
against `$RequiredClientMask`, which is the mask a **client asks for** at open
time, not what the descriptor grants; and `:253` requires exactly one AU allow
ACE, where named rights need two. The probe would throw `Authenticated Users pipe
allow mask drift` on a correct pipe. Dormant: `ci.yml` never calls it, and its
two callers need an installed running service. The probe now compares the grant
to the grant, keeps exactness against `0x0012008B`, and adds a coverage
assertion — a grant that stops covering the client mask is precisely the
`SYNCHRONIZE` regression returning.

**Not verified by execution.** `0x0012008B` is read from the tree's own
lab-proven A/B DACL dump and from the assertion at `windows_impl.rs:1197-1211`.
This Mac cannot materialize a Windows DACL. The next installed-service run
measures it. `3915300`, 64 → 62.

---

## ITEM 3 — the residue, and what it actually was

### The 19 UNCLASSIFIED, each to a verdict

The classifier reported them UNCLASSIFIED because it wraps `has`/`marker` and
these checks bypassed both — they spell the comparison inline. **Ten of the 19
were the identical whitespace defect**, invisible to the tool that was built to
find it:

| gate | checks | what they spelled |
|---|---|---|
| `phase13` | `memory_unavailable_is_optional` | `'pub memory:Option<MemoryTelemetry>' in diag` |
| `phase14` | `presentation_unknown_fail_closed`, `servicing_unknown_fail_closed`, `monotonic_passive_inventory_epochs` | bare `in` |
| `phase15` | `service_revalidates_…`, `intent_revalidates_…`, `broker_hash_…`, `broker_runs_exact_ticket_path…`, `claim_reservation_linearizes…` | `.count(...) >= 2`, `.find(...)` ordering |

`static_validate.py` had **ten more of the same**, and they are why its
UNCLASSIFIED column read 17.

The rest, read one at a time:

| check | verdict | what it actually was |
|---|---|---|
| `phase10_read_watcher_lease_release` | CHECK | `else { break };` is not Rust for a let-else. Every one of the six is `let Ok(v) = … else { break; };` — the body statement carries its own semicolon. The token was never in any tree |
| `support_owner_scoped_chunks` | CHECK | same thing: `return Err(...)` is a statement, and the token omitted its semicolon |
| `path_dependencies` | CHECK (scope) | all 12 errors were `PHASE_NN_BINARY_SAFE_PATCH/new-files/**` — patch payloads, deliberately unresolvable where they sit |
| `phase7_all_eight_surfaces` | CHECK | asserted `shortcut count == 9`. Navigation has 16 surfaces and 12 `Ctrl+Shift+` shortcuts; all nine required ids are present. Now asserted **per required id**, so deleting one surface's shortcut still fails |
| `source_has_no_placeholders` | CHECK (scope) | 8 of 13 hits were upstream `.d.ts` under `node_modules`; the other 5 were **other placeholder detectors naming the tokens they detect**. Two were already exempt; six more were not |
| `rust_impls_reject_duplicate_method_names` | CHECK | `#[cfg(unix)] fn handshake` and `#[cfg(windows)] fn handshake` are not duplicates — they never coexist, and the compiler would reject them if they did. The cfg predicate is now part of the key |
| `production_rust_has_no_panic_shortcuts` | **half scope, half TREE** | see `DBT-P63-005` below |
| `common_windows_release_pairs_are_centralized` | **TREE** | see `DBT-P63-006` below |
| the 8-check IPC cluster | CHECK | see below |

### The `482d480` cluster, repointed rather than deleted

Eight checks named one design: `SyncIoCancellation`, `cancel_registered_io`,
`bind_reader_to_current_thread`, `OpenThread(THREAD_TERMINATE`. Before
`482d480`, both ends did blocking I/O from two threads on one **synchronous**
kernel file object, so cancellation had to target a **thread**. Its commit
message: *"Windows request/response has never worked … every ServiceJob verb
hung."* The pipes are overlapped now and cancellation targets the **file
object** — `CancelIoEx(handle, None)` aborts every in-flight operation on the
handle from any thread, which is strictly broader and needs no registry and no
registration handshake.

Each check asserts the **same property** against the mechanism that now provides
it, and three of them additionally assert the old mechanism **cannot come back**:
`OpenThread(THREAD_TERMINATE`, `CancelSynchronousIo(` and `SyncIoCancellation`
are asserted absent. Satisfying the tokens as written would have reintroduced the
hang.

One is worth stating separately. `ipc_server_reader_is_bound_to_session_worker`
asserted a registration handshake that existed **only** so a thread-targeted
cancel knew which thread to name. Asserting it now would be asserting dead
scaffolding, so the check asserts the guarantee it existed for: the session read
is fenced by writer liveness and cancelled through the shared handle, and
`bind_reader_to_current_thread` appears nowhere.

The check **names** are unchanged, including
`ipc_sync_io_cancellation_uses_documented_thread_handle`, which no longer
describes its mechanism. That is a deliberate trade: the names are how these rows
stay traceable to `DBT-P61-001` and to P62's classification. Each carries an
`asserts=` detail in its own report saying what it now measures.

### The six "tree" rows, of which two were not

| row | P62 said | measured |
|---|---|---|
| `deny.toml` | tree | **tree** — fixed |
| BOM'd evidence JSON | tree | **tree** — fixed, and it is **nine** files, not four |
| `phase11_all_buttons_tactile` | tree | **tree** — fixed |
| `phase10_service_decomposed` | "a decision, not a measurement" | correct — ratcheted, `DBT-P63-004` |
| `phase7_mica_desktop_shell` | tree | **CHECK.** `DBT-P63-003` |
| `phase11_material_hierarchy` | tree | **CHECK.** Deleted on purpose in `ebfe83b` |

**`deny.toml`.** `bans.wildcards = "warn"` → `"deny"`, plus a `[sources]` block:
`unknown-registry` and `unknown-git` deny, `required-git-spec = "rev"`, crates.io
as the only allowed registry. Measured before flipping it: zero wildcard
requirements in any workspace manifest and zero git sources in `Cargo.lock`.
`allow-wildcard-paths = true` because without it `cargo deny check bans` reports
every intra-workspace `{ path = "../x" }` — roughly forty manifests, the whole
internal graph — and those are not a supply-chain risk: every member is
`publish = false`. `cargo deny check advisories bans licenses sources` →
**all four ok**.

**The BOMs.** Nine, not four. Each was proven to parse to an identical document
before and after the three bytes came off, and exactly three bytes came off each.
Nothing but `MANIFEST.sha256` hashes them.

**The buttons.** 10 of 80: eight in `FleetPage.svelte`, one in `AppShell.svelte`,
one in `EvidenceChip.svelte`. `pnpm --dir apps/ui check` → 229 files, 0 errors, 0
warnings; `pnpm --dir apps/ui build` green.

**`phase7_mica_desktop_shell` is a check failure, and this is the measurement
that corrects P62.** `tauri-utils 2.9.3` — the version in `Cargo.lock` — defines
no `noRedirectionBitmap` field on `WindowConfig`, and that struct carries
`#[serde(deny_unknown_fields)]` at `config.rs:1916`. Adding the key to
`tauri.conf.json` would fail config deserialization rather than set anything. The
flag exists one layer down as
`tao::platform::windows::WindowBuilderExtWindows::with_no_redirection_bitmap`
(`tao-0.35.3/src/platform/windows.rs:294`) and Tauri does not expose it.
`PHASE_7_DELIVERABLES.md:30` claims it is enabled; it is not, and cannot be from
configuration. `DBT-P63-003`, open.

**`phase11_material_hierarchy` is a check failure too.**
`--ac-material-elevated` was deleted deliberately in `ebfe83b` (P57): *"a neutral
fill half a step above the card: 60 uses in feature-layout.css … It bought no
distinction a reader could name and cost every screen a surface level, which is
exactly what `DIRECTION.md` principle 3 caps at three."*
`docs/phase56/DIRECTION.md:56` is that cap in writing — "Three neutral levels,
and no more". The token, its utility class and the `elevated` MaterialSurface
level went together, and zero `var(--ac-material-elevated)` references remain.
Restoring the token to satisfy a check would re-open the level the design closed.

**The router, which is a decision and is written out as one.** `main.rs` is 397
lines against a budget of 220; `router.rs` is 1,868 against 240.
`handle_request` is lines 86–1753 of that file as one match. Raising the budget
deletes the alarm. Decomposing is a real refactor with its own test plan, and it
is not free: `phase15_security` reads `router.rs` **as a file**
(`require_update_broker(peer)?`, the `legacy_service_download_rpc_disabled` arm
at `:774`/`:785`) and so do `zenith_recursive`'s two leased-route checks, so
moving code out of it silently breaks four gates that would have to move in the
same commit. So the budget becomes a **ratchet** at the exact measured values and
not one line above them: the files may shrink, and any growth fails.

Negative control, run and then reverted: two lines appended to `router.rs`
produced

```
over_ceiling: {"router.rs": {"lines": 1870, "ceiling": 1868}}   exit 1
```

The 220/240 target is recorded in the check and in `DBT-P63-004`. A ceiling that
can only fall is not a budget met; it is a budget that has stopped being lost.

---

## ITEM 3b — three tree defects found while working the gates

### `DBT-P63-005` — 15 panic shortcuts in shipped Rust

`production_rust_has_no_panic_shortcuts` reported 30 hits and both halves were
real findings of different kinds.

**15 were scope.** `product_rust_sources()` excluded `tests` and `build.rs` but
not `benches`, and `criterion` fixture code is not shipped Rust. `benches` now
joins the exclusion.

**15 were the tree's**, and they are fixed:

* `SessionConsentRegistry` (three sites) panicked on a **poisoned lock**, where
  every other mutex in that service recovers with
  `unwrap_or_else(|p| p.into_inner())`. A panic anywhere else in the process
  would have taken *consent* down with it.
* `apps/desktop`'s `fleet_trust_store()` panicked on an unopenable trust store
  while `fleet_trust_store_checked()` sat beside it returning a `Result`. The
  panicking twin is **deleted** and its three callers migrated — not wrapped.
* `llama.rs`'s backend init panicked inside a `OnceLock`. All three callers
  already returned `Result<_, String>`; it is now `OnceLock<Option<_>>` and
  reports.
* `aetherctl`'s `path.parent().unwrap()`, `to_vec_pretty(...).unwrap()` (twice)
  and `expect("host checked above")` are typed `CliError`s.
* Two `let`-else rewrites where the guard above already proved the invariant
  (`performance-telemetry`, `config_lint`) and one in the topological sort.
* The two digest sites are the ones worth reading. **A fingerprint has no honest
  fallback** — a fabricated one is indistinguishable from a real one — so
  `GraphError::Digest` and a fallible `canonical_machine_state_fingerprint`
  propagate, and `build_intelligence` turns a failure into an **invalid**
  snapshot, the same fail-closed shape `analyze` already used for an unbuildable
  graph. Reasoning, not measurement: every field of `RepairFact` and
  `RecoveryReadiness` is a `String`, an `i64` or a unit enum, so `to_vec` cannot
  actually fail today. "Cannot fail today" is not a reason to panic in a service.

### `DBT-P63-006` — `CloseHandle` had two implementations

One hit: `crates/ipc/src/windows_impl.rs`. It was not a manual release pair — it
was `impl Drop for PipeHandle`, correct in itself — but a second copy of "close
exactly once", introduced by `482d480` because `OwnedHandle` wraps a `HANDLE`,
which `windows` declares `!Send`, and overlapped I/O has to hand the handle to a
pump thread. `aethercore_windows_foundation::SendableHandle` now holds the opaque
value with the `unsafe impl Send` and its reasoning stated once; `crates/ipc`
already depended on that crate on Windows.

**Type-checked for the platform it runs on.** P62 used that target from this Mac
too, but against a *scratch* crate (its `Send` cross-check); this is the first
time a workspace crate has been compiled for it here:

```
cargo check --locked -p aethercore-ipc -p aethercore-windows-foundation \
  --all-targets --target x86_64-pc-windows-msvc
    Finished `dev` profile
```

with the same three pre-existing warnings as before the change. A whole-workspace
check for that target is **not** possible here — `libsqlite3-sys`'s build script
needs MSVC C headers — so this covers the two crates that changed, not the
workspace.

### `DBT-P63-007` — `phase12-localization`, which is a step-18 blocker

P62 recorded it as "its own row to open" and out of the 108. It is in fact inside
step 18: `verify-phase15.ps1:44` runs it. Nine issues, three kinds.

**Real**: seven raw English `placeholder=` attributes in `FleetPage.svelte`. They
are seven catalog keys in both locales now, so they *can* diverge later. Only one
of the seven is prose — `64-char SHA-256 hex` — and only it is translated; the
other six are example hostnames, an example identity-file path and comma lists,
identical in every locale by construction and named in the gate's `same_allow`
for that reason.

**Gate**: `Ctrl /` in a `<kbd>` at `NavigationRail.svelte:56` is a chord, joining
`Ctrl K` and `Esc` in `allowed_exact`. And `fleet.ssh` = `SSH`,
`repair.domain.dns` = `DNS` were flagged as "Arabic identical to English", which
is exactly what `same_allow` exists for.

`Catalog entries: en=1678 ar=1678 parity=1678` · **PASS**.

---

## ITEM 4 — the job, step by step, and what each step asserts

The windows job is 23 steps. What each one **asserts** — not what it is named —
and what this Mac could measure of it. PowerShell is not installed here, so the
six `.ps1` steps (9, 18, 19, 20, 21, 22) could only be taken apart and their Python and Cargo components
run directly; those rows say so.

| # | step | what it ASSERTS | measured here |
|---|---|---|---|
| 1 | Set up job | runner image, and that the job is schedulable at all | — |
| 2 | `actions/checkout@v7.0.1` | the commit is fetched with `persist-credentials: false`, so no step can push with the job's token | — |
| 3 | Install pinned Rust toolchain | **1.97.1 exactly**, with `rustfmt` and `clippy`, and `rustup override set` so no later step can silently use another | local toolchain is 1.97.1 (`rustc --version`) |
| 4 | `actions/setup-node@v7.0.0` | Node **22.16.0**, cache off so a stale cache cannot substitute for the lockfile | local Node is 26.7.0 — **different**, and the UI gates below therefore prove less here than they will there |
| 5 | Install pinned pnpm | **pnpm@11.22.0** exactly, globally | local pnpm is 11.22.0 |
| 6 | `actions/setup-dotnet@v6.0.0` | .NET 8.0.x, for the signing/packaging tools | — |
| 7 | `install-windows-adk` | `dismapi.lib` exists, so `crates/system-repair/build.rs` can link. `DBT-P60-006` — a shared composite action, not a copy | — (Windows only) |
| 8 | **Delivered source seal** | every git-tracked file under the sealed root is listed in `MANIFEST.sha256` and hashes equal. It is also the only place `.gitattributes`'s eol rules are checked on Windows, because the seal hashes the **working tree** | `python3 scripts/source_seal.py` → **OK, 1461 of 1461**, exit 0. Failed on `main` before this phase — `DBT-P63-001` |
| 9 | Verify approved dependency freeze | both lockfiles, ~90 manifests **including `deny.toml`**, the two baseline files, the freeze metadata, and the Rust/pnpm pins all match the approved baseline; and `cargo metadata --locked` resolves | not runnable (no PowerShell). Its Python twin, `python3 scripts/check-dependency-freeze.py` → **APPROVED**. `deny.toml` moved in this phase and its baseline moved with it — `DBT-P63-008` |
| 10 | Frozen dependency restore | `pnpm install --frozen-lockfile` — the manifest and the lock agree | `pnpm --dir apps/ui install` was already satisfied locally |
| 11 | Rust formatting | `cargo fmt --all -- --check` — zero diffs | **exit 0, empty output** |
| 12 | `fetch-embedded-model` | the real shipped `.gguf` is present, because `embedded_generation.rs` asserts its absence loudly rather than skipping | the artifact is present locally |
| 13 | **Rust unit/integration tests** | `cargo test --locked --workspace` | **135 binaries, 649 passed, 0 failed, 1 ignored, exit 0.** The runner measured the same 135 binaries at 660 passed / 11 ignored (P62); the difference is platform-gated tests, so this is a strong signal and not the same measurement |
| 14 | UI accessibility/type gate | `svelte-check --threshold warning --fail-on-warnings` — zero errors AND zero warnings | **229 files, 0 errors, 0 warnings** |
| 15 | UI production build | the app builds for production | **built in ~0.5 s**, one pre-existing chunk-size warning |
| 16 | Locked Rust workspace check | `cargo check --workspace --locked` | exit 0 |
| 17 | **Platform-neutral invariants** | `static_validate.py` — 347 structural, schema, principal/consent, kernel, design, localization and release-source invariants | **`{"ok": true, "checks": 347, "failed": []}`, exit 0.** This is the wall that came down |
| 18 | Enterprise convergence gate | the deepest step by far: `verify-phase16` → `verify-phase15` → … → `phase2`, then `enterprise-adversarial-audit`, `cargo fmt --check`, **`cargo clippy --workspace --all-targets --locked -- -D warnings`**, `cargo test --workspace --locked`, **17 named regression tests** run individually so a rename cannot silently skip one, and `pnpm check` + `pnpm build` again | its Python components all pass here: `enterprise-adversarial-audit.py` **88/88**, `phase16-ga-audit.py` **42/42**, `phase15-security-audit.py` **106/106**, `phase14` **63/63**, `phase13` **61/61**, `phase12-localization` **PASS**. **`cargo clippy --workspace --all-targets --locked -- -D warnings` → exit 101, 30 errors across 8 crates — `DBT-P63-010`, and it is the next wall.** None of the 30 is P63's: every one is in a file this phase did not touch. `cargo test --locked --workspace` → **135 / 649 / 0 failed** |
| 19 | Zenith adversarial | `zenith-adversarial-audit.py` — source interaction and accessibility | **35/35**, exit 0 |
| 20 | Zenith recursive | `zenith-recursive-audit.py` — architecture and resource safety, including every IPC row this phase repointed | **120/120**, exit 0 |
| 21 | Supply-chain audit | `cargo deny --locked check advisories licenses bans sources`, `pnpm audit --audit-level high`, `pnpm licenses list` | the cargo half runs here: `cargo deny check advisories bans licenses sources` → **all four ok** |
| 22 | Build unsigned candidate | `build-release.ps1 -SkipOnlineSupplyChain` produces the packaging candidate | — (Windows only) |
| 23 | `upload-artifact` | `if-no-files-found: error` — an empty artifact fails the job rather than uploading nothing | — |

**What this list makes obvious.** Steps 18 through 23 have never executed in this
repository. Six steps, and the heaviest of them is step 18, whose
`cargo clippy -- -D warnings` and 17 named regression tests have no local
equivalent that has ever been run here either. Getting past step 17 is necessary
and it is not the same as the job being green.

### Run `34773880960` — the first time steps 8 through 18 have ever executed from `main`

| step | result |
|---|---|
| 8 Delivered source seal | **success** — `DBT-P63-001` confirmed on the platform, not inferred |
| 9 Verify approved dependency freeze | **success** — the real PowerShell gate accepts the `deny.toml` baseline move, `DBT-P63-008` |
| 10–16 | success |
| **17 Platform-neutral invariants** | **success — the wall is down on Windows** |
| 18 Enterprise convergence gate | **failure**, and not where this phase predicted |
| 19–23 | skipped |

**P63 predicted step 18 would stop on clippy. It did not, and the correction is
the more useful finding.** Step 18 runs `verify-phase16.ps1` first, which chains
down to `verify.ps1:29`'s `cargo test`; clippy is line 23 of
`verify-enterprise.ps1`, four sub-gates later, and never ran. `DBT-P63-010`'s 30
findings remain **unmeasured on Windows**, and the row now says so.

What it stopped on is `DBT-P63-011`, and the evidence is decisive because one run
executed the same test twice from one commit on one machine:

```
step 13, 18:36:00   failure_after_barrier_requires_recovery_review ... ok
step 18, 18:48:37   failure_after_barrier_requires_recovery_review ... FAILED
                    assertion failed: status.recovery_required
```

That is a race. `status()` composes `plan_state` from the operation engine and
every record field from the database, and `fail_repair` transitions the plan
**before** it upserts the record carrying `recovery_required`. **It is
`DBT-P62-003` in a second crate** — P62 fixed the identical race in
`aethercore-cleaner` and did not check the siblings, which its own report names
as the mistake it made. All three `plan_state` loops in `system-repair`'s
coordinator tests had it; all three now wait for `plan_state` **and** `stage`.
Held by 40 consecutive runs at `--test-threads=4`, 0 failures.

**The audit P62 skipped, done here.** `crates/driver-install` has three loops of
the same shape and is *not* exposed, because it upserts the record and **then**
transitions — on every terminal path. The ordering is the discriminator:

| crate | terminal ordering | exposed |
|---|---|---|
| `cleaner` | transition → upsert | yes (test fixed P62) |
| `system-repair` | transition → upsert | yes (test fixed here) |
| `driver-install` | **upsert → transition** | no |

That difference is bigger than a test, and it is `DBT-P63-012`: in the
transition-first crates a caller of `status()` can observe a terminal plan with a
stale record, and `recovery_required` — the field that tells a user their machine
needs review — reads **false** in that window. It fails open; `driver-install`'s
ordering fails closed. The cure is already written in this repository. It is
filed rather than applied because it is a behavioural change to Windows
mutation-failure paths this Mac can type-check but not execute, and P63's
test-side fix is verified while a product-side one would not be.

### Run `34775921313`

Pushed at `2a3e642` with `DBT-P63-011` fixed. Step 17 green again; **step 18 got
further and stopped somewhere new** — past `cargo test`, through Phase 8, into
Phase 9:

```
Running Phase 9 security/source audit...
The string is missing the terminator: '.        (verify-phase10.ps1:24)
```

`scripts/phase9-security-audit.ps1:77` used `\"` inside a double-quoted
PowerShell string, where the escape character is a **backtick**. The `\"` ended
the string early and the remainder re-parsed into an unterminated single-quoted
string running to EOF. **A parse error takes the whole FILE**, so all 24 of that
script's assertions — kernel-derived named-pipe PID and session id, the retired
challenge/grant authorization surface, the Phase 1 demo transition surface,
`PROTOCOL_VERSION = 7` — have been asserting **nothing** for as long as the line
has existed. It reported no failure because it never ran a line. `DBT-P63-013`.

That is the same family as `DBT-P58-005` and `DBT-P59-002`: a gate that cannot
fail. It is the third one this project has found, and the first that was invisible
because the *file* never executed rather than because a *read* silently returned
`""`.

### Run `34778679667`

Pushed at `ac15071`. Step 17 green a third time, and Phase 9 **ran** — the fix
worked, and the first assertion it ever evaluated failed:

```
Phase 9 invariant missing: principal token is read from the exact connected pipe
client (crates/security/src/lib.rs)
```

A rename, and a good one: `security/src/lib.rs:128` takes
`ThreadImpersonation::named_pipe_client(pipe)`, whose RAII owner is the only
caller of `ImpersonateNamedPipeClient` in the workspace — which is what makes a
missed `RevertToSelf` impossible rather than merely discouraged. The assertion
now covers both ends.

**At that point the chain was swept locally rather than one 50-minute CI
round-trip at a time**, with a scanner that replays `Require-Marker`'s own regex
semantics:

| script | assertions | failing |
|---|---|---|
| `phase9-security-audit.ps1` | 42 | 0 (after the above) |
| `security-hardening-audit.ps1` | 23 | 0 |
| `phase10-architecture-audit.ps1` | 84 | **5** — all fixed |

Four of the five are a decomposition the gate never saw: `App.svelte` is a
five-line wrapper around `AppShell.svelte` and the session wiring moved to
`platform/kernel-session.ts`. The fifth is a **third defect class**, after
whitespace and rename: `Require-Marker`'s pattern is a **regex**, so
`'self.writer.shutdown();'` parses as `shutdown` + an empty group + `;` and can
never match the literal it was written to find.

**Stated as a limit, not a result**: the scanner cannot evaluate `@('a','b')`
array-form markers — `phase11-design-audit.ps1`, 8 of its 9 — or `$domain`-loop
paths. Those are **unmeasured, not passing**. `DBT-P63-014`.

### The correction this phase owes itself

`phase10-architecture-audit.ps1:136-139` throws on `main.rs >= 220` and
`router.rs >= 220`. They are 397 and 1,868.

Earlier in this phase I ratcheted **exactly that budget** in `static_validate.py`
and argued that decomposing `handle_request` was out of scope for a
gate-classification phase. That argument was sound about `static_validate` and
wrong about the job: the same budget is a **hard throw** inside step 18. So

> **the windows job cannot be green on `main` until `DBT-P63-004` is actually
> done.**

A ratchet in one gate does not soften a throw in another. `DBT-P63-004`'s row is
corrected to say so.

---

## ITEM 5 — Dependabot

**Not resumed, and the condition is not close to met.**

`.github/dependabot.yml` states it in one sentence, deliberately: *"RESTORE THE
THREE LIMITS TO 5 WHEN `ci.yml`'s WINDOWS JOB IS GREEN ON `main`. Not when a
particular step passes — a pull request that cannot get a verdict is worse than
no pull request."*

Three runs this phase, and the job's own record:

| run | at | reached | outcome |
|---|---|---|---|
| `34768701075` | `a0a5b10` | **step 8 of 23** | the seal — `DBT-P63-001` |
| `34773880960` | `e74838d` | **step 18 of 23** | a race — `DBT-P63-011` |
| `34775921313` | `2a3e642` | **step 18**, further in | a parse error — `DBT-P63-013` |
| `34778679667` | `ac15071` | **step 18**, further still | a rename — `DBT-P63-014` |

Step 17 is green three times running and steps 19–23 have still never executed.
The condition asks for green, and green is not what the runner reports.

**And it will not be reached by chipping at it**, which is the useful thing this
phase can say to the next one: `phase10-architecture-audit.ps1:136-139` throws on
the router's line count, so `DBT-P63-004` stands between here and a green job.
Every other blocker found this phase was a gate describing a tree that had moved;
that one is the tree.

Resuming Dependabot now would put four PRs into a job that cannot give any of
them a verdict — the exact failure P59 recorded and P60 refused to repeat.

---

## The ledger

Counted by the rule in `docs/LEDGER.md` § 1, scoped to § 1, with the same script
on both trees:

| | rows | open | malformed |
|---|---|---|---|
| P62 end (`a0a5b10`) | 43 | 17 | 1 |
| P63 end | **57** | **22** | 1 |

**Closed:** `DBT-P61-001` (the 108, all of it), `DBT-P63-001`, `DBT-P63-002`,
`DBT-P63-005`, `DBT-P63-006`, `DBT-P63-007`, `DBT-P63-008`, `DBT-P63-011`,
`DBT-P63-013`.
**Entered:** `DBT-P63-001` through `DBT-P63-014`.
**Open, with the measurement that justifies each:** `DBT-P63-003`
(`noRedirectionBitmap` is not expressible in `tauri-utils 2.9.3`, and
`PHASE_7_DELIVERABLES.md` claims it is enabled), `DBT-P63-004` (the router at
7.8× its budget, ratcheted), `DBT-P63-009` (~244 token comparisons that still compare exactly, in checks that
pass today), `DBT-P63-010` (clippy, still unmeasured on Windows), `DBT-P63-012`
(two crates publish a terminal plan state before the record that explains it) and
`DBT-P63-014` (the never-run PowerShell chain).

Open goes 17 → 22. `DBT-P61-001` closed; `DBT-P63-003`, `004`, `009`, `010`,
`012` and `014` opened.
`DBT-P56-002` stays four-celled, named rather than dropped, as in P58–P62.

---

## What measurement contradicted in the brief

1. **"the two pipe-descriptor rows are RESOLVED and stay out of that edit."**
   True as far as it went, and there were **three**, not two:
   `ipc_authenticated_users_lack_pipe_create_instance` was green against the
   debug-only descriptor and had never read the production builder. A row that
   passes for the wrong reason is not visible in a failure list, which is why it
   had to be found by reading the token rather than the report.

2. **"work the 21 UNCLASSIFIED to a verdict each, then the 6 TREE rows."** The
   two sets are not disjoint in the way the brief assumes. **Ten of the 19
   remaining UNCLASSIFIED were the same whitespace defect ITEM 1 had already
   fixed** — invisible to the classifier only because those checks spell the
   comparison inline instead of calling `has`. And **two of the six TREE rows
   were check failures**: `noRedirectionBitmap` cannot be set in the pinned
   framework at all, and `--ac-material-elevated` was deleted on purpose with a
   written reason.

3. **"push the windows job past step 17."** Step 17 could not have been reached
   from `main` at all. The brief was written against run `34766915417`, which ran
   at `9fe48ea`, one commit behind. The run that actually exists for `main` is
   `34768701075`, and it stopped at **step 8** on the seal, skipping fifteen
   steps — `DBT-P63-001`. Nothing in P62 mentions that run.

4. **P62's "a UTF-8 BOM in `docs/phase36/evidence/*.json` — 4 files."** Nine.

5. **P63's own prediction, which the runner corrected.** This report said step 18
   would stop on `cargo clippy -- -D warnings`. It stopped four sub-gates earlier
   on a racy test, and clippy never ran. The prediction was made from a macOS
   clippy run and the ledger row now carries that correction rather than the
   prediction. Worth stating because it is the same error class this phase spent
   its day removing: a claim about a platform, sourced from the wrong host.

6. **"fix the CHECK half of DBT-P61-001 — 81 of the 108 are one defect."** The
   81 is right and it is not the whole of the one defect. **`DBT-P63-009`**: ~244
   token comparisons in the same six gates still compare exactly — 192
   `'literal' in var`, 14 `.count('literal')`, 38 `.find('literal')` — and every
   one sits in a check that is green today. They were not swept, for a reason
   this phase can evidence: rewriting a passing check is how a green gate goes
   red by accident, and the only thing that made this phase safe was that every
   sweep diffed to deletions only.

---

## Commits

| commit | item |
|---|---|
| `2924f5a` | ITEM 0 — the seal `main` failed at step 8. `DBT-P63-001` |
| `4a4aae8` | ITEM 1 — one comparison for six gates, and the test that proves it can still fail. 108 → 64 |
| `3915300` | ITEM 2 — the AU ACE is named rights, and the hex form is forbidden. `DBT-P63-002`. 64 → 62 |
| `16dae14` | ITEM 3 — `static_validate.py` down to its six tree rows. 62 → 45 |
| `a7bf3e2` | ITEM 3 — step 17 green. `DBT-P63-003`, `DBT-P63-004`. 45 → 39 |
| `e74838d` | ITEM 3 — the last 39. `DBT-P63-005` … `DBT-P63-008`, and `DBT-P61-001` closed. 39 → 0 |
| `2a3e642` | the race that actually stops step 18. `DBT-P63-011`, `DBT-P63-012` |
| `ac15071` | the parse error that silenced the whole Phase 9 audit. `DBT-P63-013` |
| `da59e6d` | the chain behind step 18. `DBT-P63-014`, and the correction to `DBT-P63-004` |
| this one | the report |


---

## The single next action

**Decompose `services/maintenance-service/router.rs`.** `DBT-P63-004`.

It is the only blocker left that is the *tree* rather than a gate describing a
tree that had moved, and `phase10-architecture-audit.ps1:136-139` throws on it as
a hard error inside step 18. Everything else this phase met was a stale
assertion; this one is 1,868 lines and one `match`.

It is not free, and the blast radius is known: `phase15_security` reads
`router.rs` **as a file** (`require_update_broker(peer)?`, the
`legacy_service_download_rpc_disabled` arm at `:774`/`:785`) and so do
`zenith_recursive`'s two leased-route checks. Those four gates must move in the
same commit, and `static_validate.py`'s ratchet ceiling comes down with them.

Two things to carry in, both earned here rather than assumed:

* `scripts/test_gate_contains.py` is the shape a gate change should be defended
  in — assert the widening works AND that it can still fail, against the real
  tree.
* Sweep the PowerShell chain **locally** before pushing. Each CI round-trip is
  ~50 minutes and four of them this phase each surfaced exactly one defect. The
  scanner that replays `Require-Marker`'s regex semantics found five more in
  seconds; it still cannot read `@('a','b')` array markers, which is where
  `phase11-design-audit.ps1` is unmeasured.