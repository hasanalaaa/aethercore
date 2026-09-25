# P75 lane `mac-clippy` — evidence

Base `9747fe3`. Branch `lane/mac-clippy`. Host: macOS (aarch64), toolchain 1.97.1
(pinned by `rust-toolchain.toml`, same as CI).

## What the brief said vs. what was measured

The brief listed 6 findings in 2 files (`driver-backup/src/lib.rs`,
`performance-telemetry/src/macos_impl.rs`). That list is what cargo prints before
it stops at the first failing crates. With `--keep-going` the base has **30 more
in 7 more files**, and once those compiled, cargo reached `maintenance-service`
and printed **8 more in 2 more files**. Total fixed: **36 findings, 11 files**.
The measurement wins over the brief.

| class | files | findings |
|---|---|---|
| macOS-only code (the runner never compiles it) | `crates/performance-telemetry/src/macos_impl.rs` | 4 |
| `dead_code` / unused imports that exist only because macOS cfg's out their Windows consumer | `crates/driver-backup/src/lib.rs`, `crates/hardware-telemetry/src/lib.rs`, `crates/cleaner/src/lib.rs`, `crates/security/src/lib.rs` (`sid_text_from_bytes`), `apps/consent-broker/src/main.rs`, `apps/update-broker/src/main.rs`, `apps/install-hardener/src/main.rs`, `apps/desktop/src/main.rs` | 31 |
| `cfg(unix)` code no clippy run had ever reached | `crates/security/src/lib.rs:536` (`unused_unsafe`), `services/maintenance-service/src/unix_composition.rs`, `services/maintenance-service/src/unix_service.rs` | 1 + 8 |

## How each class was fixed (no `#[allow]` added, no behaviour change)

- `macos_impl.rs`: `if memsize > 0 { (used*100)/memsize } else { 0 }` became
  `(used * 100).checked_div(memsize).map_or(0, |p| p as u32)`. Same result for
  a zero divisor, which is 0. When `memsize == 0`, `used` is 0 too, so the
  multiplication cannot overflow sooner than it did before. `u64::from` came off
  `f_blocks`/`f_bavail`, and `as usize` came off `MAXCOMLEN`: in libc 0.2.189
  (apple), these are already `u64` and `usize`.
- Windows-consumer items were gated in place: `#[cfg(windows)]`, or
  `#[cfg(any(windows, test))]` where a host-agnostic unit test uses the item
  (the hardware-telemetry parsers and `sid_text_from_bytes`). They were not moved
  because text-token gates read them where they are: `static_validate.py`
  `cleanup_immutable_state_and_limits` needs `MAX_FILES_PER_CANDIDATE` in
  `cleaner/src/lib.rs`, and `phase13_storage_byte_parsers` needs the parser names
  in `hardware-telemetry/src/lib.rs`. Each import that became host-unused was
  checked by grep against its Windows users before it was gated. Desktop's setup
  closure got `#[cfg(not(windows))] let _ = app;`.
- `maintenance-service` unix: removed an unused `Transport` import and an unused
  `Ordering` import. Removed `IPC_BOOTSTRAP_ENQUEUE_TIMEOUT`, which is a dead copy
  of the `server.rs` constant. Removed a needless `mut`. Added
  `type SocketAnnouncer`. Collapsed two `if`s with let-chains (edition 2024).
  Renamed `subscription` to `_subscription`: a named binding, **not `_`**, so the
  subscription is still held for the whole session, exactly as before.

## Proof (local, at the lane head)

```
cargo clippy --workspace --all-targets --locked -- -D warnings            (macOS)
    Finished `dev` profile [unoptimized] target(s) in 0.49s      exit 0
cargo clippy -p aethercore-{driver-backup,hardware-telemetry,security,consent-broker,install-hardener}
    --target x86_64-pc-windows-msvc --all-targets --locked -- -D warnings   exit 0 (each)
cargo clippy -p aethercore-performance-telemetry --target x86_64-pc-windows-msvc --lib -- -D warnings   exit 0
cargo clippy -p aethercore-{security,hardware-telemetry,driver-backup} --target x86_64-unknown-linux-gnu --all-targets -- -D warnings   exit 0
cargo test --workspace --locked      649 passed / 0 failed / 1 ignored, 135 suites, exit 0
cargo fmt --all -- --check           exit 0
static_validate.py                   checks 347, failed []
test_gate_readers.py                 all 14 readers fail closed
ps_marker_scan.py                    total assertions=234 failed=0 unmeasured=3 (same 3 as base)
source_seal.py                       OK - 1497 of 1497 tracked files verified
```

Some checks could not be run on this host, so they are not claimed:
- The Windows-target clippy runs for `cleaner`, `update-broker`, `desktop` and
  `maintenance-service` die in `libsqlite3-sys`'s C build.
- The Windows-target clippy for performance-telemetry with `--all-targets` dies
  in `alloca`, which the dev-dependency `criterion` pulls in.

For those, the verdict is the Windows CI job, which runs
`cargo clippy --workspace --all-targets --locked --keep-going -- -D warnings` at
`verify-enterprise.ps1:23`.

