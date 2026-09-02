# P41 — full run: Gate 0f verification through Gate 3

Paste into Claude Code:  `Read and follow phase21-workspace/docs/phase41/P41-FULL-RUN.md and work it end to end`

Supersedes `P41-ELEVATED-SESSION.md` for anything not already committed PASS.

---

## HOW TO WORK THIS FILE

This is one long run. The failure mode is not running out of thinking — it is
running out of session. A previous session on this machine hit its usage limit
mid-gate. So:

- **Write every finding to disk as you get it.** Never hold a measurement only in
  your reply. `SESSION_CONTEXT.md` section 41 is the record.
- **Commit and push after every numbered item**, not every gate. If this session
  dies, the next one reads section 41 and resumes with zero loss. That is what
  actually preserves knowledge — not session length.
- Keep a running progress table in §41.8 with each item marked
  `PASS` / `FAIL` / `BLOCKED` / `NOT STARTED` and the evidence line.
- Long jobs (imaging, MSI install) print little and take many minutes. **They are
  not hung.** Do not kill them. Do read-only work while they run.

Every check below has an EXPECTED value. **Observed differs: stop that item,
record the raw observation verbatim, move to the next independent item.** Do not
theorise. Do not redefine the criterion. Do not work around it.

A security regression stops everything immediately.

Never disable Defender, UAC, Firewall or SmartScreen.

Repo `C:\dev\aethercore`. `core.autocrlf` MUST stay false — this project hashes
files and CRLF conversion breaks every SHA check. Verify it before your first commit.

Confirm elevation and record it:

    ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(544)

EXPECTED `True`. If False, stop — do not spawn RunAs children.

---

## GATE 0f — VERIFY THE IMAGE. Start here, before anything else.

An image job was started to `D:` (HIKSEMI USB). Since then the owner formatted a
flash drive as NTFS and may or may not have targeted the same device. **Do not
assume the image survived.**

**0f.A — establish what is actually attached now.**

    Get-Disk | Format-Table Number, FriendlyName, Size, PartitionStyle -AutoSize
    Get-Volume | Format-Table DriveLetter, FileSystemLabel, FileSystem, Size, SizeRemaining -AutoSize

Report every disk and volume. Identify by FriendlyName and size, not by letter —
letters move. EXPECTED: disk 0 is the NVMe system disk; at least one USB disk is
present.

**0f.B — does the image exist?**

    wbadmin get versions -backupTarget:D:
    Get-ChildItem D:\WindowsImageBackup -Recurse -ErrorAction SilentlyContinue |
      Measure-Object -Property Length -Sum

EXPECTED: at least one version listed with today's date, and a `WindowsImageBackup`
folder totalling a plausible fraction of the 533.38 GB used on `C:`.

A `wbadmin` exit code of 0 is NOT proof. The version listing is the proof. If the
listing is empty, Gate 0f is FAIL no matter what any earlier message said.

**0f.C — if the image is missing or incomplete**, determine which of these it is
and record it plainly:

- the job never finished (still running, or interrupted by the reboot/format)
- the target volume was reformatted, destroying it
- it completed and is intact

Then re-run it against a target you have enumerated with `-Force` first:

    wbadmin start backup -backupTarget:<LETTER>: -include:C: -allCritical -quiet

Do NOT format anything. Do NOT delete anything from any drive. If the only
available target holds owner data, STOP and report.

Re-verify with 0f.B afterwards. Commit Gate 0f with its numbers before continuing.

**0f.D — recovery media, recorded not created.**

    reagentc /info

EXPECTED `Windows RE status: Enabled`.

Record explicitly in §41: WinRE lives on disk 0 partition 4. It recovers a machine
that still boots. It does NOT help a machine that will not boot — which is exactly
the Stage 4 driver failure mode. Bootable recovery media is an owner action and is
a precondition of Gate 4, not of Gates 2 or 3. Flag it; do not create it.

---

## GATE 2 — the first install this machine has ever had

