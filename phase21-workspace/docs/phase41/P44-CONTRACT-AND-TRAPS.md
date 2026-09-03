# P44 — close the contract gap on macOS/Linux, disarm the build traps

Paste into Claude Code:  `Read and follow phase21-workspace/docs/phase41/P44-CONTRACT-AND-TRAPS.md and work it end to end`

**This session runs on the Mac** at `/Users/hasanalaaa/dev/aethercore`.
NOT the design worktree at `/Users/hasanalaaa/dev/aethercore-design`, and never
`~/Documents/ZZ-OLD-AetherCore-DO-NOT-USE`.

Read `SESSION_CONTEXT.md` §20.1, §42 and §43 before writing code. §42 is the
Windows fix you are extending; read how it was done before doing it again.

---

## HOW TO WORK THIS FILE

- Write every finding to §44 as you get it. Commit and push after every numbered
  item, not at the end. A P41 session died at its usage limit mid-gate.
- Every check has an EXPECTED value. **Observed differs: stop that item, record
  the raw observation verbatim, move to the next independent item.** Do not
  theorise, do not redefine the criterion.
- A security regression stops everything immediately.

---

## PART 1 — DBT-P42-005: the contract is Windows-only

P42 fixed a defect where `PerfPlatform::sample` could return a plausible-looking
all-zero payload with no fault, because `PerfSnapshot` derives `Default` and the
signature could not express "I could not measure this". The fix was `Reading<T>` —
a wrapper over a private enum, so `Measured` cannot be constructed without passing
evidence to a constructor. Nine hand-written availability rules became one contract.

**That fix landed on Windows only.** §42.9 records DBT-P42-005: the macOS and
Linux providers still build `PerfSnapshot` literally rather than through
`CollectedSubsystems`. It was left open because the Windows machine could not
compile them — "changing code this session cannot build is the worse risk", which
was the right call there.

**This machine compiles macOS natively, and Linux is reachable.** That is why this
session exists.

### 1.A — establish what is actually there before changing it

Read `crates/performance-telemetry/src/macos_impl.rs` and `linux_impl.rs` in full.
Report, with file:line for each:

1. Can each provider return success with an all-zero or empty payload and no
   fault? Show the exact path that permits it.
2. Does either have its own equivalent of the Windows mechanisms — a status code
   or sentinel read as a value, a dropped temporary, an early return that pushes
   no fault, a counter read before the collection that fills it?
3. How many hand-written availability rules does each carry, and does any of them
   fire for the right reason rather than by accident? On Windows, exactly one
   fired, and §20.1.6 records that it fired **by accident**.

Do not assume the macOS/Linux defects mirror Windows. Fixing Windows uncovered
four further defects underneath (DBT-P42-001/002/003/008) that nobody predicted.
Look for what is actually there.

### 1.B — write the failing tests first, and commit them failing

This project's practice, twice now: the LocalSystem authorization fix and the P42
telemetry fix both committed their regression tests failing before the fix existed.

§20.1 records that no test constructed `WindowsPerfPlatform`, and both real-path
tests asserted **upper bounds only** — so all-zero passed them. That is the hole.
Assert **lower bounds** on a machine doing work.

At minimum, per platform:
- the real provider reports non-zero CPU busy under load
- no collector can return success with an empty payload and no fault
- `capabilities` cannot claim `native` for a subsystem that reported nothing

Commit failing, with the raw test output in the commit message.

### 1.C — apply the contract

Bring both providers onto `Reading<T>` / `CollectedSubsystems`. Delete every
availability rule the contract makes redundant and report how many survived and
why.

**Do not change Windows behaviour.** Windows is qualified on two architectures
(§42, §43). If a shared-code change touches it, say so explicitly and show the diff.

### 1.D — prove it on this machine

    cargo test -p aethercore-performance-telemetry
    cargo build --release

Then run the real provider on macOS while the machine is under load, and diff
every reading against the host's own numbers — `iostat`, `vm_stat`, `powermetrics`,
`top`. Do not assert plausibility; show both numbers side by side with the delta
in points and as a ratio.

**Watch for the bias.** §43.5 established that x64 Windows reads consistently high
(+7.5 and +4.1 points on cpu, 3.47x on disk latency) while ARM64 Windows does not.
That is still unexplained and DBT-P42-011 is open. Whether macOS shows it too is a
third data point on a question with only two — report the deltas either way.

Linux cannot be measured on this host. Say so; do not skip it silently. Compiling
and testing it is still in scope — `cargo check --target` at minimum, and state
exactly what you could and could not verify.

---

## PART 2 — the build traps

### 2.A — DBT-P42-012: `vcomp140.dll` is named but not sourced

`scripts/build-installer.ps1` requires `vcomp140.dll` and does not say where it
comes from. §42 records **five files of that name on the machine, and the first
plausible match is wrong**. It was caught only by hashing against §41.14 before
building. The script is unchanged, so the trap is still armed.

This is the same class as the defect that stopped the x64 package building at all:
`Product.wxs` hard-coded the ARM64 OpenMP runtime, so x64 could not package. Both
are "a file named without its provenance".

Fix it so the source is explicit and verifiable — the script should state where
the file comes from and fail loudly if it is not that file, rather than picking
the first match on a path. A hash check is acceptable; a comment is not.

You cannot run this script here. Say what you changed, what a Windows session must
verify, and write that verification down as a numbered check with an EXPECTED
value so the next Windows session can just run it.

### 2.B — DBT-P43-001: the ARM64 recipe lies about its own exit code

`C:\AetherCore-P36\logs\p36_relbuild.cmd` builds a second target,
`--example ipc_two_client_probe`, which does not exist in `aethercore-ipc` — no
`examples/` directory, no `[[example]]` entry, on either machine. So the recipe
exits 101 even when the shipping build it exists to validate succeeded, and P43
had to prove Gate 1.B from the log rather than from the exit code.

A build script whose exit code cannot be trusted is a measuring instrument that
lies — the recurring pattern in this project.

The recipe lives on the VM, not in the repo. Decide and record which is right:
bring it into the repo under version control, or fix it in place and document it.
Argue for one; do not do both.

---

## OPTIONAL — DBT-P42-006, diagnose only

`aethercore-driver-hub --lib` has 6 failing tests, pre-existing and verified by
stashing. Nobody has looked at why. If Parts 1 and 2 are committed clean and you
have session left, diagnose them — **do not fix**. Report what they are and
whether they indicate a real defect or a stale test.

---

## REPORT

- what the macOS and Linux providers actually permitted, with file:line
- how many availability rules survived the contract on each, and why
- every macOS reading beside the host's, with the delta in points and ratio, and
  whether the x64 bias appears there
- exactly what could not be verified on this host, named rather than omitted
- the Windows verification checks Part 2.A leaves behind, with EXPECTED values
- anything recorded rather than worked around, with its debt ID
