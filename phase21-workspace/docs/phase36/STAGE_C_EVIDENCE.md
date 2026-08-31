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
