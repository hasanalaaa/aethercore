# AetherCore

AetherCore is a lightweight, security-first Windows maintenance, driver, optimization, and diagnostic engine built with Rust, Tauri 2, and Svelte/TypeScript.

This repository contains the engineering implementation through **Phase 16: Final Production Qualification, Stress Matrix & General Availability Seal**:

- **Phase 0** — product contract, process boundaries, threat model, workspace, build foundation, and Windows support baseline.
- **Phase 1** — privileged maintenance service, immutable operation plans, UAC consent broker, SQLite journal, secured Protobuf named-pipe IPC, structured diagnostics, and live desktop shell.
- **Phase 2** — native PnP inventory, live Windows Update Agent driver discovery, Hardware/Compatible-ID matching, protected display/firmware policy, and Drivers dashboard.
- **Phase 3** — service-minted driver-install plans, verified System Restore protection, bound OEM package export, asynchronous WUA download/install, mutation barriers, PnP verification, reboot-aware verification, and recovery evidence.
- **Phase 4** — fixed-command DISM/SFC integrity repair plus exact-evidence, allowlist-only deep cleanup with NTFS reparse/final-path defenses.
- **Phase 5** — registry/Startup-folder/scheduled-task/service inventory, passive-default recommendations, protected-target policy, reversible typed mutations, durable change history, and restore plans.
- **Phase 6** — storage reliability/NVMe plus read-only ATA SMART telemetry, current memory pressure, WHEA/Event Log evidence, lightweight minidump metadata, confidence-scored triage cards, and bounded local diagnostic history.
- **Phase 7** — unified design tokens and Fluent/Mica presentation across all eight surfaces, typed navigation and command palette, keyboard/screen-reader accessibility, reduced-motion/high-contrast behavior, High-DPI monitor awareness, and English/Arabic RTL shell localization readiness.
- **Phase 8** — WiX 6 per-machine MSI/Burn packaging, service-specific SID/ACL hardening with an unrestricted maintenance token, reproducibility controls, SBOM/license auditing, signing pipeline, installer lifecycle verification, and release evidence.
- **Phase 9** — Windows principal/session ownership across IPC and all mutable/readback domains, secret-free consent intents with atomic one-shot authorization consumption, handle-level NTFS file identity enforcement, fail-closed dependency freeze, and prototype-surface sanitation.
- **Phase 10** — modular in-process Operation Kernel, persistent principal-bound IPC v7 sessions, per-principal replay/reset event streams, global RAII machine-mutation leasing, bounded read-work budgets, modular typed Protobuf contracts, zero renderer polling, and transient progress telemetry separated from the SQLite safety ledger.
- **Phase 11** — modular event-driven Svelte feature architecture, internal physical spring/velocity runtime, pointer-down tactile primitives, interruptible dialogs, momentum/rubber-band direct-manipulation support, semantic Mica material hierarchy, optical typography, and independent reduced-motion/transparency/contrast adaptations.
- **Phase 12** — typed product-wide English/Arabic catalogs, parameter/placeholder parity, CLDR-aware pluralization, semantic localization of maintenance/diagnostic prose, strict LTR isolation of technical evidence, CSS logical layout, and Arabic RTL typography qualification.
- **Phase 13** — finite WMI/Event Log waits, hierarchical collector deadlines/cancellation, watchdog quarantine, structured `EvtRender` parsing, bounded ATA/NVMe byte parsers, typed provider-fault isolation, and deterministic fault-injection gates.
- **Phase 14** — active-console owner-bound idle scheduling, fail-closed power/presentation/network/servicing/thermal eligibility, five closed read-only autonomous workloads, 100 ms preemption with linearized publication fencing, cooperative background resource governance, randomized jitter/equal-jitter backoff, durable principal-scoped cadence, and event-stream observability with zero background mutation authority.
- **Phase 15** — split-authority signed in-app update orchestration with user-scope HTTPS retrieval, LocalSystem verification/staging, fixed elevated Burn execution under the global Update mutation lease, plus preview-first privacy-sanitized deterministic support archives with independent installation-key fingerprint verification.
- **Phase 16** — multi-host Windows GA evidence matrix, real IPC v7 concurrency/reconnect probe, 24-hour extended soak with bounded memory/handle/thread growth, adversarial restart/fault resilience, signed Burn/MSI lifecycle qualification, and a cryptographic CMS GA seal bound to the exact release and evidence inventory.