Audits that name a touched file: 22 were run. Thirteen exit 0. Nine exit 1 at
the lane head, and the same nine exit 1 at base `9747fe3` (exported with
`git archive`). A line-by-line diff of their failure output shows **no failure
that is new in the lane**. The only difference is a random temp-directory name
in `phase33`. The base failures are host-bound: P27 wire-freeze tags, Windows
qualification marked NOT_EXECUTED, and a source seal that cannot be checked
outside git.

## Ledger

### `DBT-P63-010`: proposed status cell

The row is malformed (8 cells, not 5) because its text contains literal pipes:
`bytes < 2 || bytes > 64 * 1024` and `map_or(false|true, …)`. The ledger's own
parser splits cells on `|`. The replacement text below has no pipe character:

> CLOSED in P75 (lane mac-clippy). On this Mac,
> `cargo clippy --workspace --all-targets --locked -- -D warnings` exits 0. It
> had exited 101 on 36 findings across 11 files, in three classes: macOS-only
> code, dead code whose consumer is `cfg(windows)`, and `cfg(unix)` code that no
> run had reached. All were fixed without `allow`. Windows: CI run `<RUN_ID>` at
> `<HEAD_SHA>` ran the same command through `verify-enterprise.ps1:23` and it
> passed. Evidence: `docs/phase75/lanes/mac-clippy.md`.

(The lead fills in the run id and sha from the PR's green run. If that run turns
out red at clippy, the cell stays OPEN and names the step.)

### Proposed new rows (text only, the lead assigns ids)

1. **Linux-target clippy is red and no gate runs it.**
   `cargo clippy -p aethercore-performance-telemetry --target x86_64-unknown-linux-gnu --lib -- -D warnings`
   reports 5 findings in `crates/performance-telemetry/src/linux_impl.rs`:
   - `:204` `manual_checked_ops`
   - `:336`, `:338`, `:339` `useless_conversion`
   - `:405` `collapsible_if`

   The same class as the macOS four, and present on base (this lane does not
   touch `linux_impl.rs`). Not fixed here because it is outside the lane's file
   list. Status: OPEN, machine: any.
2. **Unix IPC never forwards live events.**
   `services/maintenance-service/src/unix_composition.rs` subscribes to the
   `EventBus` for each session and never reads the subscription. So after the
   Hello replay, a UDS client receives no live events. Once
   `SUBSCRIBER_CAPACITY` events are queued, the bus's `try_send` fails and it
   drops the subscriber. The Windows host (`server.rs:285`) drains it with
   `recv_timeout`. Clippy surfaced this as `unused variable: subscription`. The
   lane only renamed the binding and kept behaviour identical. Status: OPEN,
   needs an owner decision, machine: macOS/Linux.


## Integration verification (2026-09-25)

Main `9aa1cef72e7b479140c2ec46a21d14c31534cd3f` was confirmed green by
`gh api repos/hasanalaaa/aethercore/actions/runs/36084004450` (`success`, same
`head_sha`) before integrating it. Prior lane CI `36058189476` is independently
confirmed `success` at `753cfd68772b7f74681b80fb2838fc9880db2ffd`.

Fresh local commands, with all three disk-budget environment variables set:

```text
cargo fmt --all -- --check
exit 0
cargo clippy --workspace --all-targets --locked -- -D warnings
Finished `dev` profile [unoptimized] target(s) in 1.00s; exit 0
cargo test --workspace --locked
649 passed; 0 failed; 1 ignored; 135 suites; exit 0
cargo clippy -p aethercore-driver-backup -p aethercore-hardware-telemetry -p aethercore-security -p aethercore-consent-broker -p aethercore-install-hardener --target x86_64-pc-windows-msvc --all-targets --locked -- -D warnings
exit 0
cargo clippy -p aethercore-performance-telemetry --target x86_64-pc-windows-msvc --lib --locked -- -D warnings
exit 0
python3 scripts/static_validate.py
{"ok": true, "checks": 347, "failed": []}
python3 scripts/test_gate_readers.py
all 14 readers fail closed
python3 scripts/ps_marker_scan.py
total assertions=234 failed=0 unmeasured=3
UNMEASURED $lockBaseline, $manifestBaseline (freeze-dependencies.ps1)
UNMEASURED $MutationLock (verify-installer-security.ps1)
Each is an expression this scanner does not evaluate.
```

Nineteen relevant audit scripts were re-run. Ten exited 0; nine exited 1.
The nine were also run in a detached worktree at `9aa1cef`: phase17, phase19,
phase27, phase28, phase31, phase32, phase33, phase34 and phase35. Failure output
matches after accounting for workspace/temp paths and Python set ordering.
These remain failures, not local passes. No new failure was found.

The two proposed new ledger rows are assigned `DBT-P75-001` (Linux lint,
reproduced: exit 101 with five findings) and `DBT-P75-002` (Unix event delivery,
source inspection). `DBT-P63-010` is closed and its literal table-cell pipes
are encoded so the ledger's five-cell parser can count it.
