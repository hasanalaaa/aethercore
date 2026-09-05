# P48 — finish development

Paste into Claude Code:  `Read and follow phase21-workspace/docs/phase41/P48-SHIP.md and work it end to end`

**To resume after any interruption, paste the same line again.** Read §48 in
`SESSION_CONTEXT.md`, find the first row not `DONE` / `DECIDED` /
`BLOCKED-MACHINE` / `DEFERRED-OWNER`, continue there. Nothing else is needed.

Host: the Mac at `/Users/hasanalaaa/dev/aethercore`, driving the Parallels VM.
Item 6 needs the physical Windows PC and is gated on reachability. Never
`~/Documents/ZZ-OLD-AetherCore-DO-NOT-USE`.

**One session, sequential. No parallel lanes, no delegated commits or pushes.**

---

## WHAT "FINISHED" MEANS HERE

Development is finished when the only things left require **money the owner has
chosen not to spend yet**, or **a physical machine that is not attached**.

The code-signing certificate is **explicitly deferred by the owner**. Do not
treat its absence as an open task, do not propose alternatives, do not price it,
and do not let it block any row. Record it as `DEFERRED-OWNER` once and move on.

Everything else gets closed or gets a recorded reason.

---

## RULES

- Every check has an **EXPECTED** value. Observed differs → stop that item,
  record the raw observation verbatim, move to the next independent item. Do not
  theorise, do not redefine the criterion, do not work around it.
- **The §48 row moves in the same commit as the work.** A progress table that
  lags the code is the measuring-instrument pattern, and P47 was caught by it.
- **Explicit paths when staging. Never `git add -A`.** `git status` before, not after.
- Commit and push after every item.
- A security regression stops everything immediately.
- Never disable Defender, UAC, Firewall or SmartScreen. `core.autocrlf` stays `false`.
- Fix at the type or the shared function, never per call site.

Read §42, §44, §45, §46, §47 before starting. Create **§48** as the progress table.

### The five patterns this codebase produces

Duplicated derivation · belief presented as measurement · tests that never touch
the real path · **the measuring instrument itself lying** · the empty-state trap.

The fourth one is why Item 1 exists.

---

## ITEM 1 — reconcile the debt ledger. It is currently wrong.

The ledger spans several phase tables and older rows were never updated when
later phases fixed them. Measured right now, at least these read `open` while the
code says otherwise:

    DBT-P42-012  vcomp140.dll provenance   — fixed in P44 2.A with a hash check
    DBT-P42-013  temp files under %TEMP%   — fixed in 2f1ded2
    DBT-P42-009  perProcessorBusyBp        — decided in P47 item 4
    DBT-P42-010  gpu VRAM / adapter        — VRAM implemented in P47 item 4

A ledger that reports closed work as open sends the next session to redo it, and
hides what is genuinely left. Fix that before doing anything else.

Walk **every** `DBT-*` row across all phase tables. For each, verify against the
current code and mark it: **FIXED** with the commit, **OPEN** with what remains,
**RECLASSIFIED** with why, or **SUPERSEDED** by another id. Where a row is
duplicated across tables — `DBT-P42-011` appears twice with different text —
collapse it to one authoritative row and point the others at it.

Produce one authoritative table in §48. EXPECTED: every `DBT-*` id in the file
appears exactly once as authoritative, with a status traceable to code or a commit.

**Commit the reconciled ledger before touching anything else.** It is the map for
the rest of this brief and a deliverable on its own.

---

## ITEM 2 — the icon, closed

`DBT-P36-004` is a recorded RELEASE BLOCKER because the shipped icon is a
*placeholder*. The pipeline built in P47 turns the artwork into a one-file swap,
so the blocker no longer needs to wait on a final artistic decision.

Ship `design/icon/aethercore-mark.svg` from the design worktree — the geometric
Æ ligature, monochrome `#5C7CFA` on `#030508` — as the product's mark. Run it
through the P47 pipeline, wire the full set into `tauri.conf.json` and
`Product.wxs`, and verify it reaches the MSI Icon table by sha256 as P47 did.

Then **close `DBT-P36-004`** and record, in the commit and in §48:

  the artwork is deliberate, not a placeholder; the owner may replace it at any
  time by dropping a new SVG through the pipeline; the mark is monochrome by
  design because plum is the denied-by-policy colour and spending it on an icon
  would cost it its meaning.

Note for the record, do not fix: the Æ mark as used inside the app still carries
a sky gradient outside the six roles. Raise it as its own debt id.

---

## ITEM 3 — DBT-P41-001, proven on a clean machine

P47 established this is **both-architecture, not x64-only**: 5 of 6 installed
ARM64 binaries import `VCRUNTIME140.dll`, and the service also imports
`MSVCP140.dll`. Every qualification gate in this project's history ran on a
machine that happened to have the VC++ redistributable installed. On a machine
without it, the product may not launch at all — and nothing has ever tested that.

This is the empty-state trap at its most expensive: every test passed because the
environment was favourable.

P47 decided the fix — chain `vc_redist` as a Burn prerequisite plus a `Launch`
condition. **Implement it, then prove the refusal on a machine without the runtime.**

The VM is that machine, once you make it one. This is destructive:

1. Write the **ACTION / SNAPSHOT / EXPECTED / RECOVERY** record and commit it.
2. Take a **NEW named snapshot**, `P48-PRE-REDIST-REMOVAL`. Record its UUID in
   §48. Never delete or reuse an existing snapshot; resume, never restore.
