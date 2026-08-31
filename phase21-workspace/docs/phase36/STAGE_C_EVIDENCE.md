# Phase 36 — Stage C evidence: rollback and recovery fault injection

**No new snapshot could be taken for any injection in this stage.** The
Parallels host volume is at 100% capacity (6.8 GiB free) and refuses snapshot
creation; see `SESSION_CONTEXT.md` §13. Freeing host space is a human action —
deleting snapshots is forbidden by this brief and the large directories on the
host are the user's data. Each injection therefore names an existing snapshot
plus a recorded automated replay as its recovery, and every injection below was
in fact recovered without needing a restore.

## C3 — a required file is missing at service start — RECORDED, RECOVERED

Run first, because it is the only injection that the product's own recovery
contract is designed to repair.

Injection: stop the service, rename `libomp140.aarch64.dll` to
`libomp140.aarch64.dll.missing`, attempt `sc start`.

Raw outcome (`evidence/C3-injection.txt`):

```
DLL_PRESENT_AFTER_INJECT=False
SC_START_OUTPUT=[SC] StartService FAILED 1053: The service did not respond to
                the start or control request in a timely fashion.
SC_START_ELAPSED_MS=24
SC_QUERY=... STATE : 1 STOPPED ...
PIPE_COUNT=0
```

System event log:

```
7000 Error : The AetherCore Maintenance Service service failed to start due to
             the following error: The service did not respond to the start or
             control request in a timely fashion.
7009 Error : A timeout was reached (30000 milliseconds) while waiting for the
             AetherCore Maintenance Service service to connect.
```

| question | answer |
|---|---|
| does the service start? | no — fails 1053, stays STOPPED |
| is the service orphaned? | no — registration intact, state STOPPED, not a zombie process |
| is the pipe left dangling? | **no** — `PIPE_COUNT=0` |
| is the machine usable? | yes — explorer running, ARP intact `{84140FFD-…} 0.1.1`, `C:\Windows\Installer` 206 files |
| does it fail closed? | yes — no partial service, no half-open endpoint |

**Recovery via the product's own contract.** `Product.wxs` deliberately does not
author `ARPNOREPAIR` because "Windows Installer repair is intentionally part of
the recovery contract". Exercised:

```
msiexec /f {84140FFD-5CBC-175D-928D-E493493F5F51} /qn
C3_REPAIR_EXIT=0
DLL_RESTORED=True
STATE : 4 RUNNING
```

Full verification after recovery (`evidence/verify-C3-recovered.json`): pipe
SDDL identical, libomp present, no `ipc_probe`, 8 files, and **8/8 verbs
RETURNED**. The repair contract works exactly as the authoring claims.

One artifact the repair does not clean up: the renamed
`libomp140.aarch64.dll.missing` remained in INSTALLFOLDER after the repair
restored the real DLL. That file is this session's injection artifact, not
product state, and it was removed by hand.

## C1 — install interrupted mid-transaction — RECORDED, RECOVERED

### Attempt 1 — the injection did not land

Trigger: first appearance of the string `InstallFiles` in the verbose log.
Result: fired at 1901 ms, killed one msiexec PID, and **the install completed
anyway** — `MainEngineThread is returning 0`, all files present, service
RUNNING, ARP registered. Recorded as a failed injection, not as a finding: at
that point the log only shows the *sequencing* of `InstallFiles`; the deferred
copy runs later, in the `msiexec /V` server. Raw record:
`evidence/C1-injection.txt`.

### Attempt 2 — a genuine mid-copy kill

Trigger: first appearance of `Executing op: FileCopy` — the deferred operation
that is actually writing bytes. Both msiexec processes were killed.

```
TRIGGER_AT_MS=323
FILECOPY_OPS_SEEN=2 LAST=wlnmgdsb.exe|aethercore-desktop.exe
MSIEXEC_PIDS=7796,8920
KILLED_PID=7796
KILLED_PID=8920
MSIEXEC_STILL_RUNNING=0
```

Post-kill survey (`evidence/C1b-injection.txt`):