Only after 0f is committed PASS.

    powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\aethercore\phase21-workspace\scripts\p41\stage2.ps1

The script carries its own restore-point preflight and the full Stage 2 list:
install with `/l*v`, 16 files plus hashes, service state / account / SID, pipe DACL,
install-dir ACLs, dev-binary sweep, four verbs, engineLabel.

Read its output yourself. Do not trust its summary line — verify the individual
observations against these:

**Pipe DACL** must be byte-identical to:

    O:<service SID> G:SY D:P(A;;FA;;;<service SID>)(A;;FR;;;AU)(A;;DC;;;AU)

Some APIs render the AU pair as `(A;;0x12008b;;;AU)`. **That is the SAME DACL:**
`FR|DC = 0x120089|0x2 = 0x12008b`. Do NOT report that as drift.

Reading the DACL — `Get-Acl` and a plain FileStream both fail:

    [System.IO.File]::Open('\\.\pipe\AetherCore.Maintenance.v7',
      [IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::ReadWrite)

then `.GetAccessControl().Sddl`.

**Service SID** must be UNRESTRICTED. **Install-dir ACLs** must be protected.

**engineLabel** must report `localModel`, not `ruleFallback`. Before Phase 36 every
installed copy silently ran ruleFallback because the MSI never authored the
embedded model. `aetherctl self-check --load-model` returning exit 7
(`embeddedModelLoaderNotCompiled`) is correct by design — aetherctl has
`default = []` with the loader opt-in — and is **NOT** the proof. Prove it against
the RUNNING SERVICE.

**All 16 files** must be present with matching hashes. **Zero dev binaries** in the
install directory.

Commit and push Gate 2 with its numbers. Never report a gate passed without numbers.

---

## GATE 3 — real hardware, and the DBT-P41-002 measurement

**3.A — the deciding check. Run this FIRST; it invalidates everything after it.**

A Mac session diagnosed DBT-P41-002 in commit `8612d7b`. Read §20.1 before you
measure anything.

`perProcessorBusyBp` is never written on Windows and must serialise as `[]`.

EXPECTED: `[]`

If the installed binary shows **22 zeros** instead, it is NOT built from `e91f675`.
Stop Gate 3 measurement immediately, report that, and do not measure anything else
against §20.1 — the diagnosis would need re-basing against whatever binary is
actually installed. Everything downstream of a wrong baseline is worthless.

**3.B — re-measure with the service running.**

The original all-zero observation was taken with **no service running**. Re-run
`telemetry-once` against the RUNNING SERVICE and record raw output verbatim for
cpu, storage and gpu.

EXPECTED per §20.1: cpu still reports zero — because `read_u64` passes an 8-byte
`i64` where `PdhGetFormattedCounterValue` writes a 16-byte `PDH_FMT_COUNTERVALUE`,
so the code reads `CStatus` (`PDH_CSTATUS_VALID_DATA == 0`) and reports it as basis
points. storage still empty, exiting through one of four early returns that push no
fault. gpu still correctly declares `Unavailable` WITH a reason.

If the service-backed answer DIFFERS from the offline one, that is a new fact and
more interesting than the confirmation. Record it in full.

**Do not fix any of this.** Diagnosis is committed; the fix is a separate session
with its own review. Your job here is measurement on real silicon.

**3.C — the remaining section 41.8 Stage 3 items** that were blocked behind Gate 2.
Work them in order, committing each.

Commit and push Gate 3 with its numbers.

---

## HARD STOP — do not start Gate 4

Driver work does not run in this session, and not unattended.

Reasons, recorded so no future session re-litigates them:

1. Its failure mode is an unbootable machine.
2. Bootable recovery media does not exist yet. WinRE on disk 0 does not cover it.
3. Driver acquisition and install are hardware-gated and separately authorised.

When Gate 3 is committed, write a short readiness note in §41 listing exactly what
Gate 4 needs from the owner, and stop. Report:

- the gate table with evidence for every line
- what you measured that no VM could have measured
- anything you recorded rather than worked around, with its debt ID
