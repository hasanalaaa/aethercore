# P67 — the report

Host: the Mac at `/Users/hasanalaaa/dev/aethercore`. Date: 2026-09-19. Branch
`main` throughout, from `3fd1a3d`. Every figure below names the command that
produced it. Where something is reasoning rather than measurement it says so,
and there are exactly three such places.

The short version. **`DBT-P63-010` is retired as far as this host can reach it,
and the reach is much further than four phases assumed.** The wall at
`verify-enterprise.ps1:24` was three findings deep on the runner and seventy-odd
deep behind them. `cargo clippy` now exits 0 for **28 of the 53 workspace
crates, cross-compiled to `x86_64-pc-windows-msvc` with `--all-targets`**, which
is every crate whose dependency tree contains no C build script. The other 25
need a Windows C toolchain to lint at all; they are clean on the host, which
covers all of their portable code and none of their `cfg(windows)` code. **The
row stays OPEN for that remainder, and the remainder is now measured: 17 files
carrying 13,946 lines.** `cargo test --workspace --locked`: **649 passed, 0
failed, 1 ignored, 135 suites, exit 0.**

---

## 1. The method, and what it cost to find its boundary

P66 left the command. It works:

```
cargo clippy -p aethercore-windows-foundation -p aethercore-fleet \
  --all-targets --locked --target x86_64-pc-windows-msvc -- -D warnings
```

At `3fd1a3d` that printed the runner's three findings from run `34877495185`
plus the fourth it never scheduled (`unused import: std::path::Path`,
`crates/fleet/src/transport.rs:506`, `lib test` target) — in 1.9 seconds against
a ~50-minute CI round trip.

**Which crates the method covers is computable, not guessable.**
`cargo metadata --format-version 1 --locked --filter-platform x86_64-pc-windows-msvc`
gives the resolved Windows graph; walking it for packages that reach a native C
build splits the workspace exactly:

| | crates | why |
|---|---|---|
| lintable for Windows from here | **28** | no C build script anywhere in the Windows graph |
| not lintable from here | **25** | 23 reach `libsqlite3-sys` (bundled SQLite), 2 also reach `llama-cpp-sys-2` + `clang-sys`, 1 (`aethercore-desktop`) also reaches `tauri`, `vswhom-sys`, `webview2-com-sys` |

That is why the whole-workspace cross-target form dies in `cc-rs`, and it is a
property of the dependency graph rather than of the host's patience.

**`--keep-going` is what turns the layers into sweeps.** Without it cargo stops
scheduling at the first crate that fails, which is precisely the mechanism that
hid ten crates from the runner. With it, the first full sweep over the 28
printed **41 findings across 11 crates** in one run instead of eleven.

## 2. What the layers held

Seven sweeps, each one re-run after the previous one's fixes. The interesting
content, not the full list — the full list is the diff:

* `apps/install-hardener/src/main.rs:177` — `suspicious_open_options`: the
  machine mutation lock file was opened `create(true)` with no stated
  truncation. Behaviour preserved exactly (`truncate(false)`); an existing lock
  keeps its contents.
* `crates/ipc/src/lib.rs` — three legacy payload-frame helpers were dead in the
  `lib` target on every host. `read_request` is live (it is the decoder behind
  `decode_request_frame_bytes`); `write_request` has only test callers and is
  now `#[cfg(test)]`; `write_response`/`read_response` had **no caller anywhere
  in the workspace** and are deleted.
* `crates/windows-update/src/execution_windows.rs:40` — `MachineMutationLease`
  is an RAII holder whose field is never read by design. A tuple field cannot be
  underscore-named, so it is a named `_guard` field now, with the reason in a
  doc comment, rather than an `allow`.
* `crates/operation-engine/src/lib.rs:174` and
  `crates/operation-kernel/src/event_bus.rs:110` — `large_enum_variant` (528 and
  560 bytes). Both boxed. **`serde` and `prost` see `Box<T>` exactly as `T`, so
  the persisted plan JSON and the wire event are byte-unchanged**; 5 and 4 call
  sites, all in-repo.
* `assertions_on_constants` in four test bodies became `const { assert!(…) }` —
  the invariant now fails at compile time instead of test time. One message lost
  its interpolated constant, because a `const` panic takes a literal.

## 3. The three things that are reasoning, not measurement

Named here so the next session can retire them with a command rather than trust.

**(a) Three uses of the deprecated `Response::error_message` live inside
`#[cfg(windows)]` code in crates this host cannot cross-compile**, so no clippy
run anywhere has seen them: `apps/update-broker/src/main.rs:172`,
`apps/desktop/src/main.rs:411`, `services/maintenance-service/src/server.rs:511`.
They are fixed on evidence rather than on a whim: the cross-target run *did*
flag the identical read at `apps/consent-broker/src/main.rs:77`, and
`services/maintenance-service/src/router/dispatch.rs:222,245` already carries
`#[allow(deprecated)] // wire-contract placeholder field (proto field 3)` for the
identical write. Read sites now take `ErrorInfo::technical_detail`, which both
writers set to the same string the deprecated field carries; the write site keeps
the field behind the same `allow`, because it is wire contract.

