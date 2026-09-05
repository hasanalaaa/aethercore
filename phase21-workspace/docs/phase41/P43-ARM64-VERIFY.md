# P43 — verify the P42 fix on ARM64, and settle the numeric bias

Paste into Claude Code:  `Read and follow phase21-workspace/docs/phase41/P43-ARM64-VERIFY.md and work it end to end`

**This session runs on the Mac** at `/Users/hasanalaaa/dev/aethercore` and drives
the Parallels VM through `prlctl`. It is NOT a session inside the VM.

NOT the design worktree at `/Users/hasanalaaa/dev/aethercore-design`, and never
`~/Documents/ZZ-OLD-AetherCore-DO-NOT-USE`.

---

## WHY THIS SESSION EXISTS

`crates/performance-telemetry/src/windows_impl.rs` is `#[cfg(windows)]`, **not**
`#[cfg(target_arch)]`. The ARM64 pipeline — the qualified, shipping one — runs that
exact file, and P42 changed it underneath: the oversized-write fix, the corrected
`PdhExpandWildCardPathW` arity, the collection-order fix, the percentage-vs-basis-
points fix, and the `Reading<T>` contract that replaced nine availability rules.

P42 recorded `DBT-P42-004` open because no ARM64 machine was attached to it. One
is attached to this one. Until this runs, the only shipping-qualified pipeline in
the project carries unverified changes.

Read `phase21-workspace/docs/phase36/SESSION_CONTEXT.md` §20.1, §41.15, §41.16 and
§42 before touching anything.

---

## HOW TO WORK THIS FILE

- **Write every finding to disk as you get it.** §43 in `SESSION_CONTEXT.md`.
- **Commit and push after every numbered item**, not at the end.
- Every check has an EXPECTED value. **Observed differs: stop that item, record the
  raw observation verbatim, move to the next independent item.** Do not theorise.
- A security regression stops everything immediately.
- Long builds print little. Not hung.

### VM operating facts that cost real time to learn

- `prlctl exec "Windows 11" cmd.exe /c "..."` — the **bare** form returns EMPTY.
- `prlctl` argv is limited to about 16 KB, and nested quoting through zsh into
  PowerShell is fragile. **base64 any real script on the Mac and decode it on the
  VM.** Do not fight the quoting.
- **MSVC is NOT supported for ARM64 there.** The known-good recipe is
  `C:\AetherCore-P36\logs\p36_relbuild.cmd` — VsDevCmd arm64 + clang-cl + Ninja +
  libomp. `LIBCLANG_PATH` and `cmake` are both required and both live under
  `C:\AetherCore-P36\toolchain`.
- MSI packaging takes ~12 minutes and prints nothing. The 1.07 GB model dominates
  CAB compression. **It is not hung.**

---

## 0 — RESUME THE VM. Do not restore anything.

The VM is currently **suspended**. Resume it. `prlctl resume`, not `prlctl snapshot-switch`.

**Restoring a snapshot would discard the installed state this work depends on.**
These two are FORBIDDEN under all circumstances — they predate the install:

    P36-CLEAN-BASELINE       {6b721a10-8ac1-4339-9699-bd5145efda0a}
    P36-PRE-NATIVE-MUTATION  {e9434b5f-ba68-452d-ac4c-cbcc863ffb4b}

Confirm the VM is up and reachable:

    prlctl exec "Windows 11" cmd.exe /c "echo REACHABLE && ver"

EXPECTED: `REACHABLE` and a Windows version string. If `PRL_ERR_IO_STOPPED` or an
empty reply persists after the resume, stop and report — do not retry in a loop.

Then record what state the VM is actually in before you change it: current
snapshot, installed product version, whether the service is running.

---

## PART 1 — BUILD AND TEST ON ARM64 (non-destructive)

**1.A — get the P42 code onto the VM.**

Determine how the VM's working copy is fed — a git clone, a share, or a copy — and
report which. Bring it to `a39a1bc`. Verify:

    git log --oneline -1     EXPECTED: a39a1bc
    git config core.autocrlf EXPECTED: false

`core.autocrlf` MUST be false. This project hashes files; CRLF conversion breaks
every SHA check, and it has already caused a 1027-file incident on the other machine.

**1.B — build ARM64 release** with `p36_relbuild.cmd`. Record the exit code and
duration. EXPECTED: exit 0.