AetherCore does not accept arbitrary privileged commands, arbitrary registry paths, arbitrary cleanup roots, caller-selected service names, arbitrary driver packages, arbitrary WMI/Event Log queries, or arbitrary diagnostic file paths. The privileged service remains the authority for system state and mutation; the user remains the authority for consent. Phase 6 is read-only and does not create mutation plans.

## Process architecture

- `aethercore-desktop.exe` — non-elevated Tauri shell and Svelte UI.
- `aethercore-maintenance-service.exe` — per-machine privileged maintenance service; owns native collection and all system mutation.
- `aethercore-consent-broker.exe` — one-shot `requireAdministrator` broker that receives only a non-secret consent-intent ID, retrieves the trusted plan summary from the service, and approves it only for the same logon principal.
- `aethercore-update-broker.exe` — fixed `requireAdministrator` update broker that receives only a service-issued one-shot update intent and executes only the service-verified, WiX/Burn-owned staged installer path.
- Persistent Protocol Buffers IPC v7 over a local Windows named pipe with bounded frames; every session is bound by the service to user SID + logon AuthenticationId + Windows session ID and carries multiplexed RPC responses plus principal-scoped ordered events.
- `aethercore-operation-kernel` owns machine mutation leasing, read-work budgets, recovery orchestration, cancellation, replay/reset events and in-memory progress telemetry; SQLite WAL + `synchronous=FULL` remains the durable safety ledger for plans, consent, mutation evidence and recovery checkpoints.

## Phase 5 passive-default contract

1. Every newly discovered startup/background item begins as `Unreviewed`.
2. `Unreviewed` means no action. Closing the app, rescanning, ignoring a recommendation, or doing nothing causes zero system changes.
3. `KeepEnabled` also produces no mutation and is omitted from the immutable mutation plan.
4. Only an explicit `Disable` decision can become a plan action, and only for a current service-issued, manageable, unprotected item.
5. Essential Windows/security/network/storage/input/accessibility/dependency-sensitive targets are fail-closed/protected.
6. Every startup/service mutation records exact original/applied evidence before change and is restorable only through a new immutable UAC-authorized plan.
7. A restart never automatically replays a startup/service mutation.
8. AetherCore does not invent boot-impact scores; insufficient direct evidence remains `Unknown` / `Insufficient evidence`.

## Phase 6 diagnostic honesty contract

1. There is no synthetic whole-PC, disk, or RAM health percentage.
2. Each storage metric is optional and source-attributed; unsupported values display `Not reported` rather than zero.
3. Current memory pressure is a resource metric, not a RAM hardware-health verdict.
4. A quiet WHEA window means only that no matching logged hardware errors were found in that window.
5. Kernel-Power Event 41 confirms an unclean shutdown/restart; it does not identify why it happened.
6. Minidump header metadata confirms crash evidence but does not name a culprit driver/module.
7. WHEA/crash timestamp proximity is reported as correlation, not proof of causation.
8. ATA SMART attributes are exposed only as vendor-defined raw evidence; AetherCore does not infer universal meanings/thresholds from their raw bytes.
9. System Event Log evidence is bounded to the newest 30 days and 128 matching records per scan.
10. If one diagnostic collector fails, successful evidence is retained and the scan is `Partial`; missing evidence is never silently represented as healthy.
11. Phase 6 is read-only: no repair, deletion, install, disable, restore, or reboot scheduling is exposed by its contracts.

## Supported development baseline

The current engineering contract targets Windows 11 x64, build 22621 or newer, NTFS installation/data locations, and Microsoft Edge WebView2 Evergreen. GA support is conferred only by the Phase 16 production seal after the required Windows host matrix, signed lifecycle, extended soak, resilience, supply-chain, and accessibility/bidirectional evidence pass.

## Fast start on Windows

