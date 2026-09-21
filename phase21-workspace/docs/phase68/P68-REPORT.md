# P68 — the report

Host: the Mac at `/Users/hasanalaaa/dev/aethercore`. Branch `main` throughout,
from `2932eca` to `218a180`. Every figure names the command or the run that
produced it.

**This report is written late.** P68 ended at `218a180` on 2026-09-19 and this
file was created on 2026-09-20, during P69, because it did not exist on disk and
two phases of measurement lived only in a transcript. Where a figure is quoted
from a P68 commit message it says so; where it was re-measured today against the
tree it says that too, and one re-measurement corrects what P68 recorded
(§4).

The short version. **Step 21 — `cargo clippy --workspace --all-targets --locked
-- -D warnings`, the gate at `verify-enterprise.ps1:24` that had never passed in
this repository's history — went green, and with it the whole windows job.** Run
`35471181801` at `d7b087a`: windows SUCCESS, deny-check SUCCESS, **zero failed
steps**, every step through 27. It took eight layers of findings, eight CI round
trips of roughly 30–50 minutes each, and two regressions of my own making. The
Dependabot pause held since P59 was lifted in the same phase, at `218a180`.

---

## 1. Why it took eight round trips, and why that was not avoidable

`verify-enterprise.ps1:24` runs clippy **without `--keep-going`**. With
`-D warnings`, cargo stops scheduling at the first crate that fails. Each run
therefore reports one layer, and the layer behind it is invisible until the one
in front is gone. P67 had already documented this mechanism; P68 paid for it
eight times.

| layer | commit | run it read | head of that run | crate(s) | findings |
|---|---|---|---|---|---|
| — | `0957d90` | `35424128270` | `2932eca` | maintenance-service | 4 **compile errors**, not lints |
| 1 | `8dcaef7` | `35435167251` | `0957d90` | update-engine, system-repair | 3 |
| 2 | `0630680` | `35436696740` | `8dcaef7` | cleaner | 2 |
| 3 | `38ef9bd` | `35438112652` | `0630680` | startup-manager, idle-scheduler | 5 |
| — | `51259a7` | `35439457952` | `38ef9bd` | *(step 20 regression, mine)* | 2 gate failures |
| 4 | `533806f` | `35440224640` | `51259a7` | performance-optimization | 2 |
| — | `0f75300` | *(this host, pre-empted)* | — | maintenance-service | 4 `deprecated` |
| 5 | `9f0897b` | `35442263470` | `0f75300` | maintenance-service | 9 |
| 6 | `391df3f` | `35444455795` | `9f0897b` | desktop | 2 |
| 7 | `08f30b3` | `35445956964` | `391df3f` | update-broker | 3 |
| 8 | `d7b087a` | `35469547273` | `08f30b3` | aetherctl | 9 |
| ✅ | `218a180` | `35471181801` | `d7b087a` | — | **0 — whole job green** |

Every row but two is one CI round trip of 30–50 minutes; thirty-odd findings,
discovered one layer at a time. The single change that
would collapse this loop is `--keep-going` on the gate's clippy line — P67
proved it turns eleven round trips into one sweep — and it is still not there.
Recorded as open in §7.

## 2. The 30/23 split, and the two wrong numbers before it

P67 predicted lintability from `cargo metadata --filter-platform
x86_64-pc-windows-msvc`: **28 of 53** workspace members have no C build script
anywhere in their resolved Windows graph and can be cross-linted from macOS; the
other 25 cannot. That is a prediction from the dependency graph.

`0957d90` then claimed the cross-target sweep actually compiled **32 of 53**.
That number came from counting `Checking`/`Compiling` lines in the cargo log,
which credits a crate whose *build script* compiled and then died.

`8dcaef7` corrected it by counting `compiler-artifact` records from
`--message-format=json` with `kind == custom-build` excluded: **30 of 53
checked, 23 not** — the 21 never scheduled, plus `aethercore-desktop` and
`aethercore-system-repair`, whose own build scripts failed. That is the 30/23
split, and it is a *measurement of what the sweep did*, not a prediction of what
it could do. The two numbers answer different questions and both are right:
28/25 is graph-derived reach, 30/23 is observed coverage.

