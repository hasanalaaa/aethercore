# P42 — fix DBT-P41-002, then prove the full lifecycle

Paste into Claude Code:  `Read and follow phase21-workspace/docs/phase41/P42-FIX-AND-LIFECYCLE.md and work it end to end`

**Start this session from an already-elevated PowerShell** (right-click →
Run as administrator, then launch the tool). §41.9 records that one action as the
thing that unblocks everything; without it every admin step needs its own UAC
prompt and the run stalls.

---

## HOW TO WORK THIS FILE

The failure mode here is not running out of thinking — it is running out of
session. A P41 session hit its usage limit mid-gate. So:

- **Write every finding to disk as you get it.** `SESSION_CONTEXT.md` §42 is the
  record. Never hold a measurement only in your reply.
- **Commit and push after every numbered item**, not every part. If this session
  dies, the next one reads §42 and resumes with zero loss.
- Long jobs (cargo, MSI packaging ~12 min, install) print little. **Not hung.**
- Every check has an EXPECTED value. **Observed differs: stop that item, record
  the raw observation verbatim, move to the next independent item.** Do not
  theorise. Do not redefine the criterion.
- A security regression stops everything immediately.
- Never disable Defender, UAC, Firewall or SmartScreen.
- `core.autocrlf` MUST stay false — this project hashes files.

Read `phase21-workspace/docs/phase36/SESSION_CONTEXT.md` §20.1 (the diagnosis),
§41.15 and §41.16 (the real-silicon measurements) before writing any code.

---

## PART 1 — DBT-P41-002

Diagnosed in `8612d7b`, confirmed on real x86_64 in §41.15. Not a mystery any
more. What is known, and must not be re-derived:

**The root cause is at the type.** `PerfPlatform::sample` returns `PerfSnapshot`,
not `Result`, and `PerfSnapshot` derives `Default`. So an all-zero payload with an
empty fault list is a legal, type-checking return. Nine hand-written availability
rules exist to compensate for that missing contract; gpu's is the only one that
fires, and §20.1.6 records that it fires **by accident, not by design**.

Two concrete mechanisms underneath it:

1. `read_u64` passes an 8-byte `i64` where `PdhGetFormattedCounterValue` writes a
   16-byte `PDH_FMT_COUNTERVALUE`. `largeValue` sits at offset 8, so the code reads
   `CStatus` — and `PDH_CSTATUS_VALID_DATA == 0` — and reports **a status code as a
   measurement**. §41.15 confirmed `CStatus` is `0` across 6 samples. It is also an
   **8-byte stack overflow on every counter read**, present since Phase 20. No
   crash observed in ~3 h uptime, but "not observed" is not "not present".
2. storage's wildcard pattern is built from a dropped temporary and, being a raw
   string, is never NUL-terminated. The failure exits through one of the two paths
   after `PdhExpandWildCardPathW`, neither of which pushes a fault.

And the contradiction §41.16 found: `capabilities` reports **16/16 `native`**
while storage returns `[]` and gpu declares a fault. Three deciders, disagreeing.

**The counters are present.** §41.15 measured `PhysicalDisk` with 4 live instances,
`GPU Engine` with 568, and CPU at 7.01% — through PDH, on this box, while the
product reported nothing. The data was always there.

### 1.A — write the failing tests FIRST, and commit them failing

This project's practice: the LocalSystem authorization fix committed its
regression tests failing before the fix existed. Do the same.

§20.1 records that **no test anywhere constructs `WindowsPerfPlatform`**, and both
real-path tests assert upper bounds only — so all-zero passes them. That is the
§17.7 pattern that let a total IPC failure survive to Phase 36.

Write tests that fail today:

- a test that constructs the real Windows platform and asserts CPU busy is
  **non-zero on a machine doing work**. An upper-bound assertion is exactly the
  hole that hid this; assert a lower bound.
- a test that asserts `read_u64` reads `largeValue`, not `CStatus` — give it a
  buffer whose offset-0 and offset-8 values differ so the two are distinguishable.
- a test that asserts a collector cannot return success with an empty payload and
  no fault.
- a test that asserts `capabilities` cannot report `native` for a subsystem whose
  collector declared a fault or returned empty.

Commit them failing, with their output in the commit message.

### 1.B — fix at the type, not with a tenth rule

The lazy fix here is also the correct one: **one contract, instead of nine
compensating rules.** Make it impossible to return "measured" without a
measurement — the exact shape is your call, but justify it:

- `sample` returning `Result`, or
- a payload type that cannot be constructed empty (drop the `Default` derive), or
- a reading type that carries `Measured(v)` / `Unavailable(reason)` and has no
  third state.

Then fix both mechanisms:

- `read_u64`: pass the correctly-sized `PDH_FMT_COUNTERVALUE` and read
  `largeValue` at offset 8. This closes the stack overflow. Treat that as a
  memory-safety fix, not a cosmetic one.
