# Production Packaging & Security Architecture

## Purpose

Phase 8 packages AetherCore without collapsing its security model into a permanently elevated GUI. Installation elevation and runtime maintenance elevation are separate trust events.

## Installed principals and trust boundaries

| Component | Runtime identity | Purpose | May accept arbitrary commands? |
|---|---|---|---|
| Desktop | logged-in user / `asInvoker` | Tauri + Svelte UI | No |
| Maintenance service | LocalSystem service | typed native collectors and mutations | No |
| Consent broker | one-shot UAC / `requireAdministrator` | authorize one exact plan digest | No |
| Install hardener | deferred MSI service context | fixed service/ACL policy only | No |

The web-content capability manifest remains `core:default`; packaging does not add Tauri shell, process, filesystem, or HTTP permissions.

## MSI topology

The package is x64, per-machine, and targets Windows 11 client build 22621+. The direct MSI has an equivalent launch condition to the Burn bootstrapper, so managed MSI deployment cannot bypass the OS baseline. A stable `UpgradeCode` and `MajorUpgrade Schedule="afterInstallInitialize"` bind the upgrade family while retaining installer rollback of the previous product if the replacement fails. Four native binaries are installed beneath `%ProgramFiles%\AetherCore`. Service state remains beneath `%ProgramData%\AetherCore`.

`AetherCoreMaintenance` is installed as:

- `LocalSystem`;
- own-process service;
- automatic service in MSI ownership, with delayed auto-start applied by the fixed hardener;
- unrestricted service SID applied by the fixed hardener so service identity is ACL-addressable without write-restricting broad maintenance mutations;
- start on install;
- stop on repair/uninstall as Windows Installer requires for servicing;
- remove on uninstall.

The fixed-purpose hardener reasserts the security policy after `InstallServices` and before ordinary service startup. It runs on install and reinstall/repair but not final removal. AetherCore deliberately does not rely on the MSI `ServiceConfig` table for SID/delayed-start because the current WiX documentation calls out the underlying MSI functionality as unreliable; the fixed helper keeps those settings auditable and repairable through one path.

## Why a fixed hardener exists

MSI tables are good for installation ownership but the final AetherCore policy crosses SCM service SID/DACL configuration and two directory ACL trees. A general custom-action command runner would be an unacceptable privileged interface.

The helper therefore has a deliberately tiny input language: exactly one verb, `apply`. All object names and paths are derived from fixed Windows environment roots and fixed AetherCore names. It invokes only absolute System32 tooling.

Before applying the allowlist, it resets explicit ACE drift recursively. This matters during repair: merely appending the desired ACEs would leave a malicious or accidental broad explicit grant in place.

## ACL policy

### `%ProgramFiles%\AetherCore`

- LocalSystem — Full Control
- Administrators — Full Control
- Builtin Users — Read & Execute only
- `NT SERVICE\AetherCoreMaintenance` — Read & Execute
- inheritance removed after reset

This lets standard users launch/read the desktop while preventing them from replacing executable payloads.

### `%ProgramData%\AetherCore`

- LocalSystem — Full Control
- Administrators — Full Control
- `NT SERVICE\AetherCoreMaintenance` — Full Control
- no Builtin Users allow ACE
- inheritance removed after reset

The desktop must obtain privileged state through authenticated IPC rather than reading/modifying the service database directly.

## Service DACL

The hardener applies a fixed SCM DACL allowing SYSTEM and Administrators full service control while authenticated users receive only the narrow service query/interrogate rights encoded by the fixed SDDL. The lifecycle verifier compares the effective service DACL against this policy.

## Repair and uninstall

Repair is a security feature, not cosmetic UI. AetherCore intentionally does not author `ARPNOREPAIR`.

A disposable-VM lifecycle test deliberately grants Everyone Modify on the install tree, runs MSI repair, and then requires the broad ACE to disappear. This demonstrates that the repair path actually reasserts security state.

Uninstall removes executable payload and the Windows service. It deliberately preserves non-empty `%ProgramData%\AetherCore` so recovery journal/history is not silently destroyed by removing the application. A future explicit data-purge workflow, if added, must remain separately consented.

## WebView2 prerequisite

The Burn bundle checks Microsoft Edge WebView2 Evergreen registration in the documented machine/user locations. If absent, the bundle installs the Microsoft Evergreen bootstrapper silently and per-machine before the MSI.

The release script does not accept a prerequisite URL from the caller. It downloads from the fixed Microsoft Evergreen bootstrapper link, requires a valid Authenticode signature, and requires the signer subject to identify Microsoft Corporation before the file can enter the bundle.

## IPC hardening additions

Phase 8 preserves local-only named pipes and adds malformed-frame hardening around the exact production parser:

- request-frame allocation bound;
- response-frame allocation bound;
- 128-byte request-ID limit;
- safe request-ID character grammar;
- trailing-frame rejection;
- truncated-header/payload rejection;
- protobuf decode failure handling;
- deterministic no-panic malformed corpus;
- libFuzzer harness against the production decoder.

No TCP/UDP listener and no raw shell/command field is introduced.

## Path/reparse audit

The destructive cleanup path remains protected by the Phase 4 design:

- reparse points are rejected;
- ancestor traversal is checked;
- files are reopened by handle immediately before deletion;
- final paths are resolved by handle;
- service-generated immutable evidence remains the mutation authority.

Phase 8 re-audits these source invariants so packaging changes cannot accidentally bypass them.

## Privilege verification

The installer lifecycle gate extracts PE manifests and requires:

- desktop: `asInvoker`;
- consent broker: `requireAdministrator`.

It also fails if an elevated install auto-launches the desktop, which prevents the installer from accidentally creating an elevated GUI session.

`SERVICE_SID_TYPE_UNRESTRICTED` is deliberate for the privileged maintenance executor. AetherCore must mutate Windows and third-party resources whose DACLs cannot safely be rewritten to include the product service SID (for example HKLM startup values, arbitrary service configuration, WUA, DISM/SFC and approved cleanup targets). The service SID still gives AetherCore a service-specific ACL identity for its own ProgramData, mutation lock and named-pipe objects; it is not a sandbox and does not reduce LocalSystem authority. A future privilege-minimization design should split a restricted read/IPC broker from a narrowly typed privileged mutation executor only after native compatibility and recovery behavior are proven on the Windows matrix.

## PE mitigation verification

`build-release.ps1` invokes `verify-pe-hardening.ps1` immediately after staging the four production executables and before Authenticode signing. The verifier rejects non-AMD64/non-PE32+ payloads and requires the PE `DllCharacteristics` flags for high-entropy ASLR, dynamic-base ASLR, and NX/DEP compatibility. This makes linker-hardening drift observable at release time rather than assuming toolchain defaults.

## Burn signing and interactive privilege verification

A Burn bundle has two signed trust surfaces. `sign-burn-bundle.ps1` uses the pinned WiX CLI to detach the engine, signs that cached engine, reattaches it, and only then signs the whole bundle. This protects both the distributable setup executable and the engine Burn caches for maintenance operations.

Installer lifecycle verification checks embedded execution levels (`asInvoker` for the desktop and `requireAdministrator` for the consent broker). After installation, `verify-user-shell-privilege.ps1` is intentionally run from a non-elevated user session and verifies the live desktop process token is not elevated. The script refuses to run from an elevated PowerShell host so the test cannot accidentally validate the wrong security context.
