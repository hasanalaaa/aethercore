# ADR 0009 — Production Packaging and Installer Hardening

## Status

Accepted — Phase 8.

## Decision

Use WiX Toolset 6 to produce a per-machine x64 MSI plus a Burn bootstrapper for WebView2 prerequisite handling. Keep the desktop non-elevated, install the typed maintenance service as LocalSystem, and retain the one-shot UAC consent broker. Windows Installer owns service creation/removal; the fixed post-`InstallServices` hardener owns restricted-SID, delayed-auto, service-DACL, and directory-ACL enforcement.

Use a fixed-purpose `aethercore-install-hardener.exe` deferred MSI custom action rather than a script, MSI `ServiceConfig` dependency, or general command runner. The helper accepts only `apply`, uses fixed System32 tools, reasserts service SID/configuration/DACL, and normalizes Program Files/ProgramData ACLs.

Keep ProgramData recovery state on uninstall unless a future explicit data-purge action is separately authorized.

## Rationale

AetherCore needs per-machine service installation, repair, and native maintenance integration that is awkward to express as an MSIX-only deployment. WiX gives direct control over SCM, files, upgrade/repair semantics, Burn prerequisites, and signing order.

A general elevated installer script would materially enlarge the attack surface. The fixed helper keeps the privileged install language closed and auditable.

## Consequences

- MSI/Bundle must be built and lifecycle-tested on Windows.
- Repair becomes part of the security recovery path.
- Installer tests require a disposable VM because they alter SCM/ACL state.
- Service SID restriction is resource-isolation hardening, not a claim that LocalSystem becomes low privilege.