If it fails to compile, that is the single most important finding of this session —
it means P42 broke the shipping pipeline. Record the full error and STOP Part 1.

**1.C — run the P42 regression tests on ARM64.**

    cargo test -p aethercore-performance-telemetry

The four `dbt_p41_002` tests were committed FAILING at `7818817` and pass on x64
since `cc9c51e`. EXPECTED: they pass on ARM64 too.

A test that passes on x64 and fails on ARM64 is a real architecture divergence in
code that has no architecture-specific path. Report it as such.

---

## PART 2 — MEASURE, AND SETTLE THE NUMERIC BIAS

This is the part that produces new knowledge, not just confirmation.

**2.A — the product's own readings**, against the freshly built ARM64 binary:
`telemetry-once` and `perf snapshot`, while the VM is doing work.

EXPECTED per §42: cpu busy non-zero and plausible, storage returns real instances
or an explicit fault with a reason, gpu declares its state honestly.

**2.B — the host's own counters, in the same window.** Use `Get-Counter` inside the
VM. Do not assert plausibility — diff every product reading against the host's.

**2.C — the bias question. This is the point of Part 2.**

P42's x64 numbers show a consistent bias nobody commented on:

    cpu offline    product 58.38%   host 50.91%    +7.5 points
    cpu service    product 52.31%   host 48.22%    +4.1 points
    disk latency   product 833 us   host 240 us    x3.5

Two readings biased the same direction is not sampling noise, and 3.5x is not
rounding. `DBT-P42-011` (byte rates under-report against a 1 s window) is already
open on the cadence decision; this may be the same defect showing a second face.

The experiment: **the same code runs on both architectures.** So —

- If ARM64 shows the **same** bias, it is in the shared logic — the sampling window,
  the denominator, or the conversion — and `DBT-P42-011` should be widened to name it.
- If ARM64 shows **no** bias, it is x64-specific, which would be surprising in code
  with no architecture-specific path, and is a finding in its own right.

Report the delta for every reading in points and in ratio. State which of the two
outcomes you observed. **Do not fix it** — this session establishes which it is.

**2.D — is `perProcessorBusyBp` still empty on ARM64?**

`DBT-P42-009` records it as a missing feature on Windows, not a symptom of the
misread. EXPECTED: `[]` on ARM64 as well, confirming it is a feature gap and not
architecture-dependent.

---

## PART 3 — CLOSE OR OPEN THE DEBT

Write the verdict for `DBT-P42-004` in §43 and update its row:

- **Closed** — ARM64 builds, the four tests pass, and the measurements are
  consistent with x64 except for anything you recorded explicitly.
- **Still open, narrowed** — say exactly what remains unproven and why.
- **Escalated** — ARM64 diverges. Then the shipping-qualified pipeline has a real
  defect introduced by P42, and that outranks everything else in the project.

---

## OPTIONAL PART 4 — service-path measurement. Snapshot-gated.

Only if Parts 1-3 are committed clean, and only if you judge it adds something
Part 2 did not.

Installing a rebuilt MSI on the VM is a **destructive action**. Before it:

1. Write the **ACTION / SNAPSHOT / EXPECTED / RECOVERY** record and commit it.
2. Take a NEW named snapshot — `P43-PRE-ARM64-INSTALL`. Do not reuse an old one,
   and do not delete any existing snapshot.
3. Record the new snapshot's UUID in §43 so it can be recovered by name later.

Versions **0.1.9 and 0.1.10 are recorded must-not-ship**. The VM currently carries
0.1.11.

If you do install: re-prove the pipe DACL, Service SID UNRESTRICTED, install-dir
ACLs, and `engineLabel=localModel` against the running service, exactly as Gate 2
does. The DACL criterion:

    O:<service SID> G:SY D:P(A;;FA;;;<service SID>)(A;;FR;;;AU)(A;;DC;;;AU)

`(A;;0x12008b;;;AU)` is the SAME DACL — `FR|DC = 0x120089|0x2 = 0x12008b`. **Not drift.**

---

## REPORT

- ARM64 build and test result, with exit codes and durations
- every product reading beside the host's, with the delta in points and ratio
- **which of the two bias outcomes you observed**, stated plainly
- the `DBT-P42-004` verdict
- anything recorded rather than worked around, with its debt ID
- whether you took Part 4, and if so the new snapshot UUID