- storage: bind the pattern so it outlives the call, and NUL-terminate it. Every
  early return must push a fault.

Delete the availability rules the new contract makes redundant. Report how many
of the nine survive and why.

**Do not touch the ARM64 path's behaviour.** The ARM64 pipeline is qualified and
shipping; if a change affects it, say so explicitly and show the diff.

### 1.C — prove it on this machine

    cargo test    (the 1.A tests must now pass)
    cargo build   (release, x64)

Then run `telemetry-once` and `perf snapshot` against the freshly built binary
while the machine is doing work.

EXPECTED: CPU busy is **non-zero and plausible** — §41.15 measured 7.01% through
PDH directly, so the product must now report the same order of magnitude.
storage returns real instances (4 were live) or an explicit fault with a reason.
`capabilities` no longer claims `native` for a subsystem that reported nothing.

If any subsystem still reports zero, that is a **finding, not a failure to hide**:
record the raw output and which of the three mechanisms it belongs to.

Commit and push Part 1 with its numbers before starting Part 2.

---

## PART 2 — GATE 5: the full lifecycle, zero survivors

§41.8 records Gate 5 as NOT STARTED and no longer blocked. The uninstall/survivor
sweep has **never been proven on this machine** — only on the VM.

This part is destructive: it removes the installed product. Write the
**ACTION / SNAPSHOT / EXPECTED / RECOVERY** record and commit it first.

Recovery context, so the record is honest: the §41.13 disk image exists and is
verified (521.45 GB, Bare Metal Recovery, 19 files), but §41.12 0f.7 records that
the machine was **already installed** when it was imaged — it is not a pristine-state
image. The restore point from Gate 0 is the pre-install target. State both.

### 2.A — rebuild the MSI with the Part 1 fix

    cargo → tauri → wix build → wix msi validate → payload check

EXPECTED: every exit code 0, `wix msi validate` output **EMPTY** (zero ICE, no
suppression), payload check PASS, 16 file rows. Record the new MSI sha256 and byte
size. The previous MSI was `d18d89db…` at 1,100,140,544 bytes.

Note the beforeBuildCommand defect is recorded — use the existing
`tauri.no-before-build.json` overlay and do NOT modify `tauri.conf.json`.

### 2.B — uninstall, and sweep for survivors

Uninstall with a verbose log. Then run the fourteen-check survivor sweep that
Phase 36 established — files, directories, service registration, registry keys,
scheduled tasks, event log sources, firewall rules, the named pipe, ProgramData,
user profile traces, start menu entries, uninstall entries, drivers, and the
install directory itself.

EXPECTED: **zero survivors on all fourteen.**

Any survivor is a finding. Record it with its exact path or key — do not delete it
by hand and call the gate passed. The gate measures what the uninstaller does, not
what you can clean up afterwards.

### 2.C — reinstall from the new MSI and re-prove every Gate 2 property

Install the Part 2.A MSI, then verify — do not assume any of these carried over:

- all 16 files present, hashes matching the built payload
- service LocalSystem, AUTO_START, RUNNING
- Service SID **UNRESTRICTED**
- install-dir ACLs protected, Users read-execute only
- **zero dev binaries** in the install directory
- the four verbs round-trip, with timings
- **`engineLabel=localModel`**, proven against the RUNNING SERVICE

**Pipe DACL** must be byte-identical to:

    O:<service SID> G:SY D:P(A;;FA;;;<service SID>)(A;;FR;;;AU)(A;;DC;;;AU)

Some APIs render the AU pair as `(A;;0x12008b;;;AU)`. **That is the SAME DACL:**
`FR|DC = 0x120089|0x2 = 0x12008b`. Do NOT report that as drift.

Read it with:

    [System.IO.File]::Open('\\.\pipe\AetherCore.Maintenance.v7',
      [IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::ReadWrite)

then `.GetAccessControl().Sddl`.

### 2.D — the Part 1 fix, live under the installed service

Re-run the 1.C measurements against the **installed service**, not the dev binary.
§41.15 found the service path shows identical zeros to the offline path, so the fix
must hold on both.

EXPECTED: the same real numbers 1.C produced.

Also re-check the `doctor` exit 5 anomaly §41 recorded at Gate 2 — say whether it
persists, changed, or resolved.

Commit and push Gate 5 with its numbers.

---

## HARD STOP — Gate 4 does not run in this session

Driver work stays stopped. §41.17 lists what it needs from the owner:

1. **boot-test the recovery media** — creating it is not the same as proving it
   boots this machine. Until it is boot-tested it is not a recovery plan.
2. **a driver to install** — Windows Update offers **zero** on this box (§41.16 3c).

Do not start it, and do not treat its absence as a gap in this session.

---

## REPORT

- the gate table with evidence on every line, numbers not adjectives
- how many of the nine availability rules survived the new contract, and why
- whether the stack overflow fix changed any ARM64 behaviour
- anything recorded rather than worked around, with its debt ID
- what Gate 4 still needs from the owner