The split earned its keep as a predictor, and the record is unanimous. **All ten
crates that produced a finding in P68 — update-engine, system-repair, cleaner,
startup-manager, idle-scheduler, performance-optimization, maintenance-service,
desktop, update-broker, aetherctl — are in the 23 the sweep could not reach.**
Not one finding came from the 30 it did check. That is the sweep working: it
had already cleared everything it could see.

The one partial exception is worth stating because it looks like a
counter-example and is not. Layer 4's two findings
(`crates/performance-optimization/tests/governance.rs` — an unused import and a
dead `const`) were visible to plain **host-native** `cargo clippy` all along,
because that file contains no `cfg` at all. Cross-target unreachable and
host-visible are different properties; a crate can be both, and this one is.
Those two were simply never looked at.

**Re-measured today, and it does not reproduce cleanly.** Recomputing the
graph-derived split from the same `cargo metadata` invocation gives 23 free / 30
blocked if a package is called "needs a C toolchain" whenever anything in its
graph declares `links`. Two of the packages that triggers are pure Rust:
`prettyplease` declares `links = "prettyplease02"` and `defmt` declares
`links = "defmt"`, both as version-uniqueness locks, neither compiling any C.
Excluding those two reproduces P67's **28/25** exactly. The naive `links` rule
and the observed sweep agree on the numerals 30 and 23 by coincidence, in
opposite senses. Anyone re-deriving the split should exclude `links` squatters
and expect 28/25 from metadata.

That re-derivation could not be closed by running the sweep, for the reason in
§6.

## 3. Two regressions, both mine, both the class this phase existed to fix

**`0957d90` — a same-file grep cannot see a `use super::*`.** P67 deleted
`use tracing::{error, warn};` and `use aethercore_product_identity::SERVICE_NAME;`
from `services/maintenance-service/src/main.rs` and audited the deletion by
grepping `main.rs`. The consumer is `windows_service_host.rs`, a
`#[cfg(windows)] mod` that opens with `use super::*` and reads main.rs's root
scope from another file. Run `35424128270` failed step 16 with four **compile
errors**: `cannot find macro error` at `windows_service_host.rs:25`,
`cannot find value SERVICE_NAME` at :20, :32, :38.

**`38ef9bd` → `51259a7` — the compiler is not the only consumer.** Deleting the
`SERVICE_DEMAND_START` import from `crates/startup-manager/src/windows_impl.rs`
was correct about the language and turned step 20 red, because
`scripts/static_validate.py` asserts the literal token at :455 and :468.
Windows clippy was right; two text validators were also right; only one of them
is a compiler. Step 20 had passed on every run before `38ef9bd`.

The method change that came out of it, and held for the rest of the phase: **the
full local script set runs before every push**, not just `ps_marker_scan` and
`source_seal`. The first attempt at `0f75300` was caught locally by
`static_validate.py` for exactly this reason, before it cost a round trip.

## 4. The BSD-sed ANSI-strip defect

`0957d90`'s analysis stripped ANSI escapes from cargo logs with
`sed 's/\x1b\[[0-9;]*m//g'` and then ran anchored greps over the result. The
greps matched nothing, which reads identically to "no errors found". `8dcaef7`
caught it, re-derived every count with a Python strip verified to remove the
escapes, and recorded the cause as "BSD sed does not implement `\x1b`".

**Re-measured on this host today, that cause is wrong.** Against a file
containing real `ESC[36;1m` / `ESC[0m` sequences, macOS `sed` (Darwin 25.6, the
BSD sed with no `--version`):

| form | strips? |
|---|---|
| `sed 's/\x1b\[[0-9;]*m//g'` (BRE) | **yes** |
| `sed -E 's/\x1b\[[0-9;]*m//g'` (ERE) | **yes** |
| `sed -E 's/\e\[[0-9;]*m//g'` | **no** — output byte-identical, exit 0 |
| `sed -E $'s/\033\\[[0-9;]*m//g'` (literal ESC) | yes |

So `\x1b` is implemented here and `\e` is not, and the `\e` form is the one that
fails **silently**: unchanged output, exit 0, and an anchored grep afterwards
returns 0 matches that look like a clean bill of health. The defect P68 recorded
was real in its effect and imprecise in its cause. The remedy P68 adopted —
strip in Python, verify the strip removes the escapes — is correct for both
forms and is what this report's own log reads used.