3. Enumerate first: `prlctl snapshot-list "Windows 11" -j`. As of 2026-09-03 that
   returns 8, oldest `P36-VM-QUALIFIED {a38386fa}`. Earlier briefs named two
   "forbidden" pre-install snapshots that are **not present** — §46.15 records
   that correction; do not re-inherit it.
4. Remove the VC++ redistributable, confirm it is gone by measurement.
5. Install the product. EXPECTED: the installer refuses with a clear message
   naming the missing runtime, or chains the prerequisite and succeeds. A silent
   failure or a crash on launch is a FAIL.
6. Restore the machine by installing the redistributable back — **not** by
   reverting the snapshot, so the snapshot stays as a real fallback.

Record raw output at every step. VM facts: `prlctl exec "Windows 11" cmd.exe /c
"..."` — the bare form returns EMPTY; argv caps near 16 KB, so **base64 any real
script on the Mac and decode it on the VM**. MSI packaging takes ~12 minutes and
prints nothing — not hung. Versions 0.1.9 and 0.1.10 are must-not-ship.

---

## ITEM 4 — DBT-P47-001, the light theme

`feature-layout.css` carries no `data-theme` rules, so the light theme paints
near-black text on near-black surfaces — **1.02:1 measured**, effectively
invisible. 423 colour literals remain in that file.

Migrate them to the six roles. Do not migrate blind: P47 recorded the count and
declined to move them without checking each. Work in passes, and after each pass
run the layout sweep in **both themes**, which is how the bug was found — the
sweep used to be dark-only, and that is why it passed for so long.

EXPECTED at the end: no text/background pair below **4.5:1** in either theme, and
the sweep clean at 1280 / 1024 / 960 in both languages and both themes.

Report the contrast measurements, not an adjective.

---

## ITEM 5 — whatever Item 1 says is genuinely open

Work the reconciled ledger, worst-consequence first. For each: a test committed
failing where the defect is testable, then the fix, then the measurement.

Known likely survivors, subject to Item 1's verdict:

- **`DBT-P42-005`** — macOS/Linux providers not on the `CollectedSubsystems`
  contract. P44 covered part of this; verify what actually remains rather than
  trusting the row.
- **`DBT-P45-004`** — `total == 0` still recurs at ~2% (1/50) after the bounded
  retry. Deliberately bounded, because an unbounded retry can hang the sampler.
  Either raise the ceiling with evidence, or find a tick source with a documented
  update cadence, or record it as accepted with the residual rate stated in the
  product's own terms. Do not silently leave it as "open".
- **`DBT-P42-011`** — the x64 bias. Needs the physical machine; see Item 6.

---

## ITEM 6 — the x64 items. Gated on the machine being reachable.

Check reachability first and record how you checked. If the PC is not reachable,
mark these `BLOCKED-MACHINE`, say so plainly, and finish everything else. **Do
not wait on it and do not end the session early because of it.**

**6.A** — independently verify the 4.A x64 release pipeline claim recorded in
§46.16 as a peer's claim, not a verdict: `cargo` → `tauri` → `wix build` →
`wix msi validate` → payload check. EXPECTED: all exit 0, `wix msi validate`
output **EMPTY**, payload PASS, 16 file rows, MSI sha256 and byte size recorded.

**6.B** — `DBT-P42-011`. The recorded numbers (+7.47, +4.09 points, 3.47x)
predate P45's field-boundary fix. **Re-measure on the current build before
explaining anything.** If the bias no longer reproduces, that is the finding —
say so and stop. If it does, establish the mechanism with `file:line` or state it
is not established and name the measurement that would settle it. **Do not fix
it** — a cadence or denominator change alters every reading on the qualified
platform.

**6.C** — run Item 3's clean-machine test on x64 as well, if the machine is
reachable and a snapshot or restore point can be taken first. ARM64 alone does
not prove the x64 packaging path.

---

## ITEM 7 — the release-readiness statement

When every row above is closed, write §48's final section: exactly what is
required to ship, in the owner's terms.

It must state, with evidence:

- which gates are PASS, on which architecture, against which build
- which are unproven and why
- what remains blocked on money (the certificate — one line, `DEFERRED-OWNER`)
- what remains blocked on hardware (Gate 4 driver work: the recovery media is
  created but never boot-tested and was measured detached; Windows Update offers
  the machine zero drivers, so no candidate device exists)
- the version that would ship, and that 0.1.9 / 0.1.10 are must-not-ship
- anything a first real user would hit that the project knows about and has not fixed

Be blunt in that section. It is the document the owner will decide on.

---

## THE OWNER REGISTER — list, do not attempt

- **code-signing certificate** — `DEFERRED-OWNER` by explicit decision. One line, no elaboration.
- **Gate 4** — boot-test the recovery media; nominate a printer/HID/USB-class device.
- **the UAC consent click** at the Parallels console — `PromptOnSecureDesktop` is
  `0` there, a non-default deviation that must travel with any UAC finding.
- **Windows Server runtime qualification** — needs a Server 2025 evaluation VM.
- **a production update endpoint, a production key or HSM, a dependency freeze.**
- **payment** — every MoR checked excludes Iraq for sellers in writing; Payoneer unresolved.

## REPORT — refresh at the end of every session

The §48 table with evidence on every row, numbers not adjectives; what you closed
and what you measured after; anything recorded rather than worked around with its
id; and the single next action in one sentence.
