# P41 — elevated Windows session

Paste into Claude Code:  `Read and follow phase21-workspace/docs/phase41/P41-ELEVATED-SESSION.md`

---

Read `phase21-workspace/docs/phase36/SESSION_CONTEXT.md` first, then the Phase 41
progress table at section 41.8, then §20.1 (the DBT-P41-002 diagnosis).

Repo: `C:\dev\aethercore`. `core.autocrlf` MUST stay false — this project hashes
files and CRLF conversion breaks every SHA check.

Confirm elevation first and record it:

    ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(544)

EXPECTED: `True`. If False, stop immediately and say so — do not spawn RunAs children.

An external drive is now connected. This unblocks the full disk image that
section 41 recorded as impossible. Do it BEFORE the install, not before Stage 4.

---

## GATE 0f — full system image, while the machine is still pristine

Rationale, so you do not reorder this: nothing has ever been installed on this
machine. That is the single most valuable state to capture and it cannot be
recreated after the first install. The "uninstall leaves zero traces" property was
proven on the VM, NOT on this hardware, so it is not evidence here. A restore
point covers drivers and registry; it does not recover a machine that will not boot.

Write the ACTION / SNAPSHOT / EXPECTED / RECOVERY record and commit it first.

**0f.1 — identify the target. Do NOT assume a drive letter.**

    Get-Volume | Format-Table DriveLetter, FileSystemLabel, FileSystem, Size, SizeRemaining -AutoSize

Report every volume. Print two numbers explicitly: used space on `C:`, and free
space on the candidate target.

EXPECTED: exactly one non-system volume, NTFS, with free space greater than used
space on `C:`.

- Not NTFS: STOP and report. Do NOT format anything — that is the owner's decision.
- Already contains data: enumerate the top level, report it, and STOP for
  confirmation. Never write to a drive whose contents you have not enumerated.

**0f.2 — confirm the tool exists before relying on it.**

    wbadmin /?

EXPECTED: wbadmin responds with its command list. If unavailable on this SKU, stop
and report — do not substitute a third-party imaging tool without asking.

**0f.3 — create the image.**

    wbadmin start backup -backupTarget:<LETTER>: -include:C: -allCritical -quiet

Takes a long time and prints little. It is NOT hung. Do not kill it. Record the
exit code. EXPECTED: exit 0.

**0f.4 — VERIFY. A success message is not proof; the listing is.**

    wbadmin get versions -backupTarget:<LETTER>:
    Get-ChildItem <LETTER>:\WindowsImageBackup -Recurse -ErrorAction SilentlyContinue |
      Measure-Object -Property Length -Sum

EXPECTED: at least one version listed with today's date, and a `WindowsImageBackup`
folder whose total size is a plausible fraction of `C:` used. If the version list
is empty, Gate 0f FAILS regardless of the exit code.

**0f.5 — record that bare-metal restore needs bootable media.**

    reagentc /info

EXPECTED: `Windows RE status: Enabled`. Report the raw output either way. Do not
create recovery media — flag it for the owner if WinRE is disabled.

Commit and push Gate 0f with its numbers. Never report a gate passed without numbers.

---

## GATE 2 — first install

Only after 0f is committed PASS:

    powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\aethercore\phase21-workspace\scripts\p41\stage2.ps1

That script contains its own restore-point preflight and the full Stage 2 list:
install with `/l*v`, 16 files plus hashes, service state / account / SID, the pipe
DACL, install-dir ACLs, the dev-binary sweep, four verbs, and engineLabel.

Two things to hold against it specifically:

**The pipe DACL** must be byte-identical to:

    O:<service SID> G:SY D:P(A;;FA;;;<service SID>)(A;;FR;;;AU)(A;;DC;;;AU)

Some APIs render the AU pair as `(A;;0x12008b;;;AU)`. That is the SAME DACL:
`FR|DC = 0x120089|0x2 = 0x12008b`. Do NOT report that as drift.

**engineLabel** must report `localModel`, not `ruleFallback`. This property was
silently broken in every installed copy before Phase 36 because the MSI never
authored the embedded model. `aetherctl self-check --load-model` returning exit 7
is correct by design and is NOT the proof — prove it against the RUNNING SERVICE.

Commit and push Gate 2 with its numbers.

---

## GATE 3 — real hardware, and the DBT-P41-002 measurement

Resume the section 41.8 items that were blocked behind Gate 2.

A Mac session has already diagnosed DBT-P41-002 in commit `8612d7b`. Read §20.1
before you measure. Summary: the zero is not a measurement — `read_u64` passes an
8-byte `i64` where `PdhGetFormattedCounterValue` writes a 16-byte
`PDH_FMT_COUNTERVALUE`, so the code reads `CStatus` (`PDH_CSTATUS_VALID_DATA == 0`)
and reports it as basis points. It is also an 8-byte stack overflow on every
counter read, present since Phase 20.

**Your job is measurement on real silicon, not root cause. Do not fix it.**

Run the decisive check recorded in §20.1.3(b) FIRST, because it invalidates
everything else if it fails:

`perProcessorBusyBp` is never written on Windows and must serialise as `[]`.

EXPECTED: `[]`.

If the box shows 22 zeros instead, the binary under test is NOT built from
`e91f675` — stop, report that, and do not measure anything else against §20.1,
because the diagnosis would need re-basing against whatever binary is actually
installed.

Then re-run `telemetry-once` against the RUNNING SERVICE — the original all-zero
observation was taken with no service running — and record whether cpu and storage
change. Record raw output verbatim.

---

## RULES FOR EVERY ITEM

Every check has an EXPECTED value. Observed differs: stop that item, record the raw
observation verbatim, move to the next independent item. Do not theorise, do not
redefine the criterion.

A security regression stops everything immediately.

Never disable Defender, UAC, Firewall or SmartScreen.

Commit and push after every gate, never only at the end — validated work on this
project has already had to be recovered by hand once.

STOP before Gate 4 (drivers). Driver work is a separate session with its own
authorisation, and the disk image must be verified before it starts.
