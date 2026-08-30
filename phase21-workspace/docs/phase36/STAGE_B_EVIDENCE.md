# Phase 36 — Stage B evidence: MSI lifecycle

Entry state for the whole stage: snapshot
`P36-MSI-ALIGNED {7d0696ae-ebc7-4c67-b076-438855d175f3}` — the realigned
install from Gate A. Every cycle below compares against
`evidence/verify-A5-postinstall.json`.

## B1 — Repair — PASS

```
msiexec /f {FC8A3841-759D-B452-1864-161F84F56C03} /qn /l*v C:\AetherCore-P36\logs\b1-repair.log
B1_EXIT=0
```

Full Gate-A verification re-run afterwards
(`evidence/verify-B1-repair.json`), compared field-by-field against the
post-install state:

| property | result |
|---|---|
| `sc_qc` (LocalSystem, AUTO_START DELAYED, binary path) | identical |
| `sc_qsidtype` (UNRESTRICTED) | identical |
| pipe SDDL | identical |
| install-dir `icacls` | identical |
| `HKLM\SOFTWARE\AetherCore\InstallVersion` | identical (0.1.0) |
| `libomp140.aarch64.dll` present / `ipc_probe` absent | identical |
| installed file SET | identical |
| installed file SHA-256 for every file | identical |
| ARP entry | identical |
| `C:\ProgramData\AetherCore` (5 / state 4 / logs 1 / support-staging 0) | identical |

Verbs after repair: **8/8 RETURNED** across both actual-token contexts
(`evidence/verbs-B1-repair-{STD,ADMIN}.txt`), `doctor` returning the typed
`diagnostics.stateUnavailable` rejection.

Repair degraded nothing: not the ACLs, not the Service SID, not the pipe DACL,
not the verbs. Because Gate A realigned the package first, the repair also could
not revert the IPC fixes — it reinstated exactly the binaries built from `main`.

## B2 — Uninstall — PASS

```
msiexec /x {FC8A3841-759D-B452-1864-161F84F56C03} /qn /l*v C:\AetherCore-P36\logs\b2-uninstall.log
B2_EXIT=0
```

Sequence from the verbose log (`evidence/B2-uninstall-key.txt`): `StopServices`
-> `DeleteServices` -> `RemoveRegistryValues` -> `RemoveFiles` ->
`RemoveFolders`, all returning 1.

Survival survey: `evidence/B2-survival.txt`.

### REMOVED

| item | evidence |
|---|---|
| all seven MSI-authored payload files | INSTALLFOLDER now holds exactly one file |
| service registration | `sc query` -> `OpenService FAILED 1060: The specified service does not exist`; `HKLM\SYSTEM\CurrentControlSet\Services\AetherCoreMaintenance` absent |
| the named pipe | `\\.\pipe\` enumeration: **0** `AetherCore*` pipes — no dangling pipe |
| ARP entry | `ARP_COUNT=0` |
| `HKLM\SOFTWARE\AetherCore` (and `InstallVersion`) | key absent |
| Start Menu folder | `%ProgramData%\Microsoft\Windows\Start Menu\Programs\AetherCore` absent — `ProgramMenuComponent` authors `<RemoveFolder On="uninstall">` |

### SURVIVED

| item | why |
|---|---|
| `C:\Program Files\AetherCore` (the directory) | it is not empty — see next row |
| `C:\Program Files\AetherCore\aetherctl.exe` (3,611,136 B) | **unmanaged**: no MSI component owns it, so `RemoveFiles` never sees it. This is the direct consequence of the A5 observation. |
| INSTALLFOLDER ACLs, including the service-SID ACE | unchanged: `S-1-5-80-4285065559-…-1187574229:(OI)(CI)(RX)`, `BUILTIN\Users:(OI)(CI)(RX)`, Administrators F, SYSTEM F. The service SID now renders as a raw SID because the service name no longer resolves. |
| `C:\ProgramData\AetherCore` and all three subdirectories | `ProgramDataComponent` authors only `CreateFolder` + an HKLM `RegistryValue` KeyPath. It authors **no** `RemoveFile` and **no** `RemoveFolder`, and MSI removes a `CreateFolder` directory only when empty. |
| `state\aethercore.db`, `state\machine-mutation.lock`, `logs\service.jsonl` | runtime-created, never MSI-owned, so nothing removes them |
| `support-staging\` (empty) | same |

Raw count observation, recorded not diagnosed: `C:\ProgramData\AetherCore\state`
held 4 files before the uninstall and holds 2 after
(`aethercore.db`, `machine-mutation.lock`). The two absent entries are not
identified — the pre-uninstall surveys recorded counts, not names.

`sc showsid AetherCoreMaintenance` still returns
`S-1-5-80-4285065559-3530017622-2858480679-3751456793-1187574229` with
`STATUS: Inactive`. That is not a leftover: a service SID is a deterministic
hash of the service name, computable whether or not the service exists.
`Inactive` is the correct post-uninstall reading.

### Does user data survive uninstall, and is that the intended contract?

**Yes, it survives, and yes, it is intended.** The intent is legible in the
authoring, not inferred from behaviour: `Product.wxs` gives
`ProgramMenuComponent` an explicit `<RemoveFolder Id="RemoveAetherCoreProgramMenu" On="uninstall" />`
while giving `ProgramDataComponent` only `CreateFolder` elements and no removal
element at all. The package author removed one directory deliberately and
deliberately did not remove the other. The database, the service log and the
support-staging area are retained across uninstall/reinstall by design.

The one thing that survives *without* being intended by the package is
`aetherctl.exe`, which was never in the package to begin with.

## B3 — Clean reinstall on the emptied box — PASS

```
msiexec /i C:\AetherCore-P36\build\out\AetherCore-0.1.0-arm64.msi /qn /l*v ...
B3_EXIT=0
```

No hand steps. No `REINSTALL`, no `REINSTALLMODE`, no manual file copy, no
manual `sc create`, no ACL command. Just the package.

Full verification (`evidence/verify-B3-cleaninstall.json`) compared to the
Gate-A post-install state: **zero differing fields**.

| property | result |
|---|---|
| `sc_qc` / `sc_qsidtype` / `sc query` | identical — LocalSystem, AUTO_START (DELAYED), UNRESTRICTED, RUNNING |
| pipe SDDL | identical |
| install-dir `icacls` | identical |
| `HKLM\SOFTWARE\AetherCore\InstallVersion` | 0.1.0 |
| ARP | single entry `{FC8A3841-…}` AetherCore 0.1.0 |
| every installed file SHA-256 | identical |
| verbs | **8/8 RETURNED** (`evidence/verbs-B3-cleaninstall-{STD,ADMIN}.txt`) |

Two things to read correctly:

- `C:\ProgramData\AetherCore\state` is back to 4 files. It held 2 immediately
  after uninstall. The service recreated the other two on start, which
  identifies the two files noted in B2 as runtime-managed, not MSI-managed.
- The installed file SET matches Gate A **including `aetherctl.exe`** — but the
  MSI did not install it. It survived the uninstall (B2) and was still sitting
  in `C:\Program Files\AetherCore` when the clean install ran. **A genuinely
  bare machine would receive seven files, not eight**, and would have no
  `aetherctl.exe`; the four typed verbs would then have to be driven from a
  copy staged outside INSTALLFOLDER, which is exactly how this session drives
  them (`C:\AetherCore-P36\tools\aetherctl.exe`).

The MSI alone reproduces a working install of everything the MSI authors.