Open PowerShell in the repository root:

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\setup-and-run.ps1 -InstallPrerequisites -RefreshDependencyFreeze
```

This bootstraps prerequisites, runs the **Phase 16 Windows source/native verification gate**, starts the maintenance service console elevated for development, and launches the Tauri desktop non-elevated.

Validate without launching the UI:

```powershell
.\scripts\verify-phase16.ps1
```

Optional **read-only** live Phase 6 telemetry/event probe through the inherited Phase 16 gate:

```powershell
.\scripts\verify-phase16.ps1 -LiveTelemetry
```

The Phase 6 live probe reads storage/memory telemetry, relevant System Event Log records, and minidump metadata. It intentionally performs no mutation.

Read `docs/AETHER_DESIGN_SYSTEM.md`, `docs/LUXURY_UI_UX.md`, `docs/ACCESSIBILITY_LOCALIZATION.md`, `docs/HARDWARE_CRASH_DIAGNOSTICS.md`, `docs/ENGINE_RELIABILITY.md`, `docs/AUTONOMOUS_MAINTENANCE.md`, `docs/SECURE_UPDATE_AND_SUPPORT_EXPORT.md`, `docs/FINAL_PRODUCTION_QUALIFICATION.md`, `docs/STARTUP_MANAGER.md`, `docs/PRODUCT_CONTRACT.md`, `docs/THREAT_MODEL.md`, `docs/BUILD.md`, and `docs/VALIDATION.md` before changing the native diagnostic or mutation boundaries.


## Dependency freeze prerequisite

The dependency/release freeze inherited from Phase 9 remains deliberately fail-closed through Phase 16. A trusted Windows dependency-freeze workstation must run `scripts\freeze-dependencies.ps1 -Refresh` after review and commit `Cargo.lock`, `pnpm-lock.yaml`, `release/dependency-locks.sha256`, `release/dependency-manifests.sha256`, and `release/dependency-freeze.json` together. CI and signed-release jobs only verify that approved baseline; they do not generate it implicitly.


## General Availability seal

`verify-phase16.ps1` is the master source/native qualification gate, but it intentionally does not claim GA by itself. After signed release packaging and the required Windows evidence matrix are complete, the protected production signing runner executes:

```powershell
.\scripts\verify-production.ps1 -ReleaseRoot .\out\release\0.1.0 -EvidenceDirectory .\out\ga-evidence
```

Only a successful production gate creates and verifies `GA-SEAL.json` plus detached `GA-SEAL.p7s`.

## Post-Phase-16 Enterprise convergence

The repository includes an additive adversarial convergence layer that re-audits Phase 0–16 rather than treating prior passing gates as proof of perfection. It closes support-export quota linearization, update observer reentrancy, common Windows FFI ownership, EventBus observability, strict UI event typing, and motion/accessibility hot-path issues.

On a qualified Windows developer/build host, run:

```powershell
.\scripts\verify-enterprise.ps1 -SkipOnlineSupplyChain
```

For the release stress profile (including the Phase 16 extended soak policy):

```powershell
.\scripts\verify-enterprise.ps1 -RuntimeStress -ExtendedSoak
```

A passing Enterprise gate does **not** create a GA seal. `verify-production.ps1` remains the sole production seal authority and requires the Enterprise audit plus the Phase 16 signed lifecycle/matrix/soak evidence.

See `docs/ENTERPRISE_ADVERSARIAL_AUDIT.md`, `ENTERPRISE_TRANSFORMATION_MATRIX.md`, and ADR 0019.

## Zenith adversarial convergence

The Zenith layer is an additive post-Enterprise interaction/accessibility/release-quality gate. It preserves every Phase 0–16 and Enterprise security/runtime invariant while permanently regression-testing the pointer-cancellation, reduced-motion dialog, semantic progress/toggle, and RTL defects found during the Zenith pass.

On a qualified Windows host, use the complete additive gate:

```powershell
.\scripts\verify-zenith.ps1 -SkipOnlineSupplyChain
```

`verify-zenith.ps1` runs the inherited Enterprise gate first, then the Zenith audit, and permits release packaging only after both pass. `verify-production.ps1` also requires the Zenith source audit before GA sealing. See `ZENITH_ISSUE_LEDGER.md`, `ZENITH_VERIFICATION_SUMMARY.md`, and ADR 0020.
