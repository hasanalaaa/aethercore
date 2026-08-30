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
