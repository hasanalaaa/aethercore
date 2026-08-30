# Official Windows Mechanism Authority

Phase 19 intentionally uses supported Windows mechanisms and rejects generic repair-script folklore.

Primary authority families used for source design:

- Microsoft DISM API: `DismInitialize`, `DismOpenSession`, `DismCheckImageHealth`, `DismCloseSession`, `DismShutdown` and the documented online-image health states.
- DISM servicing: fixed `RestoreHealth` path only after current health proves repair is appropriate.
- System File Checker: supported `/verifyonly` and `/scannow` operations; no locale-sensitive console string is resolution authority.
- Windows Update Agent: `IUpdateSession`, `IUpdateSearcher`, online search and operation result/HRESULT evidence.
- Service Control Manager: fixed `wuauserv` query/start for a diagnosis-scoped update repair; no renderer-selected service authority.
- CHKDSK: online `/scan` as a filesystem diagnostic; no generic destructive repair switches.
- REAgentC/WinRE: recovery state remains conservative until live structured/qualified integration is proven.

Where no robust source-level provider was defensible, the model records Unknown/guidance/capability debt instead of copying broad internet commands.
