# P45 — DBT-P44-003, and the class it belongs to

Paste into Claude Code:  `Read and follow phase21-workspace/docs/phase41/P45-FIELD-CONTRACT.md and work it end to end`

**Runs on the Mac** at `/Users/hasanalaaa/dev/aethercore`. Not the design worktree,
never `~/Documents/ZZ-OLD-AetherCore-DO-NOT-USE`.

Read `SESSION_CONTEXT.md` §20.1, §42, §44 first — especially §44's addendum on
DBT-P44-003, which P44 left open with the root cause not established.

---

## THE ROOT CAUSE IS ESTABLISHED. Confirm it, then fix the class.

`crates/performance-telemetry/src/macos_impl.rs:115`

    pub fn busy_bp_from_ticks(previous: CpuTicks, current: CpuTicks) -> u32 {
        let busy  = current.busy().saturating_sub(previous.busy());
        let total = current.total().saturating_sub(previous.total());
        if total == 0 {
            return 0;
        }

`total == 0` means no time elapsed between the two samples — the tick counters did
not advance. That is **"I have no measurement"**, not **"the CPU was 0% busy"**.
The function returns a plain `u32`, so the contract above it faithfully wraps a
failure as `Measured(0)`.

This is the same defect the entire P42/P44 arc exists to eliminate, surviving
*inside* the fix: a status condition reported as a measurement, exactly like
`read_u64` reading `CStatus` — where `PDH_CSTATUS_VALID_DATA == 0` — and
publishing it as a percentage.

`Reading<T>` cannot catch it. The zero is produced **below** the contract, inside a
pure helper whose return type cannot say "no window". The type hands over a
number; the contract wraps a number. The contract was applied at the **subsystem**
boundary and never at the **field** boundary underneath it.

**Confirm this before you act on it.** Instrument `busy_bp_from_ticks` to record
whether `total == 0` on each flaking run and show that the ~1-in-10 zeros
correspond to that branch and not to something else. If they do not, stop — you
have a different defect and this brief's premise is wrong. Say so.

---

## PART 1 — the census. Do this before any fix.

DBT-P44-003 is one instance caught by luck, because a test happened to assert a
lower bound. Find the rest.

Across `macos_impl.rs`, `linux_impl.rs` and `windows_impl.rs`, enumerate every
place a failure, an absence or a degenerate case becomes a numeric zero or an
empty collection. Start from these, already located, and do not assume the list
is complete:

    macos_impl.rs:118    total == 0 -> return 0
    macos_impl.rs:335    total == 0
    macos_impl.rs:178    .unwrap_or(0)
    linux_impl.rs:104    total == 0 -> return 0        (identical shape)
    linux_impl.rs:91-94  .unwrap_or(0) parsing /proc/stat fields
    windows_impl.rs:714, 723, 724, 725   .unwrap_or(0) in the storage path

Note `windows_impl.rs:386` carries a comment that already names this exact
pattern — "`.unwrap_or(0)` is how a failed [read] silently defaulted to zero" —
citing §20.1.3(a). The codebase has known about the shape and kept using it.

For each site, classify it as exactly one of:

  **A — a real measured zero.** The quantity was read and genuinely is zero.
       Legitimate. Leave it, and say why it is unambiguous.
  **B — a swallowed failure.** A parse failed, a call failed, a window was empty,
       a counter did not advance. The zero is a lie. Must be fixed.
  **C — cannot tell from the code.** Say so rather than guessing.

Produce the table with file:line for every entry. **Commit the census before
writing any fix** — it is the deliverable even if the fixes are not all finished.

---

## PART 2 — push the contract down to the field boundary

For every **B**, the helper must be able to say it has nothing:

    fn busy_bp_from_ticks(..) -> Option<u32>     // None when total == 0

and the caller turns `None` into a `Reading::unavailable` with a fault that names
the reason — "no tick delta in sampling window" — never into a zero.

The principle, and the reason this is the lazy fix rather than a bigger one: **the
type must not be able to express a measurement that was never made.** One contract
at the field boundary removes the whole class, the same way one contract at the
subsystem boundary removed nine hand-written availability rules in P42. Do not add
per-site guards.

Where a `Duration` or window is genuinely too short to measure, that is a fault
with a reason, not a reading of zero.

**Do not change Windows or ARM64 behaviour beyond removing the lie.** Both are
qualified (§42, §43). If a shared change touches them, say so and show the diff.

---

## PART 3 — tests that would have caught it

P44's own test caught DBT-P44-003 only because it flaked at ~1-in-10 and someone
ran it enough times. That is luck, not coverage.

Write tests that fail deterministically today:

- `busy_bp_from_ticks` with two identical `CpuTicks` — i.e. no elapsed window —
  must NOT return a measurement. EXPECTED: it reports unavailable.
- the same for the Linux equivalent at `linux_impl.rs:104`.
- for each **B** site, a test that drives the failure path and asserts a fault is
  raised rather than a zero published.

Commit them failing, with raw output in the commit message, as P42 and P44 did.

Then run the previously-flaky macOS test **at least 50 times** and report the
zero-rate. EXPECTED: zero occurrences of `totalBusyBp: 0` under guaranteed load.
Report the raw count, not a summary.

---

## PART 4 — measure, and the open bias question

Run the real provider on macOS under load and diff every reading against the
host's own counters. Show both numbers with the delta in points and as a ratio.

§43.5 left DBT-P42-011 open: x64 Windows reads consistently high (+7.5, +4.1
points on cpu, 3.47x on disk latency), ARM64 Windows does not. §44 measured macOS
once. If this fix changes the macOS deltas, that is evidence about the bias, not
just about this defect — say so explicitly either way.

---

## REPORT

- the confirmation that the ~1-in-10 zeros are the `total == 0` branch, or that
  they are not
- the full A/B/C census table with file:line
- how many B sites existed, how many are fixed, and any left open with a reason
- the 50-run zero-rate, raw
- macOS readings beside the host's, with deltas
- whether Windows or ARM64 behaviour changed at all
- anything recorded rather than worked around, with its debt ID