| question | answer |
|---|---|
| **does Windows Installer roll back cleanly?** | **No — no rollback ran at all.** `ROLLBACK_SCRIPTS=` (empty), `INPROGRESS_KEY=False`. The process that would have executed the rollback script is the process that was killed. |
| what is left on disk? | a **partial payload**: `aethercore-consent-broker.exe`, `aethercore-desktop.exe`, `aethercore-install-hardener.exe`, `aethercore-maintenance-service.exe` — 4 of 7 files. Missing: `aethercore-update-broker.exe`, `libomp140.aarch64.dll`, `update-trust.json`. |
| is the service orphaned? | **no** — `OpenService FAILED 1060`, `SERVICE_REGKEY=False`. `InstallServices` had not run yet. |
| is the pipe left dangling? | **no** — `PIPE_COUNT=0` |
| is the product registered? | **no** — `ARP=` empty, `HKLM_AETHERCORE=False`. The machine does not believe AetherCore is installed. |
| is `C:\Windows\Installer` consistent? | **almost** — 208 files vs 206 before, and one orphaned `MSICD74.tmp` remains. No in-progress key, no stranded rollback script. |
| is the machine usable? | yes — explorer running, no stuck msiexec, subsequent msiexec transactions work |

The honest summary: a hard kill of the installer engine leaves **orphaned files
with no registration and no rollback**. The failure is not silent-corrupt — the
system's own view (ARP, service database, pipe namespace) is consistent and says
"not installed" — but `C:\Program Files\AetherCore` is left holding four stale
binaries that nothing owns and nothing will clean up.

### The aftermath produced a second, unplanned finding

The first recovery attempt — a plain `msiexec /i` of the same v1 package onto
the box still holding those four orphaned files — **failed and rolled back**:

```
Executing op: ActionStart(Name=HardenInstalledSecurity,,)
Executing op: CustomActionSchedule(Action=HardenInstalledSecurity,ActionType=11282,
    Source=C:\Program Files\AetherCore\aethercore-install-hardener.exe,Target=**********,)
Executing op: ActionStart(Name=StartServices,Description=Starting services Service: [1],)
Product: AetherCore -- Error 1920. Service 'AetherCore Maintenance Service'
    (AetherCoreMaintenance) failed to start.
Executing op: RollbackInfo(,RollbackAction=Rollback,...)
Installation success or error status: 1603.
MainEngineThread is returning 1603
```

Observed vs expected: expected exit 0 (the same command passed in B3), observed
**1603** with `Error 1920` at `StartServices`. Per this brief's stop rule the
cause is **not** diagnosed here. What is proven is the rollback behaviour, and
it is good:

| after the 1603 rollback | state |
|---|---|
| `C:\Program Files\AetherCore` | back to `aetherctl.exe` only — the four orphans **and** every newly copied file removed |
| ARP | empty |
| service | `OpenService FAILED 1060` — removed |
| machine | usable |

Windows Installer rolled back a failed transaction completely and correctly,
including cleaning up the orphans the earlier killed transaction had left.

**Recovery completed.** Re-running the identical `msiexec /i` on the
now-genuinely-clean box: exit **0**, service **RUNNING**, all eight files
present. No snapshot restore was needed.

## C2 — the service fails to start during install — RECORDED, ROLLED BACK CLEANLY

Injection instrument: an **injection package**, not a machine mutation. No ACL
was touched, no Service SID changed, no pipe descriptor edited.
`AetherCore-0.9.2-arm64.msi`, ProductCode
`{4944D099-6844-678B-B98B-77B34955CD8A}`, built by the recorded recipe from a
copy of the real payload in which `libomp140.aarch64.dll` was replaced with
599,504 bytes of deterministic pseudorandom data. The service executable then
cannot resolve its OpenMP runtime import and cannot reach RUNNING.

Installed onto a bare box. Result (`evidence/C2-injection.txt`):

```
MSIEXEC_EXIT=1603
ELAPSED_MS=32206
...
Executing op: CustomActionSchedule(Action=HardenInstalledSecurity,ActionType=11282,...)
Product: AetherCore -- Error 1920. Service 'AetherCore Maintenance Service'
    (AetherCoreMaintenance) failed to start.
Executing op: RollbackInfo(,RollbackAction=Rollback,...)
Installation success or error status: 1603.
MainEngineThread is returning 1603
ROLLBACK_OPS_IN_LOG=20
```

Note the elapsed time: 32 s, i.e. the installer waited out the full
`ServiceControl Wait="yes"` window before declaring 1920.