**(b) Two lint classes were swept across the unlinted surface by grep and found
absent** — `map_or(false|true, …)` and the `(x + n - 1) / n` shape of
`manual_div_ceil` — across **all 157 `.rs` files of the 25 unlintable crates**,
gated code included. Zero hits. That is a sweep of two classes, not a lint run.

**(c) The remainder is measured as line counts, not as findings.** Files
containing `cfg(windows)`: **23 files / 6,823 lines in crates now linted for
Windows**, **17 files / 13,946 lines in crates that still cannot be**. By
filename, the Windows-only implementation files: **10 files / 5,715 lines now
linted** (`windows_impl.rs` in `ipc`, `crash-diagnostics`, `hardware-telemetry`,
`performance-telemetry`, `driver-backup`, …), **5 files / 3,067 lines not**
(`cleaner`, `startup-manager`, `system-repair`, `idle-scheduler/windows_state.rs`,
`maintenance-service/windows_service_host.rs`). Those are whole-file counts, not
gated-region counts; the gated region is smaller and I did not measure it.

## 4. The trap in `cargo clippy --fix`, which nearly shipped

`--fix` was used for machine-applicable suggestions in eight crates and every
hunk was re-read. **One was wrong and it would have broken the Windows build.**
In `services/maintenance-service/src/main.rs` it deleted

```
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use anyhow::Context;  use tracing::warn;
```

because on macOS nothing uses them — and every one of them is live inside a
`#[cfg(windows)] fn` in the same file (`run_console`, `run_server`). `--fix`
sees one cfg. They are restored behind `#[cfg(windows)]`, which is green on both
hosts. **Every removed `use` line in this diff was then re-grepped against its
crate's Windows code**; `aethercore_product_identity::SERVICE_NAME` in the same
file had no reference under any cfg and stays deleted.

## 5. What was deliberately not fixed

**The 36 findings the host still prints.** `cargo clippy --workspace
--all-targets --locked -- -D warnings` exits 101 with 36, and all 36 are the two
classes P66 named as not firing on the runner:

* **31 are `dead_code` whose consumer is `cfg(windows)`** — `consent-broker`,
  `install-hardener`, `update-broker`, `desktop`, `cleaner`, `driver-backup`,
  `hardware-telemetry`, `security`. For the five of those crates inside the
  linted 28 — `consent-broker`, `install-hardener`, `driver-backup`,
  `hardware-telemetry`, `security` — this is now *proven* rather than argued:
  the cross-target run compiles that exact code and does not flag it. For `cleaner`, `desktop` and
  `update-broker` it was checked by grep into `windows_impl.rs` / the
  `cfg(windows)` regions that call them.
* **4 are in `crates/performance-telemetry/src/macos_impl.rs`**, which the
  runner never compiles, and **1** is in a `#[cfg(unix)] fn`
  (`crates/security/src/lib.rs:536`).

**An observation, not a change:** `set_owner_only` in `crates/fleet/src/trust.rs`
is now an explicit `#[cfg(unix)]` / `#[cfg(not(unix))]` pair instead of one
function with a dead parameter. The non-unix body still returns `Ok(())`, so the
fleet `known_hosts` file gets **no owner-only ACL on Windows**. That was already
true; it is now visible and commented. Tightening it means writing Win32 ACL
code and belongs in its own change with a Windows run attached.

## 6. Everything that was run

| gate | result |
|---|---|
| `cargo clippy` × 28 crates, `--target x86_64-pc-windows-msvc --all-targets --locked -- -D warnings` | **0 errors** |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` (host) | 36, all in §5's two classes |
| `cargo fmt --all -- --check` | clean |
| `cargo test --workspace --locked` | **649 passed, 0 failed, 1 ignored, 135 suites, exit 0** |
| `scripts/ps_marker_scan.py` | 234 assertions, **0 failed, 3 unmeasured** — unchanged; all three are `$variable` arguments the tool does not evaluate |
| `scripts/static_validate.py` | `ok: true`, 347 checks |
| `scripts/test-phase12-localization.py` | 34/34 |
| `scripts/phase12-localization-audit.py` | PASS |
| `scripts/enterprise-adversarial-audit.py` | `failed: []` |
| `scripts/test_gate_readers.py` | 14 readers fail closed |
| `scripts/test_source_seal.py` | 9/10 before the manifest regeneration in this commit — the one FAIL is *this* diff, by design |
| `phase10` line budgets, hand-read | `main.rs` 150 (<220), `router.rs` 116 (<220), largest router module `dispatch.rs` **258** (<260) — **unchanged, still two lines of headroom** |

## 7. What the next session should do

1. Push, and read the windows job's step 21. It should now clear
   `verify-enterprise.ps1:24` for the 28 and fail — if it fails — inside one of
   the 25, in code no host has linted. That output is the only way to see §3(c).
2. Every finding it prints from those 25 is cheap to fix and impossible to
   predict from here. Do not guess at them.
3. Dependabot stays paused until that job is green on `main`.
