# Phase 36 — Stage E: seal

## Final machine state

Snapshot **`P36-VM-QUALIFIED {a38386fa-15f9-4f86-a231-5de585ff3cd7}`**.

(The host had refused snapshot creation from B4 through Stage C — see
`SESSION_CONTEXT.md` §13. The uninstalls and rollbacks of Stage C released
enough space inside the guest for this final one to succeed. That does not
retroactively change the B4/C records: those steps genuinely ran without a
fresh restore point, and they say so.)

Verified against the Gate-A reference (`evidence/verify-E-final.json`):

| property | result |
|---|---|
| differing fields vs `verify-A5-postinstall.json` | **NONE** |
| service | LocalSystem, AUTO_START (DELAYED), **STATE 4 RUNNING** |
| Service SID | UNRESTRICTED |
| pipe SDDL | byte-identical to the Gate-A baseline |
| install-dir ACLs | identical — Users RX, no write; service SID RX |
| every installed file SHA-256 | identical |
| ARP | one entry, `{FC8A3841-759D-B452-1864-161F84F56C03}` AetherCore 0.1.0 |
| `C:\ProgramData\AetherCore` | intact — 5 / state 4 / logs 1 / support-staging 0 |
| verbs | **8/8 RETURNED** — `hasanalaaa3a44\p36standarduser` (IsInRole False) and `hasanalaaa3a44\p36admin` (IsInRole True) |

The product on the VM is the realigned 0.1.0 build from `main` at `218e0d8`,
installed by the recorded ARM64 recipe, and it survived the entire lifecycle
and fault-injection programme.

## The defect this whole phase existed to close

The registered MSI carried PRE-FIX binaries, so any repair or reinstall would
have silently reverted both Windows IPC fixes. It is closed:

- `phase21-workspace/scripts/build-arm64-msi.cmd` is a permanent, recorded
  recipe. No AetherCore ARM64 package needs to be an ad-hoc invocation again.
- The package installed on the box was built by that recipe from `main`.
- B1 proved a repair now reinstates those binaries instead of reverting them.

## What remains that NO agent can do

1. **Desktop launch under a real standard user, and UAC.** Requires a human at
   the Parallels console. Every context this session used was a non-interactive
   one-shot Scheduled Task; it can prove token shape and verb behaviour, but it
   cannot drive a window, a consent prompt, or the secure desktop.
   **Caveat that must travel with any UAC finding from this VM:**
   `PromptOnSecureDesktop=0` here is a non-default deviation, so a UAC result
   observed on this machine is not evidence about a default-configured machine.
2. **Physical x86_64 qualification.** This VM is ARM64 throughout — OS,
   toolchain and every artifact. The x64 pipeline (`build-release.ps1`,
   `-arch x64`, the x64 ProductCode namespace) has not been exercised at all.
3. **Authenticode certificate.** Nothing in this session is signed.
   `sign-artifacts.ps1` and `sign-burn-bundle.ps1` were never invoked; the
   `-RequireSigning` path is untested.
4. **Production update endpoint.** No endpoint exists and none was contacted.
5. **Production key / HSM.** No private update key exists or was created. The
   installed `update-trust.json` ships disabled with zero channels.
6. **Dependency freeze from a trusted workstation.** `release/dependency-freeze.json`
   and the lock baselines must be produced on the trusted freeze workstation;
   `build-release.ps1` refuses to run without them, which is why the ARM64
   recipe is a separate script rather than a flag on that one.
7. **Free host disk space.** `/System/Volumes/Data` is at 100% with ~6.8 GiB
   free. This session deliberately did not delete anything to remedy it.

## Gate table

| gate | result | evidence |
|---|---|---|
| A1 canonical build script | PASS | `scripts/build-arm64-msi.cmd` committed; one tauri pre-build defect found and fixed with a config overlay |
| A2 rebuild + hashes | PASS | full rebuild from `main`; every artifact hashed; differences from the hand-deployed set tabulated and expected |
| A3 wix build + validate | PASS | both exit 0; **0** matches for `ICE\d+` in the whole log; no suppression |
| A4 reinstall vs upgrade | PASS | ProductCode proven a pure function of version+arch -> same-version REINSTALL |
| A5 install realigned MSI | PASS | `amus` refused 1638 (PackagecodeChanging), `vamus` exit 0; all five payload exes replaced; every security property unchanged |
| **GATE A** | **PASS** | package and installed files agree; 8/8 verbs |
| B1 repair | PASS | `/f` exit 0; zero differing fields; 8/8 verbs |
| B2 uninstall | PASS | service, pipe, ARP, HKLM, Start Menu all removed; ProgramData survives by authored design |
| B3 clean reinstall | PASS | plain `/i` exit 0; zero differing fields; 8/8 verbs |
| B4 upgrade 0.1.0 -> 0.1.1 | PASS | `RemoveExistingProducts` ran; single ARP entry at 0.1.1; no mixed state |
| **GATE B** | **PASS** | |
| C1 install killed mid-copy | RECORDED | no rollback runs; 4 orphaned files, no registration, no pipe; recovered |
| C2 service fails to start | PASS | 1603/1920 after the full 32 s wait; rollback complete, every field restored |
| C3 required file missing | RECORDED | 1053, STOPPED, no dangling pipe; `msiexec /f` repairs it fully |
| C4 custom action fails | PASS | 1603 in 2.0 s at the custom action; rollback complete |
| **GATE C** | **PASS** | four outcomes recorded, machine recovered every time |
| D1 `update stage` | BLOCKED BY DESIGN | unconditional match arm, `apps/aetherctl/src/offline.rs:138` |
| D2 stage/apply/rollback | BLOCKED | consequence of D1 — nothing stageable |
| D3 trust disabled, no network | PASS | `enabled:false, channels:[]`; typed refusal precedes any transport |
| **GATE D** | **PASS** | the blocker is named with evidence |
| E seal | PASS | `P36-VM-QUALIFIED {a38386fa-15f9-4f86-a231-5de585ff3cd7}` |
