# P46 Lane C — the x64 items: the bias, and the release pipeline

Paste into Claude Code:  `Read and follow phase21-workspace/docs/phase41/P46-LANE-C-X64.md and work it end to end`
To resume after a usage limit, paste the same line again.

**Runs on the physical Windows PC** at `C:\dev\aethercore`, from an
**already-elevated PowerShell** (right-click → Run as administrator, then launch
the tool). §41.9 records that one action as what unblocks everything.

---

## BRANCH DISCIPLINE — read before your first commit

Another session owns `main` and `SESSION_CONTEXT.md` §46 right now and is
committing to it continuously. **You must not push to main and must not edit §46.**

    git fetch origin
    git checkout -b feat/p46-x64 origin/main
    git push -u origin feat/p46-x64

Commit and push to `feat/p46-x64` after every numbered item. Record your findings
in a **new file**, `phase21-workspace/docs/phase41/LANE-C-FINDINGS.md`, not in
`SESSION_CONTEXT.md`. The session that owns main will merge and fold your findings
into §46 when you are done. If you need something from §46, read it — never write it.

`core.autocrlf` MUST stay `false`. Verify before your first commit.

---

## RULES

- Every check has an **EXPECTED** value. Observed differs → stop that item, record
  the raw observation verbatim, move to the next independent item. Do not
  theorise, do not redefine the criterion.
- Never report a gate passed without numbers.
- A security regression stops everything immediately.
- Never disable Defender, UAC, Firewall or SmartScreen.
- Long jobs print little. MSI packaging takes ~12 minutes and prints nothing —
  the 1.07 GB model dominates CAB compression. **It is not hung.**
- Watch for the five recurring defect patterns: duplicated derivation; belief
  presented as measurement; tests that never touch the real path; the measuring
  instrument itself lying; the empty-state trap.

Read `SESSION_CONTEXT.md` §20.1, §41, §42, §43 and §46 before starting.

---

## ITEM 1 — DBT-P42-011, the x64 bias. This is the most interesting thing open.

x64 Windows reads consistently high against the host's own counters:

    cpu offline    product 58.38%   host 50.91%    +7.47 points   ratio 1.147
    cpu service    product 52.31%   host 48.22%    +4.09 points   ratio 1.085
    disk latency   product 833 us   host 240 us    +593 us        ratio 3.47

Three readings, all the same direction. ARM64 Windows shows **no** such bias
(§43.5: +0.85 then −0.47 points, opposite signs, an order of magnitude smaller).
macOS was measured in §44/§45.

Same source file — `windows_impl.rs` is `#[cfg(windows)]`, not
`#[cfg(target_arch)]` — different answer per architecture. Establish why.

**1.A — reproduce it first.** The measurements above predate §45's field-boundary
fix. Re-measure on the current build before explaining anything: run the product
and `Get-Counter` in the same window, under real sustained load, at least three
rounds. Report every reading beside the host's with the delta in points and as a
ratio.

EXPECTED: the bias reproduces. If it does not, that is the finding — §45 changed
it, and you should say so and stop rather than hunt for a cause that is gone.

**1.B — establish the mechanism.** Candidate lines of enquiry, none privileged:

- the sampling window versus the host counter's window. §45 found macOS ties
  needing ~480 ms to resolve; a window mismatch is the obvious first suspect
- the denominator — logical processors, or a normalisation factor applied once
  too often or not at all
- the conversion to basis points. `DBT-P42-002` was exactly this class:
  percentages reported as basis points, hidden by another bug
- whether the host counter and the product read the *same* counter. §41.15 used
  `\Processor Information(_Total)\% Processor Time`; confirm the product's source
  is the same object, not `\Processor(_Total)\`, which differs on modern CPUs

EXPECTED: a named mechanism with `file:line`, or an explicit statement that it is
not established, naming the measurement that would settle it.

**Do not fix it in this session.** Establish the cause and record it. A cadence or
denominator change alters every reading on the qualified platform and needs its
own review.

**1.C — the disk-latency 3.47x** is the largest divergence and may be a separate
mechanism from the cpu one. Say whether it is the same cause or a different one.

---

## ITEM 2 — 4.A, the x64 release pipeline, end to end

Section 8 records it as never exercised, with a `tauri.conf.json`
`beforeBuildCommand` path defect. §41.4 reproduced that defect and worked around
it with the existing `tauri.no-before-build.json` overlay, without modifying
`tauri.conf.json`.

**Fix the defect properly this time.** Diagnose why `beforeBuildCommand` resolves
wrongly on x64, fix it in `tauri.conf.json`, and retire the overlay — or, if the
overlay is the correct long-term answer, say why and record that as the decision
rather than as a workaround.

Then exercise the pipeline:

    cargo → tauri → wix build → wix msi validate → check-msi-payload

EXPECTED: every exit code 0. `wix msi validate` output **EMPTY** — zero ICE, no
suppression. Payload check PASS. 16 file rows. Record the MSI sha256 and byte
size. For reference, §42's MSI was `6ecd1ee9…` at 1,100,148,736 bytes.

**Watch for the arch-specific packaging trap.** `Product.wxs` once hard-coded
`libomp140.aarch64.dll`, so x64 could not package at all; it is now selected by
WiX preprocessor on `$(sys.BUILDARCH)`. And `DBT-P42-012` — `vcomp140.dll` named
without its provenance where five candidates exist and the first match is wrong —
was fixed in P44 2.A with a hash check. Verify both still hold on a clean build.

---

## ITEM 3 — DBT-P41-001, the runtime DLLs

The x64 service imports `MSVCP140` and `VCRUNTIME140`, present on the dev box but
**not in the MSI payload**. On a clean machine that is a launch failure, and §41.4
recorded it as undetectable from the dev machine.

You are on a machine that can test it. Either author them into the payload, or
prove they are guaranteed present on every supported floor (build 17763 and up,
including Server SKUs) with a citation, not an assumption.

EXPECTED: a decision with evidence, not another observation.

---

## ITEM 4 — if the session still has room

`DBT-P42-013`: `cargo test` leaves temp files that confuse the uninstall survivor
sweep. §46.1 found it **worse than documented — 36 files from 4 sites**, not 11.
Fix the tests to clean up after themselves.

---

## REPORT

Write `LANE-C-FINDINGS.md` and keep it current. It must contain:

- every reading beside the host's, with deltas in points and ratios
- whether the bias reproduces after §45, stated plainly
- the mechanism with `file:line`, or "not established" and what would settle it
- the release pipeline's exit codes, MSI sha256 and byte size
- the `beforeBuildCommand` decision: fixed, or overlay-as-answer with the reason
- the `DBT-P41-001` decision with its evidence
- anything recorded rather than worked around, with its debt ID
- a one-paragraph merge note for the session that owns main