The general rule, which is the part worth keeping: **a text filter that can fail
open must be verified on a positive control before its output is trusted.** A
strip that silently does nothing and a strip that works produce the same exit
code, and every downstream count is then a count of zero.

## 5. `dispatch.rs` has zero headroom, not two lines

`0f75300` needed to add one `#[allow(deprecated)]` attribute to
`services/maintenance-service/src/router/dispatch.rs`. It could not.

- The file is **exactly 258 lines**. `ROUTER_MODULE_CEILING` in
  `static_validate.py:1482` is **258**. The ratchet fails at 259. P67-REPORT §6
  records this as "258 (<260) — two lines of headroom"; the measured headroom is
  **zero**.
- Buying the line by collapsing the two three-line match arms is also blocked:
  three validators assert the arms' source text verbatim, two of them including
  the `Err("legacy service-side update download is disabled"` substring
  immediately after the pattern — `static_validate.py:1944`,
  `phase15-security-audit.py:74`, `test_gate_module_reader.py:62`.

The file can neither grow nor be reflowed. The attribute went on `mod dispatch;`
in `router.rs` instead, one level up, with the reason written where the next
reader hits it. Worse code in the small, and the only move available.

## 6. What the runner proved that no host could

`cargo clippy` cross-compiled to `x86_64-pc-windows-msvc` was P67's method and
P68's fallback, and it is **unavailable on this host as of 2026-09-20**. Not
because of a C toolchain: because build scripts have to link a *host* binary,
and `cc` fails with exit 69 —

```
warning: failed running `"xcrun" "--sdk" "macosx" "--show-sdk-path"` to find MacOSX.sdk
note: You have not agreed to the Xcode license agreements.
error: linking with `cc` failed: exit status: 69
```

— reproduced today on `aethercore-contracts`, whose build script is `prost-build`
and contains no C at all. The same unaccepted Xcode licence that makes
`/usr/bin/git` unusable stops every `cargo` command that must run a build
script. Accepting it needs a password this session does not have. **Until it is
accepted, CI is the only Rust verifier for this tree**, which makes §1's missing
`--keep-going` more expensive than it was during P68, not less.

## 7. Where P68 left things

Green, measured, at `d7b087a` / run `35471181801`:

- step 16 `Rust unit/integration tests` — broken by `2932eca`, fixed by `0957d90`
- step 20 `Platform-neutral invariants` — broken by `38ef9bd`, fixed by `51259a7`
- step 21 `Enterprise convergence gate` — **the clippy wall, never green before**
- steps 22, 23 — the Zenith gates, skipped every run since step 21 first failed
- step 24 `Supply-chain audit`, step 25 unsigned packaging candidate — same

`DBT-P60-006` closes with it: `crates/system-repair/build.rs` finds
`dismapi.lib` on the runner, which is why step 16 completes at all.

Open, and each named where it will be found again:

1. **`--keep-going` is still not on `verify-enterprise.ps1:24`.** Eight round
   trips is the cost of its absence, measured.
2. **`OWNER_B`** (`crates/performance-optimization/tests/governance.rs`) was
   removed as dead in `533806f`. It is a second 64-char owner key in a file
   whose header claims "single-flight machine mutation" among its invariants,
   used zero times against `OWNER_A`'s five. That is what a never-written
   cross-owner test leaves behind, and removing the constant removed the last
   trace of it.
3. **Host clippy count unreconciled.** P67-REPORT §5 protects 36 findings;
   `cargo clippy --workspace --all-targets --locked` on this host printed **58**
   warning lines by plain count at `533806f`. P67 may have deduplicated
   lib/test targets. Not reconciled, and nothing was touched on the strength of
   either number.
4. **The phase 26/27/28 audits fail on `main`** and are not in CI. Predicted at
   `9f0897b` — "step 21 going green will expose them" — and this is what P69
   item 2 went on to measure.
5. **`_build_p27_archive.py` and `_build_p27_patch.py` rewrite tracked files
   when run.** Discovered by globbing `scripts/*.py` to find the audit set at
   `9f0897b`; they rewrote `PHASE27_FINAL_SHA256.txt` and two
   `PHASE_27_BINARY_SAFE_PATCH` files. Reverted before staging. Audits are named
   explicitly now, not globbed — but the scripts are still in the same directory
   with the same names.