| question | before | after |
|---|---|---|
| INSTALLFOLDER contents | `aetherctl.exe` only | `aetherctl.exe` only — **every installed file removed** |
| service | `OpenService FAILED 1060` | `OpenService FAILED 1060` — **not orphaned** |
| service registry key | False | False |
| named pipe | 0 | **0 — not dangling** |
| ARP | empty | empty — the failed product is **not** registered |
| `HKLM\SOFTWARE\AetherCore` | False | False |
| Start Menu folder | False | False |
| `C:\Windows\Installer` file count | 206 | **206 — consistent, byte-for-byte the same count** |
| in-progress key | False | False |
| machine usable | yes | yes |

**Windows Installer rolled back completely and cleanly.** The box is
indistinguishable from its pre-injection state.

## C4 — rollback triggered by a failing custom action — RECORDED, ROLLED BACK CLEANLY

Injection instrument: `AetherCore-0.9.4-arm64.msi`, ProductCode
`{C2FB7D6F-7B7F-315C-E5D0-24FE6C901809}`, built from a payload copy in which
`aethercore-install-hardener.exe` was replaced by `C:\Windows\System32\whoami.exe`
— a genuine signed ARM64 PE that rejects the literal argument `apply` and exits
non-zero. `Product.wxs` authors the custom action as

```xml
<CustomAction Id="HardenInstalledSecurity" FileRef="InstallHardenerExe"
              ExeCommand="apply" Execute="deferred" Impersonate="no"
              Return="check" HideTarget="yes" />
```

so a non-zero exit must abort the transaction.

Result (`evidence/C4-injection.txt`):

```
MSIEXEC_EXIT=1603
ELAPSED_MS=2029
...
Executing op: CustomActionSchedule(Action=HardenInstalledSecurity,ActionType=11282,
    Source=C:\Program Files\AetherCore\aethercore-install-hardener.exe,Target=**********,)
Executing op: RollbackInfo(,RollbackAction=Rollback,...)
Installation success or error status: 1603.
MainEngineThread is returning 1603
ROLLBACK_OPS_IN_LOG=20
```

2.0 s, versus C2's 32 s — the transaction aborted at the custom action, before
`StartServices` was ever reached, exactly as the `After="InstallServices"`
sequencing and `Return="check"` specify. `Target=**********` confirms
`HideTarget="yes"` is in force.

Post-rollback survey is identical to the pre-injection survey in every single
field: `aetherctl.exe` only, no service, `PIPE_COUNT=0`, no ARP entry, no HKLM
key, no Start Menu folder, `C:\Windows\Installer` at 206 files, no in-progress
key, machine usable, no msiexec left running.

**`Return="check"` does what it claims: a failing custom action aborts the
install and Windows Installer restores the machine completely.**

## Recovery after Stage C

Plain `msiexec /i AetherCore-0.1.0-arm64.msi /qn` — exit 0, service RUNNING,
all files present. No snapshot restore was needed for any of the four
injections.

## One artifact that survives every rollback

`C:\Windows\Installer\MSICD74.tmp` appeared during the C1 hard-kill and is
still present. It is not removed by any subsequent successful or rolled-back
transaction, and it is the only inconsistency this stage found in
`C:\Windows\Installer`. Recorded, not remediated — cleaning `C:\Windows\Installer`
by hand is outside what this brief authorizes.

## GATE C — PASS

Each of the four injections has a recorded outcome and the machine was
recovered every time.

| injection | rolls back cleanly? | machine usable? | service orphaned? | pipe dangling? | `C:\Windows\Installer` consistent? |
|---|---|---|---|---|---|
| C1 install killed mid-copy | **no rollback runs at all** — the executor was the process killed | yes | no | no | one orphaned `.tmp`, otherwise yes |
| C2 service fails to start | **yes, completely** | yes | no | no | yes (206 -> 206) |
| C3 required file missing at start | n/a (post-install fault) — the authored `msiexec /f` repair contract restores it | yes | no | no | yes |
| C4 custom action fails | **yes, completely** | yes | no | no | yes (206 -> 206) |

The one case Windows Installer cannot protect against is its own engine being
killed: it leaves orphaned files with no registration, which the next failed
transaction happens to clean up but which nothing is guaranteed to.
